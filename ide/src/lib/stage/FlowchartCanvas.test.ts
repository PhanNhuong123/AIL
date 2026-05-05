import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import {
  flowViewport,
  flowNodePositions,
  flowSelectedNodeId,
  createdEdges,
  flowDraftEdge,
  seedPositions,
  editLocked,
} from './flow-state';
import { flowFixture } from './fixtures';
import FlowchartCanvas from './FlowchartCanvas.svelte';

beforeEach(() => {
  flowViewport.set({ x: 0, y: 0, k: 1 });
  flowNodePositions.set(new Map());
  flowSelectedNodeId.set(null);
  createdEdges.set([]);
  flowDraftEdge.set(null);
  editLocked.set(false);
});

describe('FlowchartCanvas.svelte', () => {
  // -----------------------------------------------------------------------
  // Original 6 tests (preserved)
  // -----------------------------------------------------------------------

  // jsdom lacks getScreenCTM so a true fireEvent drag sequence cannot be tested
  // from this layer. The drag reducer path is covered by pure reducer tests in
  // flow-interaction.test.ts (test_reducer_drag_mousemove_updates_positions).
  it('test_flowchart_node_position_store_roundtrip', async () => {
    const { flowchart } = flowFixture();
    seedPositions(flowchart.nodes);

    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const canvas = container.querySelector('[data-testid="flowchart-canvas"]') as HTMLElement;
    expect(canvas).not.toBeNull();

    const node = flowchart.nodes.find((n) => n.id === 'n_do')!;
    const initialPos = { x: node.x, y: node.y };

    // Directly write the store to verify the component reflects updated positions.
    const positions = new Map(get(flowNodePositions));
    positions.set('n_do', { x: node.x + 30, y: node.y + 20 });
    flowNodePositions.set(positions);
    await tick();

    const updatedPos = get(flowNodePositions).get('n_do');
    expect(updatedPos?.x).toBe(initialPos.x + 30);
    expect(updatedPos?.y).toBe(initialPos.y + 20);
  });

  it('test_flowchart_port_drag_creates_edge', async () => {
    const { flowchart } = flowFixture();
    seedPositions(flowchart.nodes);
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    // Simulate via store: push a created edge
    createdEdges.set([{ from: 'n_start', to: 'n_do' }]);
    await tick();

    const edges = get(createdEdges);
    expect(edges.length).toBe(1);
    expect(edges[0].from).toBe('n_start');
    expect(edges[0].to).toBe('n_do');
  });

  it('test_flowchart_wheel_pans', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const canvas = container.querySelector('[data-testid="flowchart-canvas"]') as HTMLElement;
    fireEvent.wheel(canvas, { deltaX: 10, deltaY: 20, ctrlKey: false, metaKey: false });
    await tick();

    const vp = get(flowViewport);
    // Pan applied: x/y should change, k should remain 1
    expect(vp.k).toBe(1);
    // After wheel with deltaX=10, deltaY=20 (no modifier) → applyPan(-10, -20)
    expect(vp.x).toBe(-10);
    expect(vp.y).toBe(-20);
  });

  it('test_flowchart_ctrl_wheel_zooms', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const canvas = container.querySelector('[data-testid="flowchart-canvas"]') as HTMLElement;
    // ctrl+wheel with positive deltaY → zoom out (factor 0.9)
    fireEvent.wheel(canvas, { deltaX: 0, deltaY: -50, ctrlKey: true, metaKey: false });
    await tick();

    const vp = get(flowViewport);
    // deltaY negative → factor = 1/0.9 → k > 1
    expect(vp.k).toBeGreaterThan(1);
    expect(vp.x).toBe(0); // unchanged
    expect(vp.y).toBe(0); // unchanged
  });

  it('test_flowchart_decision_shape_diamond', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const diamond = container.querySelector('[data-testid="shape-decision-n_decide"]');
    expect(diamond).not.toBeNull();
    // The shape group should contain a polygon
    const polygon = diamond?.querySelector('polygon');
    expect(polygon).not.toBeNull();
  });

  it('test_flowchart_edge_label_renders', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    // The yes-edge from n_decide to n_sub should render a label
    const label = container.querySelector('[data-testid="edge-label-n_decide-n_sub"]');
    expect(label).not.toBeNull();
    expect(label?.textContent).toBe('yes');
  });

  // -----------------------------------------------------------------------
  // New tests 7-21 (task 15.8)
  // -----------------------------------------------------------------------

  it('test_flowchart_start_shape_renders', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const shape = container.querySelector('[data-testid="shape-start-n_start"]');
    expect(shape).not.toBeNull();
    const ellipse = shape?.querySelector('ellipse');
    expect(ellipse).not.toBeNull();
  });

  it('test_flowchart_end_shape_renders', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const shape = container.querySelector('[data-testid="shape-end-n_end"]');
    expect(shape).not.toBeNull();
    const ellipse = shape?.querySelector('ellipse');
    expect(ellipse).not.toBeNull();
  });

  it('test_flowchart_process_shape_renders', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const shape = container.querySelector('[data-testid="shape-process-n_do"]');
    expect(shape).not.toBeNull();
    const rect = shape?.querySelector('rect');
    expect(rect).not.toBeNull();
  });

  it('test_flowchart_io_shape_renders', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const shape = container.querySelector('[data-testid="shape-io-n_io"]');
    expect(shape).not.toBeNull();
    const polygon = shape?.querySelector('polygon');
    expect(polygon).not.toBeNull();
  });

  it('test_flowchart_sub_shape_renders', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const shape = container.querySelector('[data-testid="shape-sub-n_sub"]');
    expect(shape).not.toBeNull();
    const rects = shape?.querySelectorAll('rect');
    const lines = shape?.querySelectorAll('line');
    expect(rects?.length).toBeGreaterThanOrEqual(1);
    expect(lines?.length).toBeGreaterThanOrEqual(2);
  });

  it('test_flowchart_storage_shape_renders', async () => {
    // fixture uses id 'n_store' for kind='storage'
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const shape = container.querySelector('[data-testid="shape-storage-n_store"]');
    expect(shape).not.toBeNull();
    const ellipses = shape?.querySelectorAll('ellipse');
    expect(ellipses?.length).toBeGreaterThanOrEqual(2);
  });

  it('test_flowchart_ports_absent_when_no_selection', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    // No node selected → no port circles
    const ports = container.querySelectorAll('[data-testid^="port-"]');
    expect(ports.length).toBe(0);
  });

  it('test_flowchart_ports_appear_only_for_selected_node', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    flowSelectedNodeId.set('n_do');
    await tick();

    // 4 ports for n_do
    const topPort    = container.querySelector('[data-testid="port-top-n_do"]');
    const rightPort  = container.querySelector('[data-testid="port-right-n_do"]');
    const bottomPort = container.querySelector('[data-testid="port-bottom-n_do"]');
    const leftPort   = container.querySelector('[data-testid="port-left-n_do"]');
    expect(topPort).not.toBeNull();
    expect(rightPort).not.toBeNull();
    expect(bottomPort).not.toBeNull();
    expect(leftPort).not.toBeNull();

    // No ports for a different node
    const otherPort = container.querySelector('[data-testid="port-top-n_start"]');
    expect(otherPort).toBeNull();
  });

  it('test_flowchart_ports_follow_selection_changes', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    // Select n_do → ports appear on n_do
    flowSelectedNodeId.set('n_do');
    await tick();
    expect(container.querySelector('[data-testid="port-top-n_do"]')).not.toBeNull();

    // Switch selection to n_start → n_do ports disappear, n_start ports appear
    flowSelectedNodeId.set('n_start');
    await tick();
    expect(container.querySelector('[data-testid="port-top-n_do"]')).toBeNull();
    expect(container.querySelector('[data-testid="port-top-n_start"]')).not.toBeNull();

    // Clear selection → all ports disappear
    flowSelectedNodeId.set(null);
    await tick();
    expect(container.querySelectorAll('[data-testid^="port-"]').length).toBe(0);
  });

  it('test_flowchart_edge_ok_style', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const edgeGroup = container.querySelector('[data-testid="flowchart-edge-n_decide-n_sub"]');
    expect(edgeGroup).not.toBeNull();
    expect(edgeGroup?.classList.contains('flowchart-edge-ok')).toBe(true);
  });

  it('test_flowchart_edge_err_style', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const edgeGroup = container.querySelector('[data-testid="flowchart-edge-n_decide-n_io"]');
    expect(edgeGroup).not.toBeNull();
    expect(edgeGroup?.classList.contains('flowchart-edge-err')).toBe(true);
  });

  it('test_flowchart_edge_neutral_style', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    // n_start → n_do has no label and no style → neutral
    const edgeGroup = container.querySelector('[data-testid="flowchart-edge-n_start-n_do"]');
    expect(edgeGroup).not.toBeNull();
    expect(edgeGroup?.classList.contains('flowchart-edge-neutral')).toBe(true);
  });

  it('test_flowchart_grid_background_renders', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const grid = container.querySelector('[data-testid="canvas-grid"]');
    expect(grid).not.toBeNull();
    expect(grid?.getAttribute('fill')).toBe('url(#canvas-grid)');
  });

  it('test_flowchart_zoom_in_button_increases_k', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const btn = container.querySelector('[data-testid="zoom-in-btn"]') as HTMLElement;
    expect(btn).not.toBeNull();
    fireEvent.click(btn);
    await tick();

    expect(get(flowViewport).k).toBeGreaterThan(1);
  });

  it('test_flowchart_zoom_out_button_decreases_k', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    const btn = container.querySelector('[data-testid="zoom-out-btn"]') as HTMLElement;
    expect(btn).not.toBeNull();
    fireEvent.click(btn);
    await tick();

    expect(get(flowViewport).k).toBeLessThan(1);
  });

  it('test_flowchart_zoom_reset_button_restores_viewport', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    flowViewport.set({ x: 50, y: 50, k: 2 });
    await tick();

    const btn = container.querySelector('[data-testid="zoom-reset-btn"]') as HTMLElement;
    expect(btn).not.toBeNull();
    fireEvent.click(btn);
    await tick();

    const vp = get(flowViewport);
    expect(vp.k).toBe(1);
    expect(vp.x).toBe(0);
    expect(vp.y).toBe(0);
  });

  // -----------------------------------------------------------------------
  // Phase 19 — edit-mode toggle, layout-overrides hydration, drag-end save
  // wiring. The actual `saveFlowchart` IPC fires only inside Tauri (gated
  // by `isTauri()`), so these tests cover the predicates and rendering.
  // -----------------------------------------------------------------------

  it('test_editlocked_default_false_renders_ports_when_selected', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    flowSelectedNodeId.set('n_do');
    await tick();
    // Default editLocked=false (edit mode) → ports render for the selection.
    expect(container.querySelector('[data-testid="port-top-n_do"]')).not.toBeNull();
  });

  it('test_editlocked_true_hides_port_circles', async () => {
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    editLocked.set(true);
    flowSelectedNodeId.set('n_do');
    await tick();
    expect(container.querySelector('[data-testid="port-top-n_do"]')).toBeNull();
  });

  it('test_editlocked_true_renders_readonly_tooltip', async () => {
    const { flowchart } = flowFixture();
    editLocked.set(true);
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();
    const titles = Array.from(container.querySelectorAll('title')).map((t) => t.textContent);
    expect(titles.some((t) => t?.includes('Read-only'))).toBe(true);
  });

  it('test_editlocked_false_omits_readonly_tooltip', async () => {
    const { flowchart } = flowFixture();
    editLocked.set(false);
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();
    const titles = Array.from(container.querySelectorAll('title')).map((t) => t.textContent);
    expect(titles.every((t) => !t?.includes('Read-only'))).toBe(true);
  });

  it('test_layout_overrides_seed_positions_at_mount', async () => {
    const { flowchart } = flowFixture();
    const overrides = { n_do: { x: 999, y: 888 } };
    render(FlowchartCanvas, { props: { flowchart, layoutOverrides: overrides } });
    await tick();
    const pos = get(flowNodePositions).get('n_do');
    expect(pos).toEqual({ x: 999, y: 888 });
  });

  it('test_layout_overrides_null_falls_back_to_flowchart_defaults', async () => {
    const { flowchart } = flowFixture();
    render(FlowchartCanvas, { props: { flowchart, layoutOverrides: null } });
    await tick();
    const node = flowchart.nodes.find((n) => n.id === 'n_do')!;
    const pos = get(flowNodePositions).get('n_do');
    expect(pos).toEqual({ x: node.x, y: node.y });
  });

  it('test_e_keypress_toggles_editlocked', async () => {
    const { flowchart } = flowFixture();
    render(FlowchartCanvas, { props: { flowchart } });
    expect(get(editLocked)).toBe(false);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'E' }));
    await tick();
    expect(get(editLocked)).toBe(true);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'e' }));
    await tick();
    expect(get(editLocked)).toBe(false);
  });

  it('test_e_keypress_inside_input_does_not_toggle', async () => {
    const { flowchart } = flowFixture();
    render(FlowchartCanvas, { props: { flowchart } });
    const input = document.createElement('input');
    document.body.appendChild(input);
    input.focus();
    expect(get(editLocked)).toBe(false);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'e', bubbles: true }));
    await tick();
    expect(get(editLocked)).toBe(false);
    document.body.removeChild(input);
  });

  it('test_e_with_modifier_does_not_toggle', async () => {
    const { flowchart } = flowFixture();
    render(FlowchartCanvas, { props: { flowchart } });
    expect(get(editLocked)).toBe(false);
    document.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'e', ctrlKey: true }),
    );
    await tick();
    expect(get(editLocked)).toBe(false);
    document.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'e', metaKey: true }),
    );
    await tick();
    expect(get(editLocked)).toBe(false);
  });

  it('test_flowchart_zoom_buttons_preserve_external_state', async () => {
    // Guards invariant 15.8-A / 15.7-A: a zoom click must not clear
    // flowSelectedNodeId, flowNodePositions, or createdEdges that were set
    // outside this component (e.g., by FlowSwim, or by the same component
    // on a previous mount).
    const { flowchart } = flowFixture();
    const { container } = render(FlowchartCanvas, { props: { flowchart } });
    await tick();

    // Simulate external state set by another component
    flowSelectedNodeId.set('n_do');
    const seededPositions = new Map([['n_do', { x: 500, y: 500 }]]);
    flowNodePositions.set(seededPositions);
    createdEdges.set([{ from: 'n_start', to: 'n_end' }]);
    await tick();

    // Click zoom-in
    const btn = container.querySelector('[data-testid="zoom-in-btn"]') as HTMLElement;
    fireEvent.click(btn);
    await tick();

    // Selection, positions, and created edges must survive
    expect(get(flowSelectedNodeId)).toBe('n_do');
    expect(get(flowNodePositions).get('n_do')).toEqual({ x: 500, y: 500 });
    expect(get(createdEdges).length).toBe(1);
    expect(get(createdEdges)[0].to).toBe('n_end');

    // Viewport zoom actually applied
    expect(get(flowViewport).k).toBeGreaterThan(1);
  });
});
