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

import { render, fireEvent } from '@testing-library/svelte';
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
  chatPreviewCards,
  chatDraft,
  chatMode,
  isAgentRunning,
  currentRunId,
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

describe('+page.svelte — task 16.1 agent listener wiring', () => {
  it('subscribes to agent-step, agent-message, agent-complete on mount', async () => {
    render(Page);
    await tick();
    expect(listeners.has('agent-step')).toBe(true);
    expect(listeners.has('agent-message')).toBe(true);
    expect(listeners.has('agent-complete')).toBe(true);
  });

  it('test_agent_step_guarded_by_current_run_id', async () => {
    // 16.1-B layer 4: payload.runId !== currentRunId → no mutation.
    render(Page);
    await tick();
    currentRunId.set('r-42');
    const before = get(chatMessages).length;

    const stepHandler = listeners.get('agent-step')!;
    stepHandler({
      payload: { runId: 'r-99', index: 1, phase: 'plan', text: 'stale' },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();
    expect(get(chatMessages).length).toBe(before);

    // Matching runId MUST mutate.
    stepHandler({
      payload: { runId: 'r-42', index: 1, phase: 'plan', text: 'ok' },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();
    expect(get(chatMessages).length).toBe(before + 1);
    const msgs = get(chatMessages);
    expect(msgs[msgs.length - 1].text).toBe('ok');
  });

  it('test_agent_message_appends_message_and_preview_when_run_matches', async () => {
    render(Page);
    await tick();
    currentRunId.set('r-7');
    const msgBefore = get(chatMessages).length;
    const cardsBefore = get(chatPreviewCards).length;

    const msgHandler = listeners.get('agent-message')!;
    msgHandler({
      payload: {
        runId: 'r-7',
        messageId: 'm-1',
        text: 'Here is the plan',
        preview: {
          title: 'Add rate limiter',
          summary: 'Adds one step',
          patch: emptyPatch(),
        },
      },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();

    expect(get(chatMessages).length).toBe(msgBefore + 1);
    const msgs = get(chatMessages);
    expect(msgs[msgs.length - 1].text).toBe('Here is the plan');
    expect(get(chatPreviewCards).length).toBe(cardsBefore + 1);
    const cards = get(chatPreviewCards);
    const last = cards[cards.length - 1];
    expect(last.id).toBe('m-1');
    expect(last.runId).toBe('r-7');
    expect(last.patch).toBeTruthy();
  });

  it('test_agent_complete_clears_running_state_when_matching', async () => {
    render(Page);
    await tick();
    isAgentRunning.set(true);
    currentRunId.set('r-done');

    const doneHandler = listeners.get('agent-complete')!;
    doneHandler({
      payload: { runId: 'r-done', status: 'done' },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();

    expect(get(isAgentRunning)).toBe(false);
    expect(get(currentRunId)).toBeNull();
  });

  it('test_stale_event_after_cancel_rejected_by_run_id_guard', async () => {
    // Explicit coverage of the B-6 red-team gap: after cancel clears the
    // currentRunId, a late step event from the cancelled run MUST NOT
    // mutate chat state.
    render(Page);
    await tick();
    currentRunId.set('r-k');
    isAgentRunning.set(true);

    // Simulate cancel completion first.
    const doneHandler = listeners.get('agent-complete')!;
    doneHandler({
      payload: { runId: 'r-k', status: 'cancelled' },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();
    expect(get(currentRunId)).toBeNull();

    // Now fire a late step from the same (now-cancelled) run.
    const msgBefore = get(chatMessages).length;
    const stepHandler = listeners.get('agent-step')!;
    stepHandler({
      payload: { runId: 'r-k', index: 99, phase: 'plan', text: 'LATE' },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();
    expect(get(chatMessages).length).toBe(msgBefore);
  });

  it('test_preview_apply_calls_applygraphpatch_and_removes_card', async () => {
    // E2E for invariant 16.1-C: an agent-produced preview card carries a
    // patch that adds a module. Clicking Confirm in ChatPanel dispatches
    // `previewapply`, the +page.svelte handler applies the patch and
    // removes the card.
    graph.set(emptyGraph());
    const { container } = render(Page);
    await tick();
    currentRunId.set('r-p');

    const msgHandler = listeners.get('agent-message')!;
    const patch: GraphPatchJson = {
      ...emptyPatch(),
      modulesAdded: [
        {
          id: 'm-applied',
          name: 'applied',
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
    msgHandler({
      payload: {
        runId: 'r-p',
        messageId: 'm-prev',
        text: 'proposed',
        preview: { title: 'Add module', summary: 's', patch },
      },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();

    expect(get(chatPreviewCards).find((c) => c.id === 'm-prev')).toBeDefined();

    // The agent-message handler appended the new card AFTER the seed card.
    // Click the LAST Confirm button so we operate on the patch-bearing card.
    const confirmBtns = container.querySelectorAll('[data-testid="chat-preview-confirm"]');
    expect(confirmBtns.length).toBeGreaterThanOrEqual(2);
    fireEvent.click(confirmBtns[confirmBtns.length - 1] as HTMLButtonElement);
    await tick();

    expect(get(chatPreviewCards).find((c) => c.id === 'm-prev')).toBeUndefined();
    const g = get(graph);
    expect(g?.modules.some((m) => m.id === 'm-applied')).toBe(true);
  });

  it('test_preview_dismiss_removes_card_without_graph_mutation', async () => {
    graph.set(emptyGraph());
    const { container } = render(Page);
    await tick();
    currentRunId.set('r-d');

    const msgHandler = listeners.get('agent-message')!;
    msgHandler({
      payload: {
        runId: 'r-d',
        messageId: 'm-x',
        text: 'proposed',
        preview: { title: 't', summary: 's' },
      },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();
    const modulesBefore = [...(get(graph)?.modules ?? [])];

    // Dismiss the LAST card (the one we just added).
    const discardBtns = container.querySelectorAll('[data-testid="chat-preview-discard"]');
    expect(discardBtns.length).toBeGreaterThanOrEqual(2);
    fireEvent.click(discardBtns[discardBtns.length - 1] as HTMLButtonElement);
    await tick();

    expect(get(chatPreviewCards).find((c) => c.id === 'm-x')).toBeUndefined();
    expect(get(graph)?.modules).toEqual(modulesBefore);
  });

  it('test_early_event_buffered_then_replayed_when_currentRunId_resolves', async () => {
    // HIGH #1 mitigation: agent events that arrive BEFORE `runAgent()`
    // resolves on the JS side (and thus before `currentRunId` is set) MUST
    // be buffered keyed by runId, then replayed when `currentRunId`
    // becomes that id. Otherwise early events would be silently dropped.
    render(Page);
    await tick();
    expect(get(currentRunId)).toBeNull();
    const beforeMsgs = get(chatMessages).length;

    // Simulate an early step event for a run the frontend has not yet
    // learned about.
    const stepHandler = listeners.get('agent-step')!;
    stepHandler({
      payload: { runId: 'r-early', index: 1, phase: 'plan', text: 'EARLY' },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();
    // Not applied yet — currentRunId is null.
    expect(get(chatMessages).length).toBe(beforeMsgs);

    // Now `runAgent()` resolves and the frontend learns the runId.
    currentRunId.set('r-early');
    await tick();
    // Buffered event has been replayed.
    expect(get(chatMessages).length).toBe(beforeMsgs + 1);
    const msgs = get(chatMessages);
    expect(msgs[msgs.length - 1].text).toBe('EARLY');
  });

  it('test_all_listeners_unlisten_on_destroy', async () => {
    const { unmount } = render(Page);
    await tick();
    // Ensure all four were captured by the mock.
    expect(unlistenFns.has('graph-updated')).toBe(true);
    expect(unlistenFns.has('agent-step')).toBe(true);
    expect(unlistenFns.has('agent-message')).toBe(true);
    expect(unlistenFns.has('agent-complete')).toBe(true);

    unmount();
    await tick();

    for (const [, fn] of unlistenFns) {
      expect(fn).toHaveBeenCalled();
    }
  });
});
