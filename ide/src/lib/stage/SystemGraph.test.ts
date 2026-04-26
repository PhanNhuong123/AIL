import { describe, test, expect, beforeEach, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { activeLens, selection, path } from '$lib/stores';
import SystemGraph from './SystemGraph.svelte';
import { multiClusterFixture } from './fixtures';
import type { GraphJson } from '$lib/types';

vi.mock('$lib/chrome/toolbar-state', async () => {
  const actual = await vi.importActual<Record<string, unknown>>('$lib/chrome/toolbar-state');
  return { ...actual, navigateTo: vi.fn() };
});

import { navigateTo } from '$lib/chrome/toolbar-state';

beforeEach(() => {
  selection.set({ kind: 'none', id: null });
  activeLens.set('verify');
  path.set([]);
  vi.clearAllMocks();
});

test('renders the system-graph root testid', () => {
  const { container } = render(SystemGraph, { props: { graph: multiClusterFixture() } });
  expect(container.querySelector('[data-testid="system-graph"]')).not.toBeNull();
});

test('renders one module group per module in the graph', () => {
  const g: GraphJson = {
    project: { id: 'p', name: 'p', description: '', nodeCount: 3, moduleCount: 3, fnCount: 0, status: 'ok' },
    clusters: [],
    modules: [
      { id: 'module:m_a', name: 'm_a', description: '', cluster: '', clusterName: '', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] },
      { id: 'module:m_b', name: 'm_b', description: '', cluster: '', clusterName: '', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] },
      { id: 'module:m_c', name: 'm_c', description: '', cluster: '', clusterName: '', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] },
    ],
    externals: [],
    relations: [],
    types: [],
    errors: [],
    issues: [],
    detail: {},
  };
  const { container } = render(SystemGraph, { props: { graph: g } });
  expect(container.querySelector('[data-testid="sg-module-module:m_a"]')).not.toBeNull();
  expect(container.querySelector('[data-testid="sg-module-module:m_b"]')).not.toBeNull();
  expect(container.querySelector('[data-testid="sg-module-module:m_c"]')).not.toBeNull();
});

test('hover dims other modules', async () => {
  const g: GraphJson = {
    project: { id: 'p', name: 'p', description: '', nodeCount: 2, moduleCount: 2, fnCount: 0, status: 'ok' },
    clusters: [],
    modules: [
      { id: 'module:m_a', name: 'm_a', description: '', cluster: '', clusterName: '', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] },
      { id: 'module:m_b', name: 'm_b', description: '', cluster: '', clusterName: '', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] },
    ],
    externals: [],
    relations: [],
    types: [],
    errors: [],
    issues: [],
    detail: {},
  };
  const { container } = render(SystemGraph, { props: { graph: g } });
  const aGroup = container.querySelector('[data-testid="sg-module-module:m_a"]') as SVGGElement;
  await fireEvent.mouseEnter(aGroup);
  await tick();
  const bCircle = container.querySelector('[data-testid="sg-module-module:m_b"] circle.sg-module-circle') as SVGCircleElement;
  expect(bCircle.classList.contains('sg-module-dim')).toBe(true);
});

test('clicking a module calls navigateTo with module kind and level 1', async () => {
  const g: GraphJson = {
    project: { id: 'p', name: 'p', description: '', nodeCount: 1, moduleCount: 1, fnCount: 0, status: 'ok' },
    clusters: [],
    modules: [
      { id: 'module:m_a', name: 'm_a', description: '', cluster: '', clusterName: '', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] },
    ],
    externals: [],
    relations: [],
    types: [],
    errors: [],
    issues: [],
    detail: {},
  };
  const { container } = render(SystemGraph, { props: { graph: g } });
  const aGroup = container.querySelector('[data-testid="sg-module-module:m_a"]') as SVGGElement;
  await fireEvent.click(aGroup);
  expect(navigateTo).toHaveBeenCalledWith(['module:m_a'], 'module', 'module:m_a', 1);
});

test('clears hoveredId when the hovered module is removed by a graph patch', async () => {
  const makeGraph = (moduleIds: string[]): GraphJson => ({
    project: { id: 'p', name: 'p', description: '', nodeCount: moduleIds.length, moduleCount: moduleIds.length, fnCount: 0, status: 'ok' as const },
    clusters: [],
    modules: moduleIds.map((id) => ({
      id,
      name: id,
      description: '',
      cluster: '',
      clusterName: '',
      clusterColor: '#fff',
      status: 'ok' as const,
      nodeCount: 1,
      functions: [],
    })),
    externals: [],
    relations: [],
    types: [],
    errors: [],
    issues: [],
    detail: {},
  });
  const initial = makeGraph(['module:m_a', 'module:m_b']);
  const { container, rerender } = render(SystemGraph, { props: { graph: initial } });
  const aGroup = container.querySelector('[data-testid="sg-module-module:m_a"]') as SVGGElement;
  await fireEvent.mouseEnter(aGroup);
  await tick();
  // m_b should be dimmed while m_a is hovered
  let bCircle = container.querySelector('[data-testid="sg-module-module:m_b"] circle.sg-module-circle') as SVGCircleElement;
  expect(bCircle.classList.contains('sg-module-dim')).toBe(true);
  // Patch: remove m_a (the hovered module)
  const patched = makeGraph(['module:m_b']);
  await rerender({ graph: patched });
  await tick();
  // m_b should no longer be dimmed — hoveredId was cleared reactively
  bCircle = container.querySelector('[data-testid="sg-module-module:m_b"] circle.sg-module-circle') as SVGCircleElement;
  expect(bCircle.classList.contains('sg-module-dim')).toBe(false);
});

describe('legend text changes with active lens', () => {
  test('shows Verify for verify lens and Data for data lens', async () => {
    const g: GraphJson = {
      project: { id: 'p', name: 'p', description: '', nodeCount: 0, moduleCount: 0, fnCount: 0, status: 'ok' },
      clusters: [],
      modules: [],
      externals: [],
      relations: [],
      types: [],
      errors: [],
      issues: [],
      detail: {},
    };
    activeLens.set('verify');
    const { container, rerender } = render(SystemGraph, { props: { graph: g } });
    let legend = container.querySelector('[data-testid="sg-legend"]')?.textContent ?? '';
    expect(legend).toMatch(/Verify/);

    activeLens.set('data');
    await rerender({ graph: g });
    legend = container.querySelector('[data-testid="sg-legend"]')?.textContent ?? '';
    expect(legend).toMatch(/Data/);
  });
});
