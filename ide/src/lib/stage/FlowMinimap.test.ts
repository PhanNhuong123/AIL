import { describe, test, expect, beforeEach, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { path } from '$lib/stores';
import FlowMinimap from './FlowMinimap.svelte';

vi.mock('$lib/chrome/toolbar-state', async () => {
  const actual = await vi.importActual<any>('$lib/chrome/toolbar-state');
  return { ...actual, navigateTo: vi.fn() };
});

import { navigateTo } from '$lib/chrome/toolbar-state';

const sampleFlowchart = {
  nodes: [
    { id: 'n_a', label: 'Step A' } as any,
    { id: 'n_b', label: 'Step B' } as any,
    { id: 'n_c', label: 'Step C' } as any,
  ],
  edges: [],
} as any;

beforeEach(() => {
  vi.clearAllMocks();
  path.set([]);
});

test('renders one dot per flowchart node', () => {
  const { container } = render(FlowMinimap, { props: { flowchart: sampleFlowchart } });
  expect(container.querySelector('[data-testid="mm-dot-n_a"]')).not.toBeNull();
  expect(container.querySelector('[data-testid="mm-dot-n_b"]')).not.toBeNull();
  expect(container.querySelector('[data-testid="mm-dot-n_c"]')).not.toBeNull();
});

test('selected dot has mm-dot-selected class', () => {
  const { container } = render(FlowMinimap, { props: { flowchart: sampleFlowchart, selectedId: 'n_b' } });
  const sel = container.querySelector('[data-testid="mm-dot-n_b"]');
  expect(sel?.classList.contains('mm-dot-selected')).toBe(true);
  const other = container.querySelector('[data-testid="mm-dot-n_a"]');
  expect(other?.classList.contains('mm-dot-selected')).toBe(false);
});

test('clicking a dot calls navigateTo with step path and level 4', async () => {
  path.set(['project:root', 'module:m_billing', 'function:transfer']);
  const { container } = render(FlowMinimap, { props: { flowchart: sampleFlowchart } });
  const dot = container.querySelector('[data-testid="mm-dot-n_b"]') as HTMLButtonElement;
  await fireEvent.click(dot);
  expect(navigateTo).toHaveBeenCalledWith(
    ['project:root', 'module:m_billing', 'function:transfer', 'step:n_b'],
    'step',
    'step:n_b',
    4,
  );
});

test('path format matches FlowSwim convention (step: prefix)', async () => {
  const { container } = render(FlowMinimap, { props: { flowchart: sampleFlowchart } });
  const dot = container.querySelector('[data-testid="mm-dot-n_a"]') as HTMLButtonElement;
  await fireEvent.click(dot);
  const call = (navigateTo as any).mock.calls[0];
  // Last path segment must be "step:" + id (mirrors FlowSwim handleNodeClick)
  const lastSegment = call[0][call[0].length - 1];
  expect(lastSegment).toBe('step:n_a');
  // The third arg (id) is also the full "step:id" string, matching FlowSwim
  expect(call[2]).toBe('step:n_a');
  expect(call[3]).toBe(4);
});
