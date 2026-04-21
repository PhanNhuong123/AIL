import { describe, it, expect } from 'vitest';
import { reduce, emptyState, hitTest } from './flow-interaction';
import type { InteractionState, InteractionEvent } from './flow-interaction';

describe('flow-interaction (pure reducer)', () => {
  it('test_reducer_idle_mousedown_node_transitions_to_dragging_node', () => {
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
});

describe('hitTest', () => {
  it('returns port hit before node hit when node is selected', () => {
    const nodes = [{ id: 'n1', x: 100, y: 100, w: 80, h: 40 }];
    // Port 'right' is at (180, 120) for this node
    const result = hitTest(nodes, 'n1', 179, 120);
    expect(result.hit).toBe('port');
    if (result.hit === 'port') {
      expect(result.port).toBe('right');
    }
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
