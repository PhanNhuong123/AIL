import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock Tauri BEFORE any Svelte imports (LensBanner pattern)
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

// vi.mock is hoisted above top-level const declarations, so co-hoist
// `navigateToMock` via vi.hoisted() so the mock factory can access it.
const { navigateToMock } = vi.hoisted(() => ({ navigateToMock: vi.fn() }));

vi.mock('$lib/chrome/toolbar-state', async (importOriginal) => {
  const actual = await importOriginal<typeof import('$lib/chrome/toolbar-state')>();
  return { ...actual, navigateTo: navigateToMock };
});

import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
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
import { multiClusterFixture } from './fixtures';
import FunctionRow from './FunctionRow.svelte';
import type { Lens } from '$lib/types';

beforeEach(() => {
  vi.clearAllMocks();
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

describe('FunctionRow.svelte', () => {
  it('test_function_row_shows_pills_for_each_lens', async () => {
    const lenses: Lens[] = ['structure', 'rules', 'verify', 'data', 'tests'];
    const g = multiClusterFixture();
    graph.set(g);
    const module = g.modules.find((m) => m.id === 'module:m_wallet')!;
    const fn = module.functions.find((f) => f.id === 'function:fn_transfer')!;

    // Collect pill text for structure vs verify to confirm they differ
    const pillsByLens: Record<string, string[]> = {};

    for (const lens of lenses) {
      activeLens.set(lens);
      const { container, unmount } = render(FunctionRow, { props: { fn, module } });
      await tick();

      const pills = Array.from(container.querySelectorAll('[class*="pill"]'));
      expect(pills.length, `at least 1 pill for lens=${lens}`).toBeGreaterThan(0);
      pillsByLens[lens] = pills.map((p) => (p as HTMLElement).textContent ?? '');

      unmount();
    }

    // structure pill text differs from verify pill text
    expect(pillsByLens['structure'].join(',')).not.toBe(pillsByLens['verify'].join(','));
  });

  it('test_function_row_click_calls_navigate_to', async () => {
    const g = multiClusterFixture();
    graph.set(g);
    const module = g.modules.find((m) => m.id === 'module:m_wallet')!;
    const fn = module.functions.find((f) => f.id === 'function:fn_transfer')!;
    path.set(['project:root', 'module:m_wallet']);

    const { container } = render(FunctionRow, { props: { fn, module } });
    await tick();

    const row = container.querySelector('[data-testid="function-row-function:fn_transfer"]') as HTMLButtonElement;
    expect(row).not.toBeNull();
    fireEvent.click(row);
    await tick();

    expect(navigateToMock).toHaveBeenCalledOnce();
    expect(navigateToMock).toHaveBeenCalledWith(
      ['project:root', 'module:m_wallet', 'function:fn_transfer'],
      'function',
      'function:fn_transfer',
      2,
    );
  });
});
