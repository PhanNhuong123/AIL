import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  clusterCollapsed,
  toggleCluster,
  clusterCounts,
  groupByCluster,
  systemMode,
  moduleMode,
  _UNCLUSTERED_ID,
} from './stage-state';
import { multiClusterFixture } from './fixtures';
import type { GraphJson, ModuleJson } from '$lib/types';

beforeEach(() => {
  clusterCollapsed.set(new Set<string>());
  systemMode.set('Clusters');
  moduleMode.set('List');
});

describe('stage-state.ts', () => {
  it('test_group_by_cluster_preserves_cluster_order', () => {
    const g = multiClusterFixture();
    const groups = groupByCluster(g);
    expect(groups.length).toBe(3);
    expect(groups[0].cluster.id).toBe('c_identity');
    expect(groups[1].cluster.id).toBe('c_money');
    expect(groups[2].cluster.id).toBe('c_growth');
    expect(groups[0].modules.map((m) => m.id)).toEqual(['module:m_auth', 'module:m_session']);
    expect(groups[1].modules.map((m) => m.id)).toEqual(['module:m_billing', 'module:m_wallet']);
    expect(groups[2].modules.map((m) => m.id)).toEqual(['module:m_rewards', 'module:m_promos']);
  });

  it('test_group_by_cluster_empty_graph_returns_empty', () => {
    expect(groupByCluster(null)).toEqual([]);
    const empty: GraphJson = {
      project: {
        id: 'project:root',
        name: 'empty',
        description: '',
        nodeCount: 1,
        moduleCount: 0,
        fnCount: 0,
        status: 'ok',
      },
      clusters: [],
      modules: [],
      externals: [],
      relations: [],
      types: [],
      errors: [],
      issues: [],
      detail: {},
    };
    expect(groupByCluster(empty)).toEqual([]);
  });

  it('test_group_by_cluster_emits_unclustered_bucket', () => {
    const g = multiClusterFixture();
    // Rewire one module's cluster to something not in the clusters list.
    const stray: ModuleJson = {
      ...g.modules[0],
      id: 'module:m_orphan',
      name: 'orphan',
      cluster: 'c_ghost',
    };
    const modified: GraphJson = { ...g, modules: [...g.modules, stray] };
    const groups = groupByCluster(modified);
    // 3 known clusters + 1 synthesized unclustered bucket (appended last).
    expect(groups.length).toBe(4);
    const last = groups[groups.length - 1];
    expect(last.cluster.id).toBe(_UNCLUSTERED_ID);
    expect(last.cluster.name).toBe('(unclustered)');
    expect(last.modules.map((m) => m.id)).toEqual(['module:m_orphan']);
  });

  it('test_toggle_cluster_adds_and_removes', () => {
    expect(get(clusterCollapsed).has('c_money')).toBe(false);
    toggleCluster('c_money');
    expect(get(clusterCollapsed).has('c_money')).toBe(true);
    toggleCluster('c_money');
    expect(get(clusterCollapsed).has('c_money')).toBe(false);
    toggleCluster('c_identity');
    toggleCluster('c_growth');
    const both = get(clusterCollapsed);
    expect(both.has('c_identity')).toBe(true);
    expect(both.has('c_growth')).toBe(true);
    expect(both.size).toBe(2);
  });

  it('test_cluster_counts_correct', () => {
    const g = multiClusterFixture();
    const groups = groupByCluster(g);
    // Identity: 2 modules, all ok
    const identity = groups.find((x) => x.cluster.id === 'c_identity')!;
    expect(identity.counts).toEqual({ total: 2, failing: 0, warn: 0 });
    // Money: 2 modules, m_wallet is warn
    const money = groups.find((x) => x.cluster.id === 'c_money')!;
    expect(money.counts).toEqual({ total: 2, failing: 0, warn: 1 });
    // Growth: 2 modules, m_promos is fail
    const growth = groups.find((x) => x.cluster.id === 'c_growth')!;
    expect(growth.counts).toEqual({ total: 2, failing: 1, warn: 0 });
    // Direct clusterCounts on an empty list
    expect(clusterCounts([])).toEqual({ total: 0, failing: 0, warn: 0 });
  });
});
