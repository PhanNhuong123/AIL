<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import TitleBar from '$lib/chrome/TitleBar.svelte';
  import Outline from '$lib/chrome/Outline.svelte';
  import Stage from '$lib/stage/Stage.svelte';
  import RightSidebar from '$lib/chat/RightSidebar.svelte';
  import WelcomeModal from '$lib/modals/WelcomeModal.svelte';
  import QuickCreateModal from '$lib/modals/QuickCreateModal.svelte';
  import TweaksPanel from '$lib/modals/TweaksPanel.svelte';
  import {
    onGraphUpdated, startWatchProject,
    onAgentStep, onAgentMessage, onAgentComplete,
    onVerifyComplete, runVerifier, cancelVerifierRun, getNodeDetail,
    onSheafComplete, runSheafAnalysis, cancelSheafAnalysis,
  } from '$lib/bridge';
  import type {
    AgentStepPayload, AgentMessagePayload, AgentCompletePayload,
  } from '$lib/types';
  import type { GraphPatchJson, NodeDetail } from '$lib/types';
  type SelectedNodeDetailShape = { id: string; detail: NodeDetail } | null;
  import { graph, selection } from '$lib/stores';
  import {
    chatMessages, chatPreviewCards, currentRunId, isAgentRunning,
  } from '$lib/chat/chat-state';
  import { applyGraphPatch, reconcileSelectionAfterPatch } from '$lib/graph-patch';
  import { mergePatches, isEmptyPatch } from '$lib/patch-merge';
  import { patchEffects, clearPatchEffects, computePatchEffects, CLEAR_DELAY_MS } from '$lib/patch-effects';
  import type { PatchEffects } from '$lib/patch-effects';
  import { isVerifyRunning, currentVerifyRunId, verifyTick } from '$lib/verify/verify-state';
  import { scheduleVerify, reset as resetScheduler } from '$lib/verify/verifier-scheduler';
  import { isSheafRunning, currentSheafRunId, sheafConflicts, resetSheafState } from '$lib/sheaf/sheaf-state';
  import { scheduleSheaf, cancelSheafPending } from '$lib/sheaf/sheaf-scheduler';
  import { hasSheafTriggerFailure } from '$lib/sheaf/trigger-predicate';
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

  // Debounce state for graph-updated patches (task 16.2).
  // `destroyed` is hoisted to module-level so flushPatch can read it safely.
  // patchBuffer is null when no patches are buffered (enables flushNow to
  // return null rather than non-null empty-patch effects).
  let patchBuffer = null as GraphPatchJson | null;
  let debounceTimer = null as ReturnType<typeof setTimeout> | null;
  let clearEffectsTimer = null as ReturnType<typeof setTimeout> | null;
  const DEBOUNCE_MS = 250;
  let destroyed = false;

  // Phase 16.3 — lazy node detail for currently-selected node after verify.
  // Paired-id shape: { id, detail } so Stage can match by real node id.
  let selectedNodeDetail = null as SelectedNodeDetailShape;
  let lastDetailReqSeq = 0;

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

  // Reset selectedNodeDetail when selection changes to avoid stale overrides
  // carrying to a different node. Uses the paired-id shape for a direct compare.
  const unsubSelection = selection.subscribe((s) => {
    if (selectedNodeDetail !== null && selectedNodeDetail.id !== s.id) {
      selectedNodeDetail = null;
    }
  });

  // ---------------------------------------------------------------------------
  // Patch debounce helpers (task 16.2, updated 16.3)
  // ---------------------------------------------------------------------------

  // applyPatchAndAnimate is the SOLE writer of graph + selection + patchEffects
  // (invariant 16.2-A). Returns the computed PatchEffects so callers can pass
  // addedIds + modifiedIds to the verifier scheduler (16.3).
  // Note: no parameter/return type annotations — esrap rejects them (16.2-E).
  function applyPatchAndAnimate(patch) {
    graph.update((g) => (g ? applyGraphPatch(g, patch) : g));
    const next = get(graph);
    const effects = computePatchEffects(patch);
    if (next) {
      selection.update((s) => reconcileSelectionAfterPatch(s, patch, next));
      patchEffects.set(effects);
      if (clearEffectsTimer !== null) clearTimeout(clearEffectsTimer);
      clearEffectsTimer = setTimeout(() => {
        clearPatchEffects();
        clearEffectsTimer = null;
      }, CLEAR_DELAY_MS);
    }
    return effects;
  }

  function schedulePatchFlush(patch) {
    patchBuffer = patchBuffer !== null ? mergePatches(patchBuffer, patch) : patch;
    if (debounceTimer !== null) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(flushPatch, DEBOUNCE_MS);
  }

  // flushPatch — debounce timer callback; calls scheduleVerify after apply.
  function flushPatch() {
    if (destroyed || patchBuffer === null) return;
    const merged = patchBuffer;
    patchBuffer = null;
    debounceTimer = null;
    if (isEmptyPatch(merged)) return;
    const effects = applyPatchAndAnimate(merged);
    const affectedIds = [...effects.addedIds, ...effects.modifiedIds];
    if (affectedIds.length > 0) scheduleVerify(affectedIds, runVerifyNow);
  }

  // flushNow — synchronous drain; returns PatchEffects or null when no patch.
  function flushNow() {
    if (patchBuffer === null) return null as PatchEffects | null;
    const merged = patchBuffer;
    patchBuffer = null;
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
    if (isEmptyPatch(merged)) return null as PatchEffects | null;
    return applyPatchAndAnimate(merged);
  }

  // ---------------------------------------------------------------------------
  // Phase 16.3 — Verifier helpers
  // ---------------------------------------------------------------------------

  // runVerifyNow is the callback passed to scheduleVerify. It cancels any
  // in-flight verifier run, then kicks off a fresh one for the affected ids.
  // Note: no parameter/return type annotations — esrap rejects them (16.2-E).
  async function runVerifyNow(nodeIds) {
    const prevId = get(currentVerifyRunId);
    isVerifyRunning.set(true);  // set BEFORE first await — no visual flash
    if (prevId) await cancelVerifierRun(prevId).catch(() => {});
    try {
      const runId = await runVerifier({ scope: 'project', nodeIds });
      currentVerifyRunId.set(runId);
    } catch {
      isVerifyRunning.set(false);
      currentVerifyRunId.set(null);
    }
  }

  // ---------------------------------------------------------------------------
  // Phase 17.4 — Sheaf helpers
  // ---------------------------------------------------------------------------

  // runSheafNow — kicks off a new sheaf analysis, cancelling any in-flight run.
  // Mirror of runVerifyNow above. No param/return type annotations (16.2-E).
  async function runSheafNow(nodeId) {
    const prevId = get(currentSheafRunId);
    isSheafRunning.set(true);
    if (prevId) await cancelSheafAnalysis(prevId).catch(() => {});
    try {
      const runId = await runSheafAnalysis({ nodeId: nodeId as string | undefined });
      currentSheafRunId.set(runId);
    } catch {
      isSheafRunning.set(false);
      currentSheafRunId.set(null);
    }
  }

  // ---------------------------------------------------------------------------
  // onMount: event subscriptions
  // ---------------------------------------------------------------------------

  onMount(() => {
    const unlistens = [] as Array<() => void>;

    function register(p) {
      p.then((fn) => {
        if (destroyed) fn();
        else unlistens.push(fn);
      });
    }

    // Watcher (invariant 15.11-C + 16.2): patch application writes ONLY
    // `graph`, `selection`, and `patchEffects` (16.2 allowlisted writer).
    register(onGraphUpdated((patch) => {
      schedulePatchFlush(patch);
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

    // Phase 16.3 — verify-complete listener (invariant 16.3-C):
    // Only writes currentVerifyRunId, isVerifyRunning, verifyTick, and
    // script-scoped selectedNodeDetail. MUST NOT write graph/selection/
    // patchEffects/activeLens/chat stores/node-view stores/flow stores.
    register(onVerifyComplete(async (payload) => {
      try {
        // Layer 4 guard: drop events from superseded or unknown runs.
        if (payload.runId !== get(currentVerifyRunId)) return;
        isVerifyRunning.set(false);
        currentVerifyRunId.set(null);
        if (payload.cancelled) return;

        // Bump verifyTick so LensBanner refetches metrics.
        verifyTick.update((n) => n + 1);

        // Phase 17.4 — auto-trigger sheaf analysis on any non-cancelled verify
        // failure. Invariant 17.4-B: this handler MUST NOT write sheaf stores
        // directly; only schedule.
        if (hasSheafTriggerFailure(payload)) {
          scheduleSheaf(undefined, runSheafNow);
        }

        // Lazy detail re-fetch for currently-selected node if it was affected.
        const sel = get(selection);
        if (
          (sel.kind === 'step' || sel.kind === 'function') &&
          sel.id !== null &&
          payload.nodeIds.includes(sel.id)
        ) {
          const seq = ++lastDetailReqSeq;
          const selId = sel.id;  // capture BEFORE await for closure stability
          try {
            const fresh = await getNodeDetail(selId);
            if (seq === lastDetailReqSeq) selectedNodeDetail = { id: selId, detail: fresh };
          } catch { /* ignore — getNodeDetail failed; UI keeps stale detail */ }
        }
      } catch (err) {
        console.warn('[verifier] onVerifyComplete handler failed:', err);
      }
    }));

    // Phase 17.4 — sheaf-complete listener. Writes only sheaf stores.
    // Invariant 17.4-B: this handler MUST NOT write graph/selection/
    // patchEffects/activeLens/chat stores/node-view stores/flow stores.
    register(onSheafComplete((payload) => {
      if (payload.runId !== get(currentSheafRunId)) return;
      isSheafRunning.set(false);
      currentSheafRunId.set(null);
      if (payload.cancelled) {
        sheafConflicts.set([]);
        return;
      }
      sheafConflicts.set(payload.conflicts);
    }));

    return () => {
      destroyed = true;
      if (debounceTimer !== null) {
        clearTimeout(debounceTimer);
        debounceTimer = null;
      }
      if (clearEffectsTimer !== null) {
        clearTimeout(clearEffectsTimer);
        clearEffectsTimer = null;
      }
      patchBuffer = null;
      resetScheduler();
      cancelSheafPending();
      resetSheafState();
      for (const fn of unlistens) fn();
      unsubCurrentRunId();
      unsubSelection();
      pendingEvents.clear();
    };
  });

  // Start the watcher once a project becomes loaded. Tracks project id to
  // support re-load: on a new project the id changes and we start again.
  // Phase 17.4: cancel any in-flight sheaf run BEFORE the new project's state
  // takes over — backend load_project does not emit sheaf-complete on teardown.
  $: {
    const g = $graph;
    const newId = g?.project.id ?? null;
    if (newId && newId !== watchedProjectId) {
      // Cancel sheaf before accepting the new project id so stale sheaf-complete
      // events from the previous project cannot write sheafConflicts.
      const prevSheafId = get(currentSheafRunId);
      if (prevSheafId) {
        cancelSheafAnalysis(prevSheafId).catch(() => {});
      }
      cancelSheafPending();
      resetSheafState();
      watchedProjectId = newId;
      startWatchProject().catch((e) => console.warn('[watcher] start failed', e));
    }
  }

  // Preview Apply handler — invariant 16.1-C: Only +page.svelte writes
  // `graph` / `selection`. Preview cards without a patch (e.g. the seed
  // card) are simply removed.
  // 16.2: flushNow() drains pending watcher buffer before applying the preview.
  // 16.3: combines watcher + preview affectedIds for scheduleVerify.
  function handlePreviewApply(ev) {
    const card = (ev as CustomEvent).detail;
    if (!card) return;
    const watcherEffects = flushNow();
    const previewEffects = card.patch ? applyPatchAndAnimate(card.patch) : null;
    const affectedIds = [
      ...(watcherEffects?.addedIds ?? []), ...(watcherEffects?.modifiedIds ?? []),
      ...(previewEffects?.addedIds ?? []), ...(previewEffects?.modifiedIds ?? []),
    ];
    if (affectedIds.length > 0) scheduleVerify(affectedIds, runVerifyNow);
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
  <main class="region-stage" data-testid="region-stage"><Stage {selectedNodeDetail} /></main>
  <RightSidebar
    on:previewapply={handlePreviewApply}
    on:previewdismiss={handlePreviewDismiss}
  />
</div>

<WelcomeModal />
<QuickCreateModal />
<TweaksPanel />
