/**
 * Pure SVG layout helpers for SystemGraph (15.12-D). Determinism is critical:
 * positions must be identical for the same input across re-renders so graph
 * patches do not cause SVG nodes to jitter. Sort all input arrays by id (total
 * order) before assigning angles.
 *
 * MUST NOT import from $lib/bridge.ts; MUST NOT call computeLensMetrics.
 * Mirrors invariant 15.6-B / 15.12-D.
 */

export const SG_VIEWBOX = { w: 1180, h: 720, cx: 590, cy: 360 } as const;
export const SG_RING_R = 220;

export type Lens = 'structure' | 'rules' | 'verify' | 'data' | 'tests';

export interface ModulePos {
  id: string;
  x: number;
  y: number;
  clusterId: string | null;
  color: string;
}

export interface ClusterPos {
  id: string;
  cx: number;
  cy: number;
  r: number;
  color: string;
  label: string;
}

export interface GraphEdge {
  from: string;
  to: string;
  style: string;
}

export interface ModuleBadge {
  tone: 'ok' | 'warn' | 'fail' | 'muted';
  label: string;
}

interface ModuleInput {
  id: string;
  cluster?: string | null;
  clusterColor?: string;
  status?: string;
  functions?: { status?: string }[];
}

interface ClusterInput {
  id: string;
  color?: string;
  name?: string;
}

interface RelationInput {
  from: string;
  to: string;
  style?: string;
  label?: string;
}

const FALLBACK_COLOR = 'var(--ink-3)';

/**
 * Deterministic positions for clusters and modules.
 *
 * - Clusters arranged on an outer ring around (cx, cy), evenly spaced by index
 *   in the id-sorted cluster list.
 * - Modules placed on a smaller ring centered on their cluster (or a fallback
 *   ring centered at the viewBox center when they have no cluster).
 * - With 0 clusters, every module goes onto a single ring at (cx, cy).
 * - With 0 modules, returns empty arrays.
 */
export function computeModulePositions(
  modules: ModuleInput[],
  clusters: ClusterInput[],
): { modulePositions: ModulePos[]; clusterPositions: ClusterPos[] } {
  const sortedClusters = [...clusters].sort((a, b) => a.id.localeCompare(b.id));
  const sortedModules = [...modules].sort((a, b) => {
    const ca = a.cluster ?? '';
    const cb = b.cluster ?? '';
    if (ca !== cb) return ca.localeCompare(cb);
    return a.id.localeCompare(b.id);
  });

  const clusterPositions: ClusterPos[] = [];
  const clusterCenterById = new Map<string, { cx: number; cy: number; color: string }>();

  if (sortedClusters.length > 0) {
    const outerR = SG_RING_R;
    sortedClusters.forEach((c, i) => {
      const angle = (i / sortedClusters.length) * Math.PI * 2 - Math.PI / 2;
      const cx = SG_VIEWBOX.cx + Math.cos(angle) * outerR;
      const cy = SG_VIEWBOX.cy + Math.sin(angle) * outerR;
      const r = 90; // visible cluster ring radius for modules to sit inside
      const color = c.color ?? FALLBACK_COLOR;
      clusterPositions.push({ id: c.id, cx, cy, r, color, label: c.name ?? c.id });
      clusterCenterById.set(c.id, { cx, cy, color });
    });
  }

  // Group modules by cluster (or null) for ring placement.
  const groups = new Map<string | null, ModuleInput[]>();
  for (const m of sortedModules) {
    const key = m.cluster && clusterCenterById.has(m.cluster) ? m.cluster : null;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(m);
  }

  const modulePositions: ModulePos[] = [];
  for (const [clusterId, mods] of groups) {
    const center =
      clusterId !== null
        ? clusterCenterById.get(clusterId)!
        : { cx: SG_VIEWBOX.cx, cy: SG_VIEWBOX.cy, color: FALLBACK_COLOR };

    // Ring radius scales with module count, with a sane min/max.
    const ringR = Math.min(70, Math.max(30, mods.length * 12));
    const isFallback = clusterId === null && sortedClusters.length === 0;
    const effectiveR = isFallback ? SG_RING_R : ringR;

    mods.forEach((m, i) => {
      const angle =
        mods.length === 1
          ? -Math.PI / 2
          : (i / mods.length) * Math.PI * 2 - Math.PI / 2;
      const x = center.cx + Math.cos(angle) * effectiveR;
      const y = center.cy + Math.sin(angle) * effectiveR;
      modulePositions.push({
        id: m.id,
        x,
        y,
        clusterId,
        color: m.clusterColor ?? center.color,
      });
    });
  }

  return { modulePositions, clusterPositions };
}

/**
 * Filter relations by lens. Returns array stably sorted by `from→to` for
 * deterministic edge order across renders.
 */
export function filterRelations(
  relations: RelationInput[],
  lens: Lens,
): GraphEdge[] {
  const filtered = relations.filter((r) => {
    if (lens === 'data') return r.style === 'data';
    if (lens === 'rules') return r.style !== 'async';
    return true; // structure / verify / tests pass everything through
  });
  const edges: GraphEdge[] = filtered.map((r) => ({
    from: r.from,
    to: r.to,
    style: r.style ?? 'sync',
  }));
  edges.sort((a, b) => {
    const ka = `${a.from}→${a.to}`;
    const kb = `${b.from}→${b.to}`;
    return ka.localeCompare(kb);
  });
  return edges;
}

/**
 * Lens-aware module badge — count + tone for the small badge above each
 * module circle.
 */
export function badgeFor(
  module: ModuleInput,
  lens: Lens,
): ModuleBadge {
  const fns = module.functions ?? [];
  const failing = fns.filter((f) => f.status === 'fail').length;
  const warn = fns.filter((f) => f.status === 'warn').length;

  if (lens === 'verify') {
    if (failing > 0) return { tone: 'fail', label: String(failing) };
    if (warn > 0) return { tone: 'warn', label: String(warn) };
    return { tone: 'ok', label: 'ok' };
  }

  // structure / rules / data / tests — show function count, muted
  return { tone: 'muted', label: String(fns.length) };
}
