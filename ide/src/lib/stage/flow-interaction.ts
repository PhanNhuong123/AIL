/**
 * flow-interaction.ts — Pure state-machine reducer for flowchart interactions.
 *
 * INVARIANT 16.6-A: Dragging nodes, dragging ports, panning, zooming, and
 * selection are mutually exclusive — one active mode at a time.
 *
 * This file has ZERO Svelte imports. It is a pure reducer over plain data.
 *
 * Panning is WHEEL-ONLY (no mousedown-pan mode).
 *
 * **v4.0 → v4.1 read-only flag.** v4.0 shipped with `readOnly: true` baked in
 * via the `READ_ONLY_V4` constant. v4.1 promotes it to a per-state field on
 * [`InteractionState`] so the same reducer drives both modes — read-only
 * (legacy v4.0 chat-only) and edit (v4.1 default with drag persist). Callers
 * set `state.readOnly` from the [`flow-state.editLocked`] Svelte store; the
 * reducer keeps no opinion about how that store is owned.
 *
 * `READ_ONLY_V4` is retained as the v4.1 default for the **initial** value of
 * `state.readOnly` so legacy importers (Svelte components that have not yet
 * adopted the store wire-up) keep their old behaviour.
 *
 * Transition table:
 *   wheel (no mod)       any              → pan via applyPan, mode unchanged
 *   wheel + ctrl/meta    any              → zoom via applyZoom, mode unchanged
 *   zoom-in              any              → applyZoom(vp, -100), mode unchanged
 *   zoom-out             any              → applyZoom(vp, 100), mode unchanged
 *   zoom-reset           any              → viewport {x:0, y:0, k:1}, mode unchanged
 *   mousedown background idle             → clear selection, stay idle
 *   mousedown node       idle             → select + capture drag origin → dragging-node
 *                                           (when state.readOnly: select only, mode stays idle)
 *   mousedown port       idle             → init draft edge → dragging-port
 *                                           (when state.readOnly: no-op)
 *   mousemove            dragging-node    → setNodePos delta, unchanged
 *   mousemove            dragging-port    → update draft tip, unchanged
 *   mouseup on node      dragging-port    → push new edge → idle
 *   mouseup elsewhere    dragging-port    → clear draft → idle
 *   mouseup              dragging-node    → clear drag → idle
 *   mousedown (any)      dragging-*       → IGNORED (mutual exclusion)
 */

import { applyPan, applyZoom, setNodePos } from './flow-state';
import type { Viewport, DraftEdge, PortSide } from './flow-state';
import type { FlowNodeJson, FlowEdgeJson } from '$lib/types';

/**
 * Default read-only setting. v4.1 ships `false` so drag persist is on by
 * default; the IDE flips it to `true` only when the user enables read-only
 * mode (E key + lock toggle in the zoom controls).
 */
export const READ_ONLY_V4 = false;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type InteractionMode = 'idle' | 'dragging-node' | 'dragging-port';

export interface InteractionState {
  mode: InteractionMode;
  viewport: Viewport;
  positions: Map<string, { x: number; y: number }>;
  selectedNodeId: string | null;
  /** Active in dragging-node: the node being dragged */
  activeNodeId: string | null;
  /** Active in dragging-node: last mouse coords in SVG space */
  dragOriginX: number;
  dragOriginY: number;
  /** Active in dragging-port */
  draftEdge: DraftEdge | null;
  createdEdges: FlowEdgeJson[];
  /**
   * When true, the reducer treats `mousedown-node` as select-only, ignores
   * `mousedown-port`, and `hitTest` skips port detection. Sourced by the
   * `editLocked` store at the component layer; pure callers can override per
   * state.
   */
  readOnly: boolean;
}

