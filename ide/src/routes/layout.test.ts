import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';

const invoke = vi.fn();
// Capture listeners registered by onMount so tests can dispatch synthetic
// `graph-updated` events and assert the handler's behavior.
type PatchHandler = (payload: unknown) => void;
const listeners = new Map<string, PatchHandler>();
const unlistenFns = new Map<string, ReturnType<typeof vi.fn>>();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
  isTauri: () => 'isTauri' in window && (window as Window & { isTauri?: boolean }).isTauri === true,
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
import { sidebarActiveTab, resetSidebarState } from '$lib/chat/sidebar-state';
import {
  nodeViewActiveTab,
  nodeCodeLang,
} from '$lib/stage/node-view-state';
import { patchEffects, clearPatchEffects } from '$lib/patch-effects';
import { isVerifyRunning, currentVerifyRunId, verifyTick, resetVerifyState } from '$lib/verify/verify-state';
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
  resetSidebarState();
  resetVerifyState();
  sidebarActiveTab.set('chat');
  nodeViewActiveTab.set('code');
  nodeCodeLang.set('python');
});

describe('+page.svelte — canonical 3-column layout', () => {
  it('test_root_mounts_three_regions', () => {
    const { container } = render(Page);

    expect(container.querySelector('[data-testid="region-titlebar"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="region-outline"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="region-stage"]')).not.toBeNull();
    // 15.12-A: ChatPanel is mounted inside RightSidebar, not directly in the grid.
    const sidebar = container.querySelector('[data-testid="right-sidebar"]');
    expect(sidebar).not.toBeNull();
    expect(sidebar!.querySelector('[data-testid="chat-panel"]')).not.toBeNull();
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
    // 16.2: handler now flows through a 250 ms debounce buffer; we advance
    // fake timers to force a flush before asserting graph state.
    vi.useFakeTimers();
    try {
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
      vi.advanceTimersByTime(250);

      const g = get(graph);
      expect(g?.modules.map((m) => m.id)).toEqual(['m-new']);
    } finally {
      vi.useRealTimers();
    }
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
    // 16.2: debounce buffer flush required before selection reconcile fires.
    vi.useFakeTimers();
    try {
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
      vi.advanceTimersByTime(250);

      expect(get(selection)).toEqual({ kind: 'project', id: 'p1' });
    } finally {
      vi.useRealTimers();
    }
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

// ---------------------------------------------------------------------------
// Helper builders for 16.2 tests
// ---------------------------------------------------------------------------

function seedGraphTwoModules(): GraphJson {
  return {
    project: { id: 'p1', name: 'Proj', description: '', nodeCount: 2, moduleCount: 2, fnCount: 0, status: 'ok' },
    clusters: [],
    modules: [
      { id: 'm1', name: 'm1', description: '', cluster: 'c', clusterName: 'c', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] },
      { id: 'm2', name: 'm2', description: '', cluster: 'c', clusterName: 'c', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] },
    ],
    externals: [],
    relations: [],
    types: [],
    errors: [],
    issues: [],
    detail: {},
  };
}

function seedGraphWithModule(id: string): GraphJson {
  return {
    project: { id: 'p1', name: 'Proj', description: '', nodeCount: 1, moduleCount: 1, fnCount: 0, status: 'ok' },
    clusters: [],
    modules: [
      { id, name: id, description: '', cluster: 'c', clusterName: 'c', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] },
    ],
    externals: [],
    relations: [],
    types: [],
    errors: [],
    issues: [],
    detail: {},
  };
}

function patchAddModule(id: string): GraphPatchJson {
  return {
    ...emptyPatch(),
    modulesAdded: [{ id, name: id, description: '', cluster: 'c', clusterName: 'c', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] }],
    timestamp: Date.now(),
  };
}

function patchRemoveModule(id: string): GraphPatchJson {
  return { ...emptyPatch(), modulesRemoved: [id], timestamp: Date.now() };
}

function patchModifyModule(id: string): GraphPatchJson {
  return {
    ...emptyPatch(),
    modulesModified: [{ id, name: id, description: 'modified', cluster: 'c', clusterName: 'c', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] }],
    timestamp: Date.now(),
  };
}

