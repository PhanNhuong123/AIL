import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, selection, path, history, activeLens } from '$lib/stores';
import { zoomLevel } from '$lib/chrome/toolbar-state';
import { flowMode, flowSelectedNodeId, flowFocusedNodeId } from './flow-state';
import { flowFixture } from './fixtures';
import FlowSwim from './FlowSwim.svelte';

beforeEach(() => {
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  path.set([]);
  history.set({ back: [], forward: [] });
  zoomLevel.set(2);
  flowMode.set('Swim');
  flowSelectedNodeId.set(null);
  flowFocusedNodeId.set(null);
  activeLens.set('verify');
});

describe('FlowSwim.svelte', () => {
  it('test_swim_renders_all_nodes', async () => {
    const { flowchart, graph: g } = flowFixture();
    graph.set(g);
    const { container } = render(FlowSwim, { props: { flowchart, fn: g.modules[0].functions[0] } });
    await tick();
    for (const n of flowchart.nodes) {
      expect(container.querySelector(`[data-testid="swim-node-${n.id}"]`)).not.toBeNull();
    }
  });

  it('test_swim_branch_edge_has_testid', async () => {
    const { flowchart, graph: g } = flowFixture();
    graph.set(g);
    const { container } = render(FlowSwim, { props: { flowchart, fn: g.modules[0].functions[0] } });
    await tick();
    expect(container.querySelector('[data-testid="swim-branch-edge-yes"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="swim-branch-edge-no"]')).not.toBeNull();
  });

  it('test_swim_focus_dims_non_neighbors', async () => {
    const { flowchart, graph: g } = flowFixture();
    graph.set(g);
    flowFocusedNodeId.set('n_decide');
    const { container } = render(FlowSwim, { props: { flowchart, fn: g.modules[0].functions[0] } });
    await tick();
    // n_start is not connected to n_decide (path: n_start -> n_do -> n_decide)
    const startNode = container.querySelector('[data-testid="swim-node-n_start"]');
    expect(startNode?.classList.contains('swim-node-dimmed')).toBe(true);
    // n_do is a direct neighbor of n_decide (edge n_do -> n_decide)
    const doNode = container.querySelector('[data-testid="swim-node-n_do"]');
    expect(doNode?.classList.contains('swim-node-dimmed')).toBe(false);
    // Out-neighbors of n_decide (edges n_decide -> n_sub, n_decide -> n_io)
    const subNode = container.querySelector('[data-testid="swim-node-n_sub"]');
    expect(subNode?.classList.contains('swim-node-dimmed')).toBe(false);
    const ioNode = container.querySelector('[data-testid="swim-node-n_io"]');
    expect(ioNode?.classList.contains('swim-node-dimmed')).toBe(false);
    // Focused node itself is not dimmed
    const decideNode = container.querySelector('[data-testid="swim-node-n_decide"]');
    expect(decideNode?.classList.contains('swim-node-dimmed')).toBe(false);
  });

  it('test_swim_no_dimming_when_focus_null', async () => {
    const { flowchart, graph: g } = flowFixture();
    graph.set(g);
    flowFocusedNodeId.set(null);
    const { container } = render(FlowSwim, { props: { flowchart, fn: g.modules[0].functions[0] } });
    await tick();
    const dimmed = container.querySelectorAll('.swim-node-dimmed');
    expect(dimmed.length).toBe(0);
  });

  it('test_swim_lens_hint_verify_shows_status', async () => {
    const { flowchart, graph: g } = flowFixture();
    graph.set(g);
    activeLens.set('verify');
    const { container } = render(FlowSwim, { props: { flowchart, fn: g.modules[0].functions[0] } });
    await tick();
    // fixture: n_do status=ok, n_decide status=warn, n_io status=fail
    const doText = container.querySelector('[data-testid="swim-node-n_do"]')?.textContent ?? '';
    const decideText = container.querySelector('[data-testid="swim-node-n_decide"]')?.textContent ?? '';
    const ioText = container.querySelector('[data-testid="swim-node-n_io"]')?.textContent ?? '';
    expect(doText).toContain('✓');
    expect(decideText).toContain('⚠');
    expect(ioText).toContain('✗');
  });
});
