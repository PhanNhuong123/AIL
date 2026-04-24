import { describe, it, expect, beforeEach, vi } from 'vitest';

const invoke = vi.fn();
// Capture listeners registered by onMount so tests can dispatch synthetic
// `graph-updated` events and assert the handler's behavior.
type PatchHandler = (payload: unknown) => void;
const listeners = new Map<string, PatchHandler>();
const unlistenFns = new Map<string, ReturnType<typeof vi.fn>>();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, cb: PatchHandler) => {
    listeners.set(event, cb);
    const un = vi.fn();
    unlistenFns.set(event, un);
    return Promise.resolve(un);
  }),
}));

import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import {
  activeLens,
  overlays,
  graph,
  selection,
  quickCreateModalOpen,
  welcomeModalOpen,
  tweaksPanelOpen,
} from '$lib/stores';
import type { Lens } from '$lib/stores';
import type { GraphJson, GraphPatchJson } from '$lib/types';
import {
  chatMessages,
  chatDraft,
  chatMode,
  resetChatState,
} from '$lib/chat/chat-state';
import {
  nodeViewActiveTab,
  nodeCodeLang,
} from '$lib/stage/node-view-state';
import Page from './+page.svelte';

function emptyGraph(projectId = 'p1'): GraphJson {
  return {
    project: {
      id: projectId,
      name: 'Proj',
      description: '',
      nodeCount: 0,
      moduleCount: 0,
      fnCount: 0,
      status: 'ok',
    },
    clusters: [],
    modules: [],
    externals: [],
    relations: [],
    types: [],
    errors: [],
    issues: [],
    detail: {},
  };
}

function emptyPatch(): GraphPatchJson {
  return {
    modulesAdded: [],
    modulesModified: [],
    modulesRemoved: [],
    functionsAdded: [],
    functionsModified: [],
    functionsRemoved: [],
    stepsAdded: [],
    stepsModified: [],
    stepsRemoved: [],
    timestamp: 0,
  };
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
  listeners.clear();
  unlistenFns.clear();
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  activeLens.set('verify');
  overlays.set({
    rules: false,
    verification: true,
    dataflow: false,
    dependencies: false,
    tests: false,
  });
  quickCreateModalOpen.set(false);
  welcomeModalOpen.set(false);
  tweaksPanelOpen.set(false);
  resetChatState();
  nodeViewActiveTab.set('code');
  nodeCodeLang.set('python');
});

describe('+page.svelte — canonical 3-column layout', () => {
  it('test_root_mounts_three_regions', () => {
    const { container } = render(Page);

    expect(container.querySelector('[data-testid="region-titlebar"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="region-outline"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="region-stage"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="chat-panel"]')).not.toBeNull();
  });

  it('test_removed_components_absent_from_shell', () => {
    const { container } = render(Page);

    expect(container.querySelector('.region-toolbar')).toBeNull();
    expect(container.querySelector('[data-testid="context-panel"]')).toBeNull();
    expect(container.querySelector('[data-testid="bottom-panel"]')).toBeNull();
  });

  it('test_app_root_has_no_bottom_collapsed_class', () => {
    const { container } = render(Page);
    const root = container.querySelector('[data-testid="app-root"]');
    expect(root).not.toBeNull();
    expect(root?.classList.contains('bottom-collapsed')).toBe(false);
  });

  it('test_activeLens_store_defaults_to_verify', () => {
    render(Page);
    expect(get(activeLens)).toBe('verify' satisfies Lens);
  });

  it('test_overlays_store_preserved_unchanged', () => {
    render(Page);
    const o = get(overlays);
    expect(o.rules).toBe(false);
    expect(o.verification).toBe(true);
    expect(o.dataflow).toBe(false);
    expect(o.dependencies).toBe(false);
    expect(o.tests).toBe(false);
  });
});

describe('+page.svelte — task 15.11 watcher subscription', () => {
  it('subscribes to graph-updated on mount', async () => {
    render(Page);
    await tick();
    expect(listeners.has('graph-updated')).toBe(true);
  });

  it('graph-updated patch handler applies the patch to $graph', async () => {
    graph.set(emptyGraph());
    render(Page);
    await tick();

    const handler = listeners.get('graph-updated');
    expect(handler).toBeDefined();

    const patch: GraphPatchJson = {
      ...emptyPatch(),
      modulesAdded: [
        {
          id: 'm-new',
          name: 'new',
          description: '',
          cluster: 'default',
          clusterName: 'default',
          clusterColor: '#2997ff',
          status: 'ok',
          nodeCount: 1,
          functions: [],
        },
      ],
    };
    handler!({ payload: patch } as unknown as Parameters<PatchHandler>[0]);

    const g = get(graph);
    expect(g?.modules.map((m) => m.id)).toEqual(['m-new']);
  });

  it('invariant 15.11-C: patch apply preserves non-graph/non-selection stores', async () => {
    graph.set(emptyGraph());
    activeLens.set('rules');
    chatDraft.set('my draft');
    chatMode.set('ask');
    nodeViewActiveTab.set('proof');
    nodeCodeLang.set('typescript');
    const beforeChatCount = get(chatMessages).length;

    render(Page);
    await tick();
    const handler = listeners.get('graph-updated')!;
    handler({ payload: emptyPatch() } as unknown as Parameters<PatchHandler>[0]);

    expect(get(activeLens)).toBe('rules');
    expect(get(chatDraft)).toBe('my draft');
    expect(get(chatMode)).toBe('ask');
    expect(get(nodeViewActiveTab)).toBe('proof');
    expect(get(nodeCodeLang)).toBe('typescript');
    expect(get(chatMessages).length).toBe(beforeChatCount);
  });

  it('reconciles selection when current node is removed by the patch', async () => {
    const g = emptyGraph();
    g.modules.push({
      id: 'm1',
      name: 'm1',
      description: '',
      cluster: 'default',
      clusterName: 'default',
      clusterColor: '#2997ff',
      status: 'ok',
      nodeCount: 1,
      functions: [],
    });
    graph.set(g);
    selection.set({ kind: 'module', id: 'm1' });

    render(Page);
    await tick();
    const handler = listeners.get('graph-updated')!;
    handler({
      payload: { ...emptyPatch(), modulesRemoved: ['m1'] },
    } as unknown as Parameters<PatchHandler>[0]);

    expect(get(selection)).toEqual({ kind: 'project', id: 'p1' });
  });

  it('starts watcher once per project id and not again for the same id', async () => {
    render(Page);
    await tick();
    // Trigger the reactive block by publishing a graph.
    graph.set(emptyGraph('p-xyz'));
    await tick();
    expect(invoke).toHaveBeenCalledWith('start_watch_project');
    const firstCalls = invoke.mock.calls.filter(
      (c) => c[0] === 'start_watch_project',
    ).length;

    // Republish same graph (same project id) — should NOT re-fire.
    graph.set({ ...emptyGraph('p-xyz') });
    await tick();
    const secondCalls = invoke.mock.calls.filter(
      (c) => c[0] === 'start_watch_project',
    ).length;
    expect(secondCalls).toBe(firstCalls);
  });

  it('starts a new watcher when the project id changes (re-load)', async () => {
    render(Page);
    await tick();
    graph.set(emptyGraph('p-a'));
    await tick();
    graph.set(emptyGraph('p-b'));
    await tick();
    const calls = invoke.mock.calls.filter(
      (c) => c[0] === 'start_watch_project',
    );
    expect(calls.length).toBe(2);
  });
});