// ---------------------------------------------------------------------------
// Task 16.2 — debounce, merge, animation
// ---------------------------------------------------------------------------

describe('+page.svelte — task 16.2 debounce, merge, animation', () => {
  afterEach(() => {
    clearPatchEffects();
  });

  it('L1 — two rapid graph-updated coalesce to one graph.update call', async () => {
    vi.useFakeTimers();
    try {
      graph.set(emptyGraph());
      render(Page);
      await tick();

      const handler = listeners.get('graph-updated');
      expect(handler).toBeDefined();

      const updateSpy = vi.spyOn(graph, 'update');
      handler!({ payload: patchAddModule('m-coalesce-1') } as unknown as Parameters<PatchHandler>[0]);
      handler!({ payload: patchAddModule('m-coalesce-2') } as unknown as Parameters<PatchHandler>[0]);

      // No update yet — debounce timer not elapsed
      expect(updateSpy).not.toHaveBeenCalled();

      vi.advanceTimersByTime(250);

      // Exactly one batch update
      expect(updateSpy).toHaveBeenCalledTimes(1);
      updateSpy.mockRestore();
    } finally {
      vi.useRealTimers();
    }
  });

  it('L2 — merged patch contains content from both coalesced patches', async () => {
    vi.useFakeTimers();
    try {
      graph.set(emptyGraph());
      render(Page);
      await tick();

      const handler = listeners.get('graph-updated')!;
      handler({ payload: patchAddModule('m-merged-a') } as unknown as Parameters<PatchHandler>[0]);
      handler({ payload: patchAddModule('m-merged-b') } as unknown as Parameters<PatchHandler>[0]);

      vi.advanceTimersByTime(250);

      const ids = get(graph)!.modules.map((m) => m.id);
      expect(ids).toContain('m-merged-a');
      expect(ids).toContain('m-merged-b');
    } finally {
      vi.useRealTimers();
    }
  });

  it('L3 — debounce-buffer preserves 15.11-C stores AND writes patchEffects', async () => {
    vi.useFakeTimers();
    try {
      clearPatchEffects();
      graph.set(emptyGraph());
      activeLens.set('rules');
      chatDraft.set('preserved-draft');
      chatMode.set('ask');
      nodeViewActiveTab.set('proof');
      nodeCodeLang.set('typescript');
      const beforeChatCount = get(chatMessages).length;

      render(Page);
      await tick();

      const handler = listeners.get('graph-updated')!;
      handler({ payload: patchAddModule('m-l3-a') } as unknown as Parameters<PatchHandler>[0]);
      handler({ payload: patchAddModule('m-l3-b') } as unknown as Parameters<PatchHandler>[0]);

      vi.advanceTimersByTime(250);

      // 15.11-C stores must remain untouched
      expect(get(activeLens)).toBe('rules');
      expect(get(chatDraft)).toBe('preserved-draft');
      expect(get(chatMode)).toBe('ask');
      expect(get(nodeViewActiveTab)).toBe('proof');
      expect(get(nodeCodeLang)).toBe('typescript');
      expect(get(chatMessages).length).toBe(beforeChatCount);

      // patchEffects is the NEW allowlisted write
      expect(get(patchEffects).addedIds.length).toBeGreaterThan(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it('L4 — unmount cancels pending debounce timer (no post-destroy graph.update)', async () => {
    vi.useFakeTimers();
    try {
      graph.set(emptyGraph());
      const { unmount } = render(Page);
      await tick();

      const handler = listeners.get('graph-updated')!;
      handler({ payload: patchAddModule('m-l4') } as unknown as Parameters<PatchHandler>[0]);

      const updateSpy = vi.spyOn(graph, 'update');

      // Unmount before timer fires
      unmount();
      await tick();

      // Advance timer — should NOT fire because destroyed=true and timer cleared
      vi.advanceTimersByTime(500);

      expect(updateSpy).not.toHaveBeenCalled();
      updateSpy.mockRestore();
    } finally {
      vi.useRealTimers();
    }
  });

  it('L5 — preview Apply during burst drains buffer synchronously before patch', async () => {
    vi.useFakeTimers();
    try {
      graph.set(emptyGraph());
      const { container } = render(Page);
      await tick();

      currentRunId.set('r-l5');

      // Queue a watcher patch without flushing
      const handler = listeners.get('graph-updated')!;
      handler({ payload: patchAddModule('m-from-watcher') } as unknown as Parameters<PatchHandler>[0]);

      // Now simulate agent message with preview
      const msgHandler = listeners.get('agent-message')!;
      msgHandler({
        payload: {
          runId: 'r-l5',
          messageId: 'm-l5',
          text: 'proposed',
          preview: { title: 'T', summary: 'S', patch: patchAddModule('m-from-preview') },
        },
      } as unknown as Parameters<PatchHandler>[0]);
      await tick();

      // Click apply on the newly added card (last Confirm button)
      const confirmBtns = container.querySelectorAll('[data-testid="chat-preview-confirm"]');
      fireEvent.click(confirmBtns[confirmBtns.length - 1] as HTMLButtonElement);
      await tick();

      // Both modules must appear (watcher flushed + preview applied)
      const ids = get(graph)!.modules.map((m) => m.id);
      expect(ids).toContain('m-from-watcher');
      expect(ids).toContain('m-from-preview');

      // Timer no longer pending (flushNow cleared it)
      const updateSpy = vi.spyOn(graph, 'update');
      vi.advanceTimersByTime(500);
      expect(updateSpy).not.toHaveBeenCalled();
      updateSpy.mockRestore();
    } finally {
      vi.useRealTimers();
    }
  });

  it('L6 — patchEffects.addedIds populated after flush, cleared after delay', async () => {
    // Fake timers so the assertion lands on deterministic instants:
    //   t=250ms → flushPatch fires → patchEffects populated; setTimeout(clearPatchEffects, 650) scheduled.
    //   t=250+650=900ms → clearPatchEffects fires → patchEffects empty.
    vi.useFakeTimers();
    try {
      graph.set(emptyGraph());
      render(Page);
      await tick();

      clearPatchEffects();
      const handler = listeners.get('graph-updated')!;
      handler({ payload: patchAddModule('m-effects') } as unknown as Parameters<PatchHandler>[0]);

      // Flush
      vi.advanceTimersByTime(250);
      expect(get(patchEffects).addedIds).toContain('m-effects');

      // Clear delay
      vi.advanceTimersByTime(650);
      expect(get(patchEffects).addedIds).toHaveLength(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it('L7 — only modified modules see content change; untouched modules are content-equal', async () => {
    // applyGraphPatch shallow-clones the modules array on entry, so strict
    // object identity (===) is NOT preserved on untouched modules. Svelte's
    // keyed {#each (mod.id)} block uses the id key, not object identity, so
    // re-render scoping still works correctly. This test asserts the
    // observable invariant: untouched module retains its data; modified
    // module reflects the patch payload.
    vi.useFakeTimers();
    try {
      graph.set(seedGraphTwoModules());
      render(Page);
      await tick();

      const m1Before = get(graph)!.modules[0];
      const m2Before = get(graph)!.modules[1];

      const handler = listeners.get('graph-updated')!;
      handler({ payload: patchModifyModule('m2') } as unknown as Parameters<PatchHandler>[0]);

      vi.advanceTimersByTime(250);

      const modsAfter = get(graph)!.modules;
      // m1 is unchanged in content (description still '')
      expect(modsAfter[0].id).toBe('m1');
      expect(modsAfter[0].description).toBe(m1Before.description);
      expect(modsAfter[0].name).toBe(m1Before.name);
      // m2 has the modified content (description: 'modified' from builder)
      expect(modsAfter[1].id).toBe('m2');
      expect(modsAfter[1].description).toBe('modified');
      expect(modsAfter[1].description).not.toBe(m2Before.description);
    } finally {
      vi.useRealTimers();
    }
  });

  it('L8 — buffer that cancels out (add then remove same id) produces zero graph.update calls', async () => {
    vi.useFakeTimers();
    try {
      graph.set(emptyGraph());
      render(Page);
      await tick();

      const updateSpy = vi.spyOn(graph, 'update');
      const handler = listeners.get('graph-updated')!;

      handler({ payload: patchAddModule('m-cancel') } as unknown as Parameters<PatchHandler>[0]);
      handler({ payload: patchRemoveModule('m-cancel') } as unknown as Parameters<PatchHandler>[0]);

      vi.advanceTimersByTime(250);

      // Merged patch is empty → flushPatch bails early → no graph.update
      expect(updateSpy).not.toHaveBeenCalled();
      updateSpy.mockRestore();
    } finally {
      vi.useRealTimers();
    }
  });

  it('L10 — 200-module graph: patch modifying 1 module triggers exactly 1 graph.update flush', async () => {
    vi.useFakeTimers();
    try {
      // Build a 200-module graph
      const big = emptyGraph();
      for (let i = 0; i < 200; i++) {
        big.modules.push({
          id: `m${i}`, name: `m${i}`, description: '', cluster: 'c',
          clusterName: 'c', clusterColor: '#fff', status: 'ok',
          nodeCount: 1, functions: [],
        });
      }
      graph.set(big);
      render(Page);
      await tick();

      const updateSpy = vi.spyOn(graph, 'update');
      const handler = listeners.get('graph-updated')!;
      // Modify one module
      handler({ payload: patchModifyModule('m100') } as unknown as Parameters<PatchHandler>[0]);
      vi.advanceTimersByTime(250);

      // Exactly one debounced flush
      expect(updateSpy).toHaveBeenCalledTimes(1);
      // Only m100 has its content changed; other 199 retain original content
      const mods = get(graph)!.modules;
      expect(mods).toHaveLength(200);
      expect(mods[100].description).toBe('modified');
      expect(mods[0].description).toBe('');
      expect(mods[199].description).toBe('');
      updateSpy.mockRestore();
    } finally {
      vi.useRealTimers();
    }
  });

  it('L9 — preview Apply drains pending watcher buffer before applying preview patch', async () => {
    vi.useFakeTimers();
    try {
      graph.set(emptyGraph());
      const { container } = render(Page);
      await tick();

      currentRunId.set('r-l9');

      // Queue watcher patch without flushing (timer pending)
      const handler = listeners.get('graph-updated')!;
      handler({ payload: patchAddModule('m-watcher-l9') } as unknown as Parameters<PatchHandler>[0]);

      // Add a preview card via agent-message
      const msgHandler = listeners.get('agent-message')!;
      msgHandler({
        payload: {
          runId: 'r-l9',
          messageId: 'm-l9',
          text: 'proposed',
          preview: { title: 'T', summary: 'S', patch: patchAddModule('m-preview-l9') },
        },
      } as unknown as Parameters<PatchHandler>[0]);
      await tick();

      // Click Apply on the newly added card
      const confirmBtns = container.querySelectorAll('[data-testid="chat-preview-confirm"]');
      fireEvent.click(confirmBtns[confirmBtns.length - 1] as HTMLButtonElement);
      await tick();

      // Both modules must be present (watcher flushed via flushNow + preview applied)
      const ids = get(graph)!.modules.map((m) => m.id);
      expect(ids).toContain('m-watcher-l9');
      expect(ids).toContain('m-preview-l9');

      // Debounce timer was cleared by flushNow → no double-apply
      const updateSpy = vi.spyOn(graph, 'update');
      vi.advanceTimersByTime(500);
      expect(updateSpy).not.toHaveBeenCalled();
      updateSpy.mockRestore();
    } finally {
      vi.useRealTimers();
    }
  });
});

// ---------------------------------------------------------------------------
// Phase E (16.3) — Verifier lens integration wiring
// ---------------------------------------------------------------------------

describe('+page.svelte — Phase E verifier wiring (task 16.3)', () => {
  it('E1 — onVerifyComplete listener is registered in onMount', async () => {
    render(Page);
    await tick();
    expect(listeners.has('verify-complete')).toBe(true);
  });

  it('E2 — onVerifyComplete handler with mismatched runId does NOT bump verifyTick', async () => {
    render(Page);
    await tick();
    currentVerifyRunId.set('run-A');
    const tickBefore = get(verifyTick);

    const handler = listeners.get('verify-complete')!;
    handler({
      payload: {
        runId: 'run-B',
        ok: true,
        failures: [],
        scope: 'project',
        nodeIds: [],
      },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();

    expect(get(verifyTick)).toBe(tickBefore);
    // isVerifyRunning and currentVerifyRunId must be untouched
    expect(get(currentVerifyRunId)).toBe('run-A');
  });

  it('E3 — onVerifyComplete handler with matching runId bumps verifyTick and clears running state', async () => {
    render(Page);
    await tick();
    isVerifyRunning.set(true);
    currentVerifyRunId.set('run-X');
    const tickBefore = get(verifyTick);

    const handler = listeners.get('verify-complete')!;
    handler({
      payload: {
        runId: 'run-X',
        ok: true,
        failures: [],
        scope: 'project',
        nodeIds: [],
      },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();

    expect(get(isVerifyRunning)).toBe(false);
    expect(get(currentVerifyRunId)).toBeNull();
    expect(get(verifyTick)).toBe(tickBefore + 1);
  });

  it('E4 — onVerifyComplete handler with cancelled=true does NOT bump verifyTick', async () => {
    render(Page);
    await tick();
    isVerifyRunning.set(true);
    currentVerifyRunId.set('run-C');
    const tickBefore = get(verifyTick);

    const handler = listeners.get('verify-complete')!;
    handler({
      payload: {
        runId: 'run-C',
        ok: false,
        failures: [],
        scope: 'project',
        nodeIds: [],
        cancelled: true,
      },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();

    expect(get(isVerifyRunning)).toBe(false);
    expect(get(currentVerifyRunId)).toBeNull();
    // cancelled run must NOT bump tick
    expect(get(verifyTick)).toBe(tickBefore);
  });

  it('E5 — onVerifyComplete handler skips getNodeDetail when payload.cancelled is true', async () => {
    render(Page);
    await tick();
    isVerifyRunning.set(true);
    currentVerifyRunId.set('run-cancel');

    const getNodeDetailCalls = invoke.mock.calls.filter((c) => c[0] === 'get_node_detail').length;

    const handler = listeners.get('verify-complete')!;
    handler({
      payload: {
        runId: 'run-cancel',
        ok: false,
        failures: [],
        scope: 'project',
        nodeIds: ['step:s1'],
        cancelled: true,
      },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();

    const getNodeDetailCallsAfter = invoke.mock.calls.filter((c) => c[0] === 'get_node_detail').length;
    expect(getNodeDetailCallsAfter).toBe(getNodeDetailCalls);
  });

  it('E6 — applyPatchAndAnimate returns PatchEffects with addedIds after patch with added module', async () => {
    vi.useFakeTimers();
    try {
      graph.set(emptyGraph());
      render(Page);
      await tick();

      const handler = listeners.get('graph-updated')!;
      handler({
        payload: {
          ...patchAddModule('m-effects-e6'),
        },
      } as unknown as Parameters<PatchHandler>[0]);

      vi.advanceTimersByTime(250);

      // After flush, patchEffects should have the added id
      expect(get(patchEffects).addedIds).toContain('m-effects-e6');
    } finally {
      vi.useRealTimers();
    }
  });

  it('E7 — verify-complete handler MUST NOT write graph/selection/activeLens/chat stores', async () => {
    graph.set(emptyGraph());
    activeLens.set('rules');
    chatDraft.set('untouched-draft');
    chatMode.set('ask');
    nodeViewActiveTab.set('proof');
    nodeCodeLang.set('typescript');
    const beforeChatCount = get(chatMessages).length;
    const modulesBefore = get(graph)?.modules.length ?? 0;

    render(Page);
    await tick();
    isVerifyRunning.set(true);
    currentVerifyRunId.set('run-invariant');

    const handler = listeners.get('verify-complete')!;
    handler({
      payload: {
        runId: 'run-invariant',
        ok: true,
        failures: [],
        scope: 'project',
        nodeIds: [],
      },
    } as unknown as Parameters<PatchHandler>[0]);
    await tick();

    // 16.3-C: none of these may be touched by the verify-complete handler
    expect(get(activeLens)).toBe('rules');
    expect(get(chatDraft)).toBe('untouched-draft');
    expect(get(chatMode)).toBe('ask');
    expect(get(nodeViewActiveTab)).toBe('proof');
    expect(get(nodeCodeLang)).toBe('typescript');
    expect(get(chatMessages).length).toBe(beforeChatCount);
    expect(get(graph)?.modules.length).toBe(modulesBefore);
  });
});
