/**
 * stage-state.ts — Stage-scoped stores and pure helpers.
 *
 * Stores: systemMode (Clusters/Grid/Graph), moduleMode (List/Graph),
 * clusterCollapsed (Set of cluster ids collapsed in the Clusters view).
 *
 * Helpers are pure: groupByCluster, clusterCounts. toggleCluster wraps
 * the clusterCollapsed store.
 */

import { writable, get } from 'svelte/store';
import type { Writable } from 'svelte/store';
import type { GraphJson, ModuleJson, ClusterJson, Status } from '$lib/types';

export type SystemMode = 'Clusters' | 'Grid' | 'Graph';
export type ModuleMode = 'List' | 'Graph';

export interface ClusterCounts {
  total: number;
  failing: number;
  warn: number;
}

export interface ClusterGroupEntry {
  cluster: ClusterJson;
  modules: ModuleJson[];
  counts: ClusterCounts;
}

export const systemMode: Writable<SystemMode> = writable('Clusters');
export const moduleMode: Writable<ModuleMode> = writable('List');
export const clusterCollapsed: Writable<Set<string>> = writable(new Set<string>());

export function toggleCluster(id: string): void {
  const cur = get(clusterCollapsed);
  const next = new Set(cur);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  clusterCollapsed.set(next);
}

export function clusterCounts(modules: ModuleJson[]): ClusterCounts {
  let failing = 0;
  let warn = 0;
  for (const m of modules) {
    if (m.status === 'fail') failing++;
    else if (m.status === 'warn') warn++;
  }
  return { total: modules.length, failing, warn };
}

const UNCLUSTERED_ID = '__unclustered';
const UNCLUSTERED_COLOR = 'var(--ink-3)';

export function groupByCluster(g: GraphJson | null): ClusterGroupEntry[] {
  if (!g) return [];

  const result: ClusterGroupEntry[] = [];
  const knownIds = new Set<string>();

  for (const c of g.clusters) {
    knownIds.add(c.id);
    const mods = g.modules.filter((m) => m.cluster === c.id);
    result.push({ cluster: c, modules: mods, counts: clusterCounts(mods) });
  }

  const stray = g.modules.filter((m) => !knownIds.has(m.cluster));
  if (stray.length > 0) {
    const fallback: ClusterJson = {
      id: UNCLUSTERED_ID,
      name: '(unclustered)',
      color: UNCLUSTERED_COLOR,
    };
    result.push({ cluster: fallback, modules: stray, counts: clusterCounts(stray) });
  }

  return result;
}

export const _UNCLUSTERED_ID = UNCLUSTERED_ID;
export type { Status };