export function emptyState(): InteractionState {
  return {
    mode: 'idle',
    viewport: { x: 0, y: 0, k: 1 },
    positions: new Map(),
    selectedNodeId: null,
    activeNodeId: null,
    dragOriginX: 0,
    dragOriginY: 0,
    draftEdge: null,
    createdEdges: [],
    readOnly: READ_ONLY_V4,
  };
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

export type InteractionEvent =
  | { type: 'wheel'; dx: number; dy: number; ctrlKey: boolean; metaKey: boolean }
  | { type: 'mousedown-background' }
  | { type: 'mousedown-node'; nodeId: string; svgX: number; svgY: number }
  | { type: 'mousedown-port'; nodeId: string; port: PortSide; tipX: number; tipY: number }
  | { type: 'mousemove'; svgX: number; svgY: number }
  | { type: 'mouseup-node'; nodeId: string }
  | { type: 'mouseup-background' }
  | { type: 'zoom-in' }
  | { type: 'zoom-out' }
  | { type: 'zoom-reset' };

// ---------------------------------------------------------------------------
// Hit test
// ---------------------------------------------------------------------------

export interface NodeBounds {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * Returns the first node whose bounding box contains (svgX, svgY),
 * checking port circles BEFORE node body (radius 6 px).
 *
 * Returns { hit: 'port', nodeId, port } | { hit: 'node', nodeId } | { hit: 'none' }
 *
 * `readOnly` defaults to false (v4.1 edit mode); pass `true` to suppress
 * port detection — useful when ports are not rendered.
 */
export function hitTest(
  nodes: NodeBounds[],
  selectedNodeId: string | null,
  svgX: number,
  svgY: number,
  readOnly = false,
): { hit: 'port'; nodeId: string; port: PortSide } | { hit: 'node'; nodeId: string } | { hit: 'none' } {
  const PORT_RADIUS = 6;

  // Only check ports when a node is selected. Skip in read-only mode —
  // ports are not rendered, so there is nothing to hit.
  if (selectedNodeId !== null && !readOnly) {
    const selNode = nodes.find((n) => n.id === selectedNodeId);
    if (selNode) {
      const ports: Array<{ port: PortSide; cx: number; cy: number }> = [
        { port: 'top',    cx: selNode.x + selNode.w / 2, cy: selNode.y },
        { port: 'right',  cx: selNode.x + selNode.w,     cy: selNode.y + selNode.h / 2 },
        { port: 'bottom', cx: selNode.x + selNode.w / 2, cy: selNode.y + selNode.h },
        { port: 'left',   cx: selNode.x,                 cy: selNode.y + selNode.h / 2 },
      ];
      for (const p of ports) {
        const dist = Math.hypot(svgX - p.cx, svgY - p.cy);
        if (dist <= PORT_RADIUS) {
          return { hit: 'port', nodeId: selNode.id, port: p.port };
        }
      }
    }
  }

  // Node body hit.
  for (const n of nodes) {
    if (svgX >= n.x && svgX <= n.x + n.w && svgY >= n.y && svgY <= n.y + n.h) {
      return { hit: 'node', nodeId: n.id };
    }
  }

  return { hit: 'none' };
}

// ---------------------------------------------------------------------------
// Pure reducer
// ---------------------------------------------------------------------------

export function reduce(state: InteractionState, event: InteractionEvent): InteractionState {
  switch (event.type) {
    case 'wheel': {
      if (event.ctrlKey || event.metaKey) {
        return { ...state, viewport: applyZoom(state.viewport, event.dy) };
      }
      return { ...state, viewport: applyPan(state.viewport, -event.dx, -event.dy) };
    }

    case 'mousedown-background': {
      // Mutual exclusion: ignore new mousedown while already dragging.
      if (state.mode !== 'idle') return state;
      return { ...state, selectedNodeId: null };
    }

    case 'mousedown-node': {
      // Mutual exclusion: ignore new mousedown while already dragging.
      if (state.mode !== 'idle') return state;
      if (state.readOnly) {
        // Select-only — clicking a node selects it but does not start a drag.
        return { ...state, selectedNodeId: event.nodeId };
      }
      return {
        ...state,
        mode: 'dragging-node',
        selectedNodeId: event.nodeId,
        activeNodeId: event.nodeId,
        dragOriginX: event.svgX,
        dragOriginY: event.svgY,
      };
    }

    case 'mousedown-port': {
      // Mutual exclusion: ignore new mousedown while already dragging.
      if (state.mode !== 'idle') return state;
      // Ports are not rendered in read-only mode; reject any synthetic event.
      if (state.readOnly) return state;
      const draft: DraftEdge = {
        fromId: event.nodeId,
        fromPort: event.port,
        tipX: event.tipX,
        tipY: event.tipY,
      };
      return {
        ...state,
        mode: 'dragging-port',
        draftEdge: draft,
      };
    }

    case 'mousemove': {
      if (state.mode === 'dragging-node' && state.activeNodeId !== null) {
        const dx = event.svgX - state.dragOriginX;
        const dy = event.svgY - state.dragOriginY;
        const nextPositions = setNodePos(
          state.positions,
          state.activeNodeId,
          dx,
          dy,
          // base coords — setNodePos uses current map value, delta is relative to last
          0, 0,
        );
        return {
          ...state,
          positions: nextPositions,
          dragOriginX: event.svgX,
          dragOriginY: event.svgY,
        };
      }
      if (state.mode === 'dragging-port' && state.draftEdge !== null) {
        return {
          ...state,
          draftEdge: { ...state.draftEdge, tipX: event.svgX, tipY: event.svgY },
        };
      }
      return state;
    }

    case 'mouseup-node': {
      if (state.mode === 'dragging-port' && state.draftEdge !== null) {
        // Drop on a different node → create edge.
        if (event.nodeId !== state.draftEdge.fromId) {
          const newEdge: FlowEdgeJson = {
            from: state.draftEdge.fromId,
            to: event.nodeId,
          };
          return {
            ...state,
            mode: 'idle',
            draftEdge: null,
            createdEdges: [...state.createdEdges, newEdge],
          };
        }
        // Drop on same node → cancel.
        return { ...state, mode: 'idle', draftEdge: null };
      }
      if (state.mode === 'dragging-node') {
        return { ...state, mode: 'idle', activeNodeId: null };
      }
      return state;
    }

    case 'mouseup-background': {
      if (state.mode === 'dragging-port') {
        return { ...state, mode: 'idle', draftEdge: null };
      }
      if (state.mode === 'dragging-node') {
        return { ...state, mode: 'idle', activeNodeId: null };
      }
      return state;
    }

    case 'zoom-in': {
      return { ...state, viewport: applyZoom(state.viewport, -100) };
    }

    case 'zoom-out': {
      return { ...state, viewport: applyZoom(state.viewport, 100) };
    }

    case 'zoom-reset': {
      return { ...state, viewport: { x: 0, y: 0, k: 1 } };
    }

    default:
      return state;
  }
}
