import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, selection, path, history } from '$lib/stores';
import { zoomLevel } from '$lib/chrome/toolbar-state';
import { flowMode, createdEdges, flowNodePositions, flowViewport } from './flow-state';
import { flowFixture } from './fixtures';
import FlowView from './FlowView.svelte';

beforeEach(() => {
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  path.set([]);
  history.set({ back: [], forward: [] });
  zoomLevel.set(2);
  flowMode.set('Swim');
  createdEdges.set([]);
  flowNodePositions.set(new Map());
  flowViewport.set({ x: 0, y: 0, k: 1 });
});

describe('FlowView.svelte', () => {
  it('test_flow_view_swim_default', async () => {
    const { flowchart, graph: g } = flowFixture();
    graph.set(g);

    const { container } = render(FlowView, {
      props: {
        fn: g.modules[0].functions[0],
        flowchart,
        detail: null,
      },
    });
    await tick();

    // Swim button pressed by default
    const swimBtn = container.querySelector('[data-testid="flow-mode-btn-swim"]');
    expect(swimBtn).not.toBeNull();
    expect(swimBtn?.getAttribute('aria-pressed')).toBe('true');

    // Flow swim rendered
    expect(container.querySelector('[data-testid="flow-swim"]')).not.toBeNull();
  });

  it('test_mode_switch_preserves_selection', async () => {
    const { flowchart, graph: g } = flowFixture();
    graph.set(g);
    selection.set({ kind: 'function', id: 'function:fn_demo' });

    const { container } = render(FlowView, {
      props: {
        fn: g.modules[0].functions[0],
        flowchart,
        detail: null,
      },
    });
    await tick();

    const selBefore = get(selection);

    // Switch to Flowchart
    const fcBtn = container.querySelector('[data-testid="flow-mode-btn-flowchart"]') as HTMLButtonElement;
    fireEvent.click(fcBtn);
    await tick();
    expect(get(flowMode)).toBe('Flowchart');

    // Switch back to Swim
    const swimBtn = container.querySelector('[data-testid="flow-mode-btn-swim"]') as HTMLButtonElement;
    fireEvent.click(swimBtn);
    await tick();
    expect(get(flowMode)).toBe('Swim');

    // Selection unchanged
    expect(get(selection).id).toBe(selBefore.id);
    expect(get(selection).kind).toBe(selBefore.kind);
  });

  it('test_code_mode_renders_flow_code', async () => {
    const { flowchart, graph: g } = flowFixture();
    graph.set(g);

    const { container } = render(FlowView, {
      props: {
        fn: g.modules[0].functions[0],
        flowchart,
        detail: null,
      },
    });
    await tick();

    const codeBtn = container.querySelector('[data-testid="flow-mode-btn-code"]') as HTMLButtonElement;
    fireEvent.click(codeBtn);
    await tick();

    expect(get(flowMode)).toBe('Code');
    expect(container.querySelector('[data-testid="flow-code"]')).not.toBeNull();
  });
});
