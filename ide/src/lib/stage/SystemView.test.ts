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
import {
  systemMode,
  moduleMode,
  clusterCollapsed,
} from './stage-state';
import { multiClusterFixture, bigSystemFixture } from './fixtures';
import SystemView from './SystemView.svelte';
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

describe('SystemView.svelte', () => {
  it('test_system_view_clusters_default', async () => {
    const g = multiClusterFixture();
    graph.set(g);

    const { container } = render(SystemView, { props: { graph: g } });
    await tick();

    expect(get(systemMode)).toBe('Clusters');
    const clustersRoot = container.querySelector('[data-testid="system-clusters"]');
    expect(clustersRoot).not.toBeNull();
    // 3 cluster groups rendered
    expect(container.querySelector('[data-testid="cluster-group-c_identity"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="cluster-group-c_money"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="cluster-group-c_growth"]')).not.toBeNull();
  });

  it('test_cluster_collapse_toggle', async () => {
    const g = multiClusterFixture();
    graph.set(g);

    const { container } = render(SystemView, { props: { graph: g } });
    await tick();

    // Initially expanded — grid visible
    expect(container.querySelector('[data-testid="cluster-grid-c_money"]')).not.toBeNull();

    // Click the header to collapse
    const header = container.querySelector('[data-testid="cluster-header-c_money"]') as HTMLButtonElement;
    expect(header).not.toBeNull();
    fireEvent.click(header);
    await tick();

    expect(get(clusterCollapsed).has('c_money')).toBe(true);
    // Grid removed when collapsed
    expect(container.querySelector('[data-testid="cluster-grid-c_money"]')).toBeNull();

    // Click again to expand
    fireEvent.click(header);
    await tick();
    expect(get(clusterCollapsed).has('c_money')).toBe(false);
    expect(container.querySelector('[data-testid="cluster-grid-c_money"]')).not.toBeNull();
  });

  it('test_grid_mode_flat_layout', async () => {
    const g = multiClusterFixture();
    graph.set(g);

    const { container } = render(SystemView, { props: { graph: g } });
    await tick();

    const gridBtn = container.querySelector('[data-testid="system-mode-btn-grid"]') as HTMLButtonElement;
    expect(gridBtn).not.toBeNull();
    fireEvent.click(gridBtn);
    await tick();

    expect(get(systemMode)).toBe('Grid');
    const gridRoot = container.querySelector('[data-testid="system-grid"]');
    expect(gridRoot).not.toBeNull();
    // No cluster groups in grid mode
    expect(container.querySelector('[data-testid="cluster-group-c_identity"]')).toBeNull();
    // All 6 modules flat
    const cards = container.querySelectorAll('[data-testid^="module-card-module:"]');
    expect(cards.length).toBe(6);
    // Externals block rendered because the fixture has one external.
    expect(container.querySelector('[data-testid="system-grid-externals"]')).not.toBeNull();
  });

  it('test_system_view_head_action_reflects_active_lens', async () => {
    const lenses: Lens[] = ['structure', 'rules', 'verify', 'data', 'tests'];
    const g = multiClusterFixture();
    graph.set(g);

    const { container } = render(SystemView, { props: { graph: g } });
    await tick();

    for (const lens of lenses) {
      activeLens.set(lens);
      await tick();

      // Current lens chip is present
      const current = container.querySelector(`[data-testid="system-head-action-${lens}"]`);
      expect(current, `head action for ${lens} should be present`).not.toBeNull();

      // All other lens chips are absent
      for (const other of lenses) {
        if (other === lens) continue;
        const otherEl = container.querySelector(`[data-testid="system-head-action-${other}"]`);
        expect(otherEl, `head action for ${other} should be absent when lens=${lens}`).toBeNull();
      }
    }
  });

  it('test_system_view_externals_present_when_nonempty', async () => {
    const g = multiClusterFixture(); // has 1 external: ext_stripe
    graph.set(g);

    const { container } = render(SystemView, { props: { graph: g } });
    await tick();

    // Switch to grid mode to see externals
    const gridBtn = container.querySelector('[data-testid="system-mode-btn-grid"]') as HTMLButtonElement;
    fireEvent.click(gridBtn);
    await tick();

    expect(container.querySelector('[data-testid="system-grid-externals"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="system-grid-external-external:ext_stripe"]')).not.toBeNull();
  });

  it('test_system_view_externals_absent_when_empty', async () => {
    const g = multiClusterFixture();
    g.externals = [];
    graph.set(g);

    const { container } = render(SystemView, { props: { graph: g } });
    await tick();

    // Switch to grid mode
    const gridBtn = container.querySelector('[data-testid="system-mode-btn-grid"]') as HTMLButtonElement;
    fireEvent.click(gridBtn);
    await tick();

    expect(container.querySelector('[data-testid="system-grid-externals"]')).toBeNull();
  });

  it('test_cluster_header_counts_correct', async () => {
    const g = multiClusterFixture();
    graph.set(g);

    const { container } = render(SystemView, { props: { graph: g } });
    await tick();

    // c_identity: 2 modules, 0 failing, 0 warn
    const idTotal = container.querySelector('[data-testid="cluster-header-count-modules-c_identity"]');
    expect(idTotal?.textContent).toBe('2');

    // c_money: 2 modules, 1 warn (m_wallet)
    const moneyTotal = container.querySelector('[data-testid="cluster-header-count-modules-c_money"]');
    expect(moneyTotal?.textContent).toBe('2');
    const moneyWarn = container.querySelector('[data-testid="cluster-header-count-warn-c_money"]');
    expect(moneyWarn?.textContent).toContain('1 warn');

    // c_growth: 2 modules, 1 failing (m_promos)
    const growthFail = container.querySelector('[data-testid="cluster-header-count-failing-c_growth"]');
    expect(growthFail?.textContent).toContain('1 failing');
  });

  it('test_system_view_renders_200_nodes_without_crash', async () => {
    const big = bigSystemFixture(200);
    graph.set(big);

    // Must not throw and must render at least one cluster + some cards.
    const { container } = render(SystemView, { props: { graph: big } });
    await tick();

    expect(container.querySelector('[data-testid="system-clusters"]')).not.toBeNull();
    // 5 clusters × 4 modules = 20 cards in the default clusters mode.
    const cards = container.querySelectorAll('[data-testid^="module-card-module:"]');
    expect(cards.length).toBe(20);
    // Sanity: the fixture reports at least 200 nodes.
    expect(big.project.nodeCount).toBeGreaterThanOrEqual(200);
  });
});
