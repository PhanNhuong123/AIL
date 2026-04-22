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
import { multiClusterFixture } from './fixtures';
import ModuleCard from './ModuleCard.svelte';

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

describe('ModuleCard.svelte', () => {
  it('test_module_card_shows_verify_metrics', async () => {
    const g = multiClusterFixture();
    graph.set(g);
    const wallet = g.modules.find((m) => m.id === 'module:m_wallet')!;

    const { container } = render(ModuleCard, { props: { module: wallet } });
    await tick();

    // m_wallet status='warn', 1/2 functions ok
    const issues = container.querySelector('[data-testid="pill-verify-issues"]');
    expect(issues).not.toBeNull();
    expect(issues?.textContent).toBe('⚠ issues');
    const ratio = container.querySelector('[data-testid="pill-verify-ratio"]');
    expect(ratio?.textContent).toBe('1/2');
    expect(container.querySelector('[data-testid="bar-dots"]')).not.toBeNull();
  });

  it('test_module_card_shows_rules_metrics_via_active_lens', async () => {
    const g = multiClusterFixture();
    graph.set(g);
    activeLens.set('rules');
    const billing = g.modules.find((m) => m.id === 'module:m_billing')!;

    const { container } = render(ModuleCard, { props: { module: billing } });
    await tick();

    // 2 rules, both proven
    const rulesCount = container.querySelector('[data-testid="pill-rules-count"]');
    expect(rulesCount).not.toBeNull();
    expect(rulesCount?.textContent).toBe('2 rules');
    // Verification pills must be absent
    expect(container.querySelector('[data-testid="pill-verify-verified"]')).toBeNull();
    // Bar switched to segmented
    expect(container.querySelector('[data-testid="bar-seg"]')).not.toBeNull();
  });

  it('test_module_card_shows_structure_metrics', async () => {
    const g = multiClusterFixture();
    graph.set(g);
    activeLens.set('structure');
    const wallet = g.modules.find((m) => m.id === 'module:m_wallet')!;

    const { container } = render(ModuleCard, { props: { module: wallet } });
    await tick();

    // structure: fn count + step count pills
    const fnCount = container.querySelector('[data-testid="pill-structure-fn-count"]');
    expect(fnCount).not.toBeNull();
    expect(fnCount?.textContent).toBe('2 fn');

    const stepCount = container.querySelector('[data-testid="pill-structure-step-count"]');
    expect(stepCount).not.toBeNull();
    expect(stepCount?.textContent).toBe('3 steps');
  });

  it('test_module_card_click_navigates', async () => {
    const g = multiClusterFixture();
    graph.set(g);
    const wallet = g.modules.find((m) => m.id === 'module:m_wallet')!;

    const { container } = render(ModuleCard, { props: { module: wallet } });
    await tick();

    const card = container.querySelector('[data-testid="module-card-module:m_wallet"]') as HTMLButtonElement;
    expect(card).not.toBeNull();
    fireEvent.click(card);
    await tick();

    expect(get(path)).toEqual(['project:root', 'module:m_wallet']);
    expect(get(selection).kind).toBe('module');
    expect(get(selection).id).toBe('module:m_wallet');
    expect(get(zoomLevel)).toBe(1);
    // navigateTo is the single history entry point → back length grows by 1
    expect(get(history).back.length).toBe(0); // current path was empty, push skipped
    // Navigate a second time from a non-empty path and confirm history entry.
    fireEvent.click(card);
    await tick();
    expect(get(history).back.length).toBe(1);
  });
});
