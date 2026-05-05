import { describe, it, expect } from 'vitest';
import { reduce, emptyState, hitTest } from './flow-interaction';
import type { InteractionState, InteractionEvent } from './flow-interaction';

describe('flow-interaction (pure reducer)', () => {
  it('test_reducer_idle_mousedown_node_transitions_to_dragging_node', () => {
    // v4.1 default: edit mode (state.readOnly = false) → mousedown enters drag.
    const s0 = emptyState();
    const event: InteractionEvent = {
      type: 'mousedown-node',
      nodeId: 'n_do',
      svgX: 100,
      svgY: 150,
    };
    const s1 = reduce(s0, event);
    expect(s1.mode).toBe('dragging-node');
    expect(s1.selectedNodeId).toBe('n_do');
    expect(s1.activeNodeId).toBe('n_do');
    expect(s1.dragOriginX).toBe(100);
    expect(s1.dragOriginY).toBe(150);
  });

  it('test_reducer_readonly_mousedown_node_select_only', () => {
    // Read-only mode contract: mousedown-node must select but stay idle.
    const s0: InteractionState = { ...emptyState(), readOnly: true };
    const s1 = reduce(s0, {
      type: 'mousedown-node',
      nodeId: 'n_do',
      svgX: 100,
      svgY: 150,
    });
    expect(s1.mode).toBe('idle');
    expect(s1.selectedNodeId).toBe('n_do');
    expect(s1.activeNodeId).toBeNull();
  });

  it('test_reducer_readonly_mousedown_port_is_noop', () => {
    // Ports are not rendered in read-only; a synthetic event must not
    // transition into dragging-port either.
    const s0: InteractionState = { ...emptyState(), readOnly: true };
    const s1 = reduce(s0, {
      type: 'mousedown-port',
      nodeId: 'n_do',
      port: 'right',
      tipX: 100,
      tipY: 100,
    });
    expect(s1.mode).toBe('idle');
    expect(s1.draftEdge).toBeNull();
  });

  it('test_reducer_idle_wheel_pans_viewport', () => {
    const s0 = emptyState();
    const event: InteractionEvent = {
      type: 'wheel',
      dx: 20,
      dy: 30,
      ctrlKey: false,
      metaKey: false,
    };
    const s1 = reduce(s0, event);
    expect(s1.mode).toBe('idle');
    expect(s1.viewport.x).toBe(-20);
    expect(s1.viewport.y).toBe(-30);
    expect(s1.viewport.k).toBe(1); // unchanged
  });

  it('test_16_6_A_mutual_exclusion_mousedown_ignored_while_dragging', () => {
    // Start dragging-node
    const s0 = emptyState();
    const s1 = reduce(s0, {
      type: 'mousedown-node',
      nodeId: 'n_do',
      svgX: 100,
      svgY: 150,
    });
    expect(s1.mode).toBe('dragging-node');

    // Another mousedown-node while already dragging → IGNORED
    const s2 = reduce(s1, {
      type: 'mousedown-node',
      nodeId: 'n_start',
      svgX: 200,
      svgY: 50,
    });
    // Mode must not change; selectedNodeId must not change
    expect(s2.mode).toBe('dragging-node');
    expect(s2.selectedNodeId).toBe('n_do'); // unchanged
    expect(s2.activeNodeId).toBe('n_do');   // unchanged

    // Also test mousedown-background ignored while dragging
    const s3 = reduce(s1, { type: 'mousedown-background' });
    expect(s3.mode).toBe('dragging-node');
    expect(s3.selectedNodeId).toBe('n_do'); // unchanged

    // Port drag also ignored while dragging-node
    const s4 = reduce(s1, {
      type: 'mousedown-port',
      nodeId: 'n_do',
      port: 'right',
      tipX: 150,
      tipY: 150,
    });
    expect(s4.mode).toBe('dragging-node'); // unchanged
  });

  it('test_reducer_drag_mousemove_updates_positions', () => {
    let state = emptyState();
    // Seed an initial position for the node we will drag.
    state = { ...state, positions: new Map([['n_do', { x: 100, y: 100 }]]) };

    // mousedown-node enters dragging mode and captures origin.
    state = reduce(state, {
      type: 'mousedown-node',
      nodeId: 'n_do',
      svgX: 150,
      svgY: 120,
    });
    expect(state.mode).toBe('dragging-node');

    // Mousemove applies delta (20, 10) relative to the drag origin.
    state = reduce(state, { type: 'mousemove', svgX: 170, svgY: 130 });

    const pos = state.positions.get('n_do');
    expect(pos?.x).toBe(120); // 100 + 20
    expect(pos?.y).toBe(110); // 100 + 10

    // mouseup ends drag and clears activeNodeId.
    state = reduce(state, { type: 'mouseup-node', nodeId: 'n_do' });
    expect(state.mode).toBe('idle');
    expect(state.activeNodeId).toBeNull();
  });
});

