/**
 * flow-state.ts — Flow-view-scoped stores and pure viewport/position helpers.
 *
 * Stores:
 *   flowMode           — active sub-mode tab: Swim | Flowchart | Code
 *   flowViewport       — SVG pan/zoom transform { x, y, k }
 *   flowNodePositions  — mutable per-node positions (overrides FlowNodeJson.x/y)
 *   flowSelectedNodeId — id of the selected node (null = none)
 *   flowFocusedNodeId  — id of the focused node for dimming neighbours (null = no focus)
 *   flowDraftEdge      — edge being drawn from a port drag (null = none)
 *   createdEdges       — edges created by port-drag interactions this session
 *
 * Pure helpers: clampZoom, applyPan, applyZoom, getNodePos, setNodePos
 */

import { writable, get } from 'svelte/store';
import type { Writable } from 'svelte/store';
import type { FlowEdgeJson, FlowNodeJson } from '$lib/types';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type FlowMode = 'Swim' | 'Flowchart' | 'Code';

export interface Viewport {
  x: number;
  y: number;
  k: number; // scale factor
}

export interface DraftEdge {
  fromId: string;
  fromPort: PortSide;
  tipX: number;
  tipY: number;
}

export type PortSide = 'top' | 'right' | 'bottom' | 'left';

// ---------------------------------------------------------------------------
// Stores
// ---------------------------------------------------------------------------

export const flowMode: Writable<FlowMode>                        = writable('Swim');
export const flowViewport: Writable<Viewport>                    = writable({ x: 0, y: 0, k: 1 });
export const flowNodePositions: Writable<Map<string, { x: number; y: number }>> = writable(new Map());
export const flowSelectedNodeId: Writable<string | null>         = writable(null);
export const flowFocusedNodeId: Writable<string | null>          = writable(null);
export const flowDraftEdge: Writable<DraftEdge | null>           = writable(null);
export const createdEdges: Writable<FlowEdgeJson[]>              = writable([]);

// v4.1 edit-mode toggle. `false` (default) = drag, port circles, and
// inline edits are enabled. `true` = read-only canvas (the v4.0 ship-mode
// behaviour). Driven by the `E` keyboard shortcut and the lock toggle in
// the zoom controls.
export const editLocked: Writable<boolean>                       = writable(false);

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

const MIN_ZOOM = 0.2;
const MAX_ZOOM = 4.0;

export function clampZoom(k: number): number {
  return Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, k));
}

export function applyPan(vp: Viewport, dx: number, dy: number): Viewport {
  return { ...vp, x: vp.x + dx, y: vp.y + dy };
}

export function applyZoom(vp: Viewport, deltaY: number): Viewport {
  const factor = deltaY > 0 ? 0.9 : 1 / 0.9;
  return { ...vp, k: clampZoom(vp.k * factor) };
}

/**
 * Get the current position for a node, falling back to FlowNodeJson coords.
 */
export function getNodePos(
  positions: Map<string, { x: number; y: number }>,
  node: FlowNodeJson,
): { x: number; y: number } {
  return positions.get(node.id) ?? { x: node.x, y: node.y };
}

/**
 * Return a new Map with the given node's position updated by delta.
 */
export function setNodePos(
  positions: Map<string, { x: number; y: number }>,
  nodeId: string,
  dx: number,
  dy: number,
  baseX: number,
  baseY: number,
): Map<string, { x: number; y: number }> {
  const next = new Map(positions);
  const cur = positions.get(nodeId) ?? { x: baseX, y: baseY };
  next.set(nodeId, { x: cur.x + dx, y: cur.y + dy });
  return next;
}

/**
 * Seed positions from FlowNodeJson array (call when loading a new flowchart).
 *
 * `layoutOverrides` carries previously persisted drag positions from the
 * project's sidecar layout. When an entry matches a node id, its `x`/`y`
 * replace the FlowNodeJson defaults so reload preserves the user's last
 * drag. Pass `null`/`undefined` (or call with one argument) for the legacy
 * v4.0 read-only behaviour.
 */
export function seedPositions(
  nodes: FlowNodeJson[],
  layoutOverrides?: Record<string, { x: number; y: number }> | null,
): void {
  const m = new Map<string, { x: number; y: number }>();
  for (const n of nodes) {
    const override = layoutOverrides?.[n.id];
    m.set(n.id, override ? { x: override.x, y: override.y } : { x: n.x, y: n.y });
  }
  flowNodePositions.set(m);
}

/**
 * Reset all flow stores to defaults. Call when navigating away.
 */
export function resetFlowState(): void {
  flowMode.set('Swim');
  flowViewport.set({ x: 0, y: 0, k: 1 });
  flowNodePositions.set(new Map());
  flowSelectedNodeId.set(null);
  flowFocusedNodeId.set(null);
  flowDraftEdge.set(null);
  createdEdges.set([]);
}

// Re-export get for convenience in interaction module
export { get };
