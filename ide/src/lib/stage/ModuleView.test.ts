import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import {
  graph,
  selection,
  activeLens,
  path,
  history,
  paletteVisible,
} from '$lib/stores';
import { zoomLevel, pickerOpen, pickerItems } from '$lib/chrome/toolbar-state';
import { systemMode, moduleMode, clusterCollapsed } from './stage-state';
import { walletFixture } from '$lib/chrome/fixtures';
import ModuleView from './ModuleView.svelte';
import type { Lens } from '$lib/types';

beforeEach(() => {
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  activeLens.set('verify');
  path.set([]);
  history.set({ back: [], forward: [] });
  paletteVisible.set(false);
  zoomLevel.set(0);
  pickerOpen.set(false);
  pickerItems.set([]);
  systemMode.set('Clusters');
  moduleMode.set('List');
  clusterCollapsed.set(new Set<string>());
});

describe('ModuleView.svelte', () => {
  it('test_module_view_lists_functions', async () => {
    const g = walletFixture();
    graph.set(g);
    const wallet = g.modules.find((m) => m.id === 'module:m_wallet')!;
    path.set(['project:root', 'module:m_wallet']);
    selection.set({ kind: 'module', id: 'module:m_wallet' });
    zoomLevel.set(1);

    const { container } = render(ModuleView, { props: { module: wallet } });
    await tick();

    expect(container.querySelector('[data-testid="module-view"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="module-view-name"]')?.textContent).toBe('wallet');
    const list = container.querySelector('[data-testid="module-view-function-list"]');
    expect(list).not.toBeNull();
    // 2 functions: fn_transfer, fn_balance
    expect(container.querySelector('[data-testid="function-row-function:fn_transfer"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="function-row-function:fn_balance"]')).not.toBeNull();
  });

  it('test_function_click_navigates_flow', async () => {
    const g = walletFixture();
    graph.set(g);
    const wallet = g.modules.find((m) => m.id === 'module:m_wallet')!;
    path.set(['project:root', 'module:m_wallet']);
    selection.set({ kind: 'module', id: 'module:m_wallet' });
    zoomLevel.set(1);

    const { container } = render(ModuleView, { props: { module: wallet } });
    await tick();

    const row = container.querySelector('[data-testid="function-row-function:fn_transfer"]') as HTMLButtonElement;
    expect(row).not.toBeNull();
    fireEvent.click(row);
    await tick();

    expect(get(path)).toEqual(['project:root', 'module:m_wallet', 'function:fn_transfer']);
    expect(get(selection).kind).toBe('function');
    expect(get(selection).id).toBe('function:fn_transfer');
    expect(get(zoomLevel)).toBe(2);
    // navigateTo pushed exactly one entry (previous path was non-empty).
    expect(get(history).back.length).toBe(1);
  });

  it('test_module_view_head_action_reflects_active_lens', async () => {
    const lenses: Lens[] = ['structure', 'rules', 'verify', 'data', 'tests'];
    const g = walletFixture();
    graph.set(g);
    const wallet = g.modules.find((m) => m.id === 'module:m_wallet')!;

    const { container } = render(ModuleView, { props: { module: wallet } });
    await tick();

    for (const lens of lenses) {
      activeLens.set(lens);
      await tick();

      // Current lens chip is present
      const current = container.querySelector(`[data-testid="module-head-action-${lens}"]`);
      expect(current, `head action for ${lens} should be present`).not.toBeNull();

      // All other lens chips are absent
      for (const other of lenses) {
        if (other === lens) continue;
        const otherEl = container.querySelector(`[data-testid="module-head-action-${other}"]`);
        expect(otherEl, `head action for ${other} should be absent when lens=${lens}`).toBeNull();
      }
    }
  });
});
