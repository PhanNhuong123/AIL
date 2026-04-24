<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import TitleBar from '$lib/chrome/TitleBar.svelte';
  import Outline from '$lib/chrome/Outline.svelte';
  import Stage from '$lib/stage/Stage.svelte';
  import ChatPanel from '$lib/chat/ChatPanel.svelte';
  import WelcomeModal from '$lib/modals/WelcomeModal.svelte';
  import QuickCreateModal from '$lib/modals/QuickCreateModal.svelte';
  import TweaksPanel from '$lib/modals/TweaksPanel.svelte';
  import {
    onGraphUpdated, startWatchProject,
    onAgentStep, onAgentMessage, onAgentComplete,
  } from '$lib/bridge';
  import type {
    AgentStepPayload, AgentMessagePayload, AgentCompletePayload,
  } from '$lib/types';
  import { graph, selection } from '$lib/stores';
  import {
    chatMessages, chatPreviewCards, currentRunId, isAgentRunning,
  } from '$lib/chat/chat-state';
  import { applyGraphPatch, reconcileSelectionAfterPatch } from '$lib/graph-patch';
  import '../styles/tokens.css';
  import '../styles/chrome.css';
  import '../styles/stage.css';

  // Track the project id we've already started a watcher for. Re-fires only
  // when `loadProject` returns a different project (re-load scenario).
  let watchedProjectId = null as string | null;

  // Per-component message sequence used for assistant-step/error ids so
  // distinct runs don't collide in the keyed {#each} in ChatMessages.
  let routeMsgSeq = 0;

  // Pending events for run ids the frontend has not yet learned about.
  // The Rust reader task can emit `agent-step` BEFORE `runAgent()` resolves
  // to set `currentRunId`. Without this buffer those early events would be
  // dropped by the runId guard. Keys are runId strings; values are arrays
  // of `{ kind, payload }` records replayed when `currentRunId` matches.
  // Bounded per-run to prevent unbounded growth from a misbehaving sidecar.
  const pendingEvents = new Map();
  const PENDING_LIMIT = 64;

  function bufferEvent(runId, event) {
    const arr = pendingEvents.get(runId) ?? [];
    if (arr.length >= PENDING_LIMIT) return; // drop overflow silently
    arr.push(event);
    pendingEvents.set(runId, arr);
  }

  function applyStep(p) {
    const step = p as AgentStepPayload;
    const id = `step-${step.runId}-${step.index}`;
    chatMessages.update((arr) => [
      ...arr,
      { id, role: 'assistant', text: step.text },
    ]);
  }

  function applyMessage(p) {
    const msg = p as AgentMessagePayload;
    chatMessages.update((arr) => [
      ...arr,
      { id: msg.messageId, role: 'assistant', text: msg.text },
    ]);
    if (msg.preview) {
      chatPreviewCards.update((arr) => [
        ...arr,
        {
          id: msg.messageId,
          title: msg.preview!.title,
          summary: msg.preview!.summary,
          patch: msg.preview!.patch,
          runId: msg.runId,
          messageId: msg.messageId,
        },
      ]);
    }
  }

  function applyComplete(p) {
    const cmp = p as AgentCompletePayload;
    if (cmp.status === 'error') {
      const id = `err-${cmp.runId}-${++routeMsgSeq}`;
      const text = cmp.error ? `Agent error: ${cmp.error}` : 'Agent error.';
      chatMessages.update((arr) => [...arr, { id, role: 'assistant', text }]);
    }
    isAgentRunning.set(false);
    currentRunId.set(null);
  }

  // When `currentRunId` is set after `runAgent()` resolves, replay any
  // events that arrived before the JS round-trip completed.
  const unsubCurrentRunId = currentRunId.subscribe((id) => {
    if (!id) return;
    const buffered = pendingEvents.get(id);
    if (!buffered) return;
    pendingEvents.delete(id);
    for (const e of buffered) {
      if (e.kind === 'step') applyStep(e.payload);
      else if (e.kind === 'message') applyMessage(e.payload);
      else applyComplete(e.payload);
    }
  });

  onMount(() => {
    const unlistens = [] as Array<() => void>;
    let destroyed = false;

    function register(p) {
      p.then((fn) => {
        if (destroyed) fn();
        else unlistens.push(fn);
      });
    }

    // Watcher (invariant 15.11-C): patch application writes ONLY `graph`
    // and `selection`.
    register(onGraphUpdated((patch) => {
      graph.update((g) => (g ? applyGraphPatch(g, patch) : g));
      const next = get(graph);
      if (next) {
        selection.update((s) => reconcileSelectionAfterPatch(s, patch, next));
      }
    }));

    // Phase 16 task 16.1 agent listeners. Each handler guards
    // `payload.runId === currentRunId` BEFORE mutating any chat store
    // (invariant 16.1-B layer 4). Events whose runId is not yet known
    // (race between Rust emit and `runAgent()` IPC reply) are buffered
    // and replayed when `currentRunId` becomes that id. Events whose
    // runId belongs to a cancelled or superseded run are dropped.
    register(onAgentStep((p) => {
      const cur = get(currentRunId);
      if (cur === p.runId) {
        applyStep(p);
      } else if (cur === null) {
        bufferEvent(p.runId, { kind: 'step', payload: p });
      }
    }));

    register(onAgentMessage((p) => {
      const cur = get(currentRunId);
      if (cur === p.runId) {
        applyMessage(p);
      } else if (cur === null) {
        bufferEvent(p.runId, { kind: 'message', payload: p });
      }
    }));

    register(onAgentComplete((p) => {
      const cur = get(currentRunId);
      if (cur === p.runId) {
        applyComplete(p);
      } else if (cur === null) {
        bufferEvent(p.runId, { kind: 'complete', payload: p });
      }
    }));

    return () => {
      destroyed = true;
      for (const fn of unlistens) fn();
      unsubCurrentRunId();
      pendingEvents.clear();
    };
  });

  // Start the watcher once a project becomes loaded. Tracks project id to
  // support re-load: on a new project the id changes and we start again.
  $: {
    const g = $graph;
    const newId = g?.project.id ?? null;
    if (newId && newId !== watchedProjectId) {
      watchedProjectId = newId;
      startWatchProject().catch((e) => console.warn('[watcher] start failed', e));
    }
  }

  // Preview Apply handler — invariant 16.1-C: Only +page.svelte writes
  // `graph` / `selection`. Preview cards without a patch (e.g. the seed
  // card) are simply removed.
  function handlePreviewApply(ev) {
    const card = (ev as CustomEvent).detail;
    if (!card) return;
    if (card.patch) {
      graph.update((g) => (g ? applyGraphPatch(g, card.patch) : g));
      const next = get(graph);
      if (next) {
        selection.update((s) => reconcileSelectionAfterPatch(s, card.patch, next));
      }
    }
    chatPreviewCards.update((arr) => arr.filter((c) => c.id !== card.id));
  }

  function handlePreviewDismiss(ev) {
    const id = (ev as CustomEvent).detail?.id;
    if (!id) return;
    chatPreviewCards.update((arr) => arr.filter((c) => c.id !== id));
  }
</script>

<div class="app-root" data-testid="app-root">
  <TitleBar />
  <Outline />
  <main class="region-stage" data-testid="region-stage"><Stage /></main>
  <ChatPanel
    on:previewapply={handlePreviewApply}
    on:previewdismiss={handlePreviewDismiss}
  />
</div>

<WelcomeModal />
<QuickCreateModal />
<TweaksPanel />