describe('zoom reducer events (task 15.8)', () => {
  it('test_flowchart_zoom_clamps_at_max', () => {
    let state = emptyState();
    for (let i = 0; i < 50; i++) {
      state = reduce(state, { type: 'zoom-in' });
    }
    expect(state.viewport.k).toBeLessThanOrEqual(4.0);
  });

  it('test_flowchart_zoom_clamps_at_min', () => {
    let state = emptyState();
    for (let i = 0; i < 50; i++) {
      state = reduce(state, { type: 'zoom-out' });
    }
    expect(state.viewport.k).toBeGreaterThanOrEqual(0.2);
  });

  it('test_zoom_in_increases_k', () => {
    const s0 = emptyState();
    const s1 = reduce(s0, { type: 'zoom-in' });
    expect(s1.viewport.k).toBeGreaterThan(1);
    expect(s1.mode).toBe('idle'); // mode unchanged
  });

  it('test_zoom_out_decreases_k', () => {
    const s0 = emptyState();
    const s1 = reduce(s0, { type: 'zoom-out' });
    expect(s1.viewport.k).toBeLessThan(1);
    expect(s1.mode).toBe('idle');
  });

  it('test_zoom_reset_restores_viewport', () => {
    let state = emptyState();
    state = reduce(state, { type: 'zoom-in' });
    state = reduce(state, { type: 'zoom-in' });
    state = reduce(state, { type: 'zoom-reset' });
    expect(state.viewport.k).toBe(1);
    expect(state.viewport.x).toBe(0);
    expect(state.viewport.y).toBe(0);
    expect(state.mode).toBe('idle');
  });
});

describe('hitTest', () => {
  it('returns port hit before node hit when node is selected (edit mode)', () => {
    const nodes = [{ id: 'n1', x: 100, y: 100, w: 80, h: 40 }];
    // Port 'right' is at (180, 120) for this node. v4.1 default readOnly=false
    // → port detection is on.
    const result = hitTest(nodes, 'n1', 179, 120);
    expect(result.hit).toBe('port');
    if (result.hit === 'port') {
      expect(result.port).toBe('right');
    }
  });

  it('falls through to node hit when readOnly=true even with port-area click', () => {
    const nodes = [{ id: 'n1', x: 100, y: 100, w: 80, h: 40 }];
    // Same coordinates as above, but readOnly=true → port detection skipped,
    // resolves to a node-body hit instead.
    const result = hitTest(nodes, 'n1', 179, 120, true);
    expect(result.hit).toBe('node');
  });

  it('returns node hit when click is inside the body', () => {
    const nodes = [{ id: 'n1', x: 100, y: 100, w: 80, h: 40 }];
    const result = hitTest(nodes, null, 140, 120);
    expect(result.hit).toBe('node');
    if (result.hit === 'node') {
      expect(result.nodeId).toBe('n1');
    }
  });

  it('returns none for empty space', () => {
    const nodes = [{ id: 'n1', x: 100, y: 100, w: 80, h: 40 }];
    const result = hitTest(nodes, null, 300, 300);
    expect(result.hit).toBe('none');
  });
});
