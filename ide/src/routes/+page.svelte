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
    runReviewer, cancelReviewerRun, onCoverageComplete,
    loadProject, loadProjectLayout, runAgent, scaffoldProject, getTutorialPath,
    healthCheckCore, healthCheckAgent, openProjectDialog,
    isTauri,
  } from '$lib/bridge';
  import type {
    AgentStepPayload, AgentMessagePayload, AgentCompletePayload,
  } from '$lib/types';
  import type { GraphPatchJson, NodeDetail } from '$lib/types';
  type SelectedNodeDetailShape = { id: string; detail: NodeDetail } | null;
  import { graph, selection, welcomeModalOpen, welcomeNotice, quickCreateModalOpen, quickCreateNotice, projectLayout } from '$lib/stores';
  import { sidecarHealth, sidecarChecking } from '$lib/sidecar/sidecar-state';
  import {
    chatMessages, chatPreviewCards, currentRunId, isAgentRunning,
  } from '$lib/chat/chat-state';
  import {
    getWelcomeDismissed, setWelcomeDismissed,
  } from '$lib/chat/sidebar-state';
  import { initTweaksState } from '$lib/chrome/tweaks-state';
  import { applyGraphPatch, reconcileSelectionAfterPatch } from '$lib/graph-patch';
  import { mergePatches, isEmptyPatch } from '$lib/patch-merge';
  import { patchEffects, clearPatchEffects, computePatchEffects, CLEAR_DELAY_MS } from '$lib/patch-effects';
  import type { PatchEffects } from '$lib/patch-effects';
  import { isVerifyRunning, currentVerifyRunId, verifyTick } from '$lib/verify/verify-state';
  import { scheduleVerify, reset as resetScheduler } from '$lib/verify/verifier-scheduler';
  import { isSheafRunning, currentSheafRunId, sheafConflicts, resetSheafState } from '$lib/sheaf/sheaf-state';
  import { scheduleSheaf, cancelSheafPending } from '$lib/sheaf/sheaf-scheduler';
  import { hasSheafTriggerFailure } from '$lib/sheaf/trigger-predicate';
  import {
    isReviewerRunning, currentReviewerRunId,
    updateLastReviewed, getLastReviewedStatus, resetReviewerState,
  } from '$lib/reviewer/reviewer-state';
  import {
    scheduleReview, cancelReviewerPending,
    reset as resetReviewerScheduler,
  } from '$lib/reviewer/reviewer-scheduler';
  import {
    hasReviewerTrigger, hasMaterialStatusChange,
  } from '$lib/reviewer/trigger-predicate';
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

  // applyPatchAndAnimate is the sole writer of `graph`/`selection`/`patchEffects`
  // for incremental watcher patches (invariant 16.2-A). Initial hydration of `graph`
  // is performed by `loadAndCloseWelcome` from the `loadProject` IPC response;
  // this function applies subsequent diff patches on top of that hydrated state.
  // Returns the computed PatchEffects so callers can pass addedIds + modifiedIds
  // to the verifier scheduler (16.3).
  // Note: no parameter/return type annotations — esrap rejects them (16.2-E).
  function applyPatchAndAnimate(patch) {
    graph.update((g) => (g ? applyGraphPatch(g, patch) : g));
    const next = get(graph);
    const effects = computePatchEffects(patch);
    if (next) {
      selection.update((s) => reconcileSelectionAfterPatch(s, patch, next));
      patchEffects.set(effects);
      // Sheaf conflicts referencing a step that the patch removed must be
      // dropped — otherwise NodeViewConflictSection still renders a "Jump
      // to peer →" button that no-ops because handleStepJump can't locate
      // the peer in the graph (R3 mitigation). Filter on the post-patch
      // graph so the UI stays in sync with reality.
      sheafConflicts.update((conflicts) => filterConflictsByGraph(conflicts, next));
      if (clearEffectsTimer !== null) clearTimeout(clearEffectsTimer);
      clearEffectsTimer = setTimeout(() => {
        clearPatchEffects();
        clearEffectsTimer = null;
      }, CLEAR_DELAY_MS);
    }
    return effects;
  }

  // Helper: a sheaf conflict is alive iff both its endpoints still exist as
  // step ids in the current graph. Anything else is stale state from a
  // pre-patch sheaf run and must be discarded.
  function filterConflictsByGraph(conflicts, g) {
    if (!g) return conflicts;
    const stepIds = new Set();
    for (const m of g.modules) {
      for (const fn_ of m.functions) {
        for (const step of (fn_.steps ?? [])) {
          stepIds.add(step.id);
        }
      }
    }
    return conflicts.filter((c) => stepIds.has(c.nodeA) && stepIds.has(c.nodeB));
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
  // Phase 16.4 — Reviewer helpers
  // ---------------------------------------------------------------------------

  // runReviewerNow — kicks off a new reviewer run, cancelling any in-flight
  // run. Mirror of runSheafNow above. No param/return type annotations (16.2-E).
  async function runReviewerNow(nodeId) {
    const prevId = get(currentReviewerRunId);
    isReviewerRunning.set(true);                                  // BEFORE first await
    if (prevId) await cancelReviewerRun(prevId).catch(() => {});
    try {
      const runId = await runReviewer({ nodeId });
      currentReviewerRunId.set(runId);
    } catch {
      isReviewerRunning.set(false);
      currentReviewerRunId.set(null);
    }
  }

  // resolveReviewerNodeId — returns the module id for the current selection,
  // or null if selection is project/type/error/none. Used to derive the
  // reviewer scope (module-level). No TS type annotations (16.2-E).
  function resolveReviewerNodeId(sel, g) {
    if (!g || !sel || sel.id === null) return null;
    if (sel.kind === 'module') return sel.id;
    if (sel.kind === 'function' || sel.kind === 'step') {
      for (const mod of g.modules) {
        for (const fn_ of mod.functions) {
          if (fn_.id === sel.id) return mod.id;
          for (const step of (fn_.steps ?? [])) {
            if (step.id === sel.id) return mod.id;
          }
        }
      }
    }
    return null;
  }

  // ---------------------------------------------------------------------------
  // Welcome / Quick Create / Sidecar handlers (closes review findings
  // N1, N1.b, N2, N3). Modals must NOT import bridge.ts (modals/CLAUDE.md);
  // they dispatch Svelte events handled here. The route owns IPC + store
  // writes and closes modals via `welcomeModalOpen` / `quickCreateModalOpen`.
  // ---------------------------------------------------------------------------

  // "User dismissed Welcome on this machine" persistence is delegated to
  // `lib/chat/sidebar-state.ts`, which is the SINGLE allowed localStorage
  // writer in the frontend (invariant 15.12-B). The route reads via
  // `getWelcomeDismissed()` on mount and writes via `setWelcomeDismissed(true)`
  // after a successful project load. Stored under key
  // `ail3_welcome_dismissed_v1` (separate from the sidebar key).

  // Directory picker now lives in `bridge.ts` (`openProjectDialog`) so the
  // route does not import `@tauri-apps/plugin-dialog` directly — bridge.ts
  // is the single Tauri import surface (ide/src/lib/CLAUDE.md).

  async function loadAndCloseWelcome(path) {
    const result = await loadProject(path);
    graph.set(result);
    // Hydrate the per-project sidecar layout so previously persisted drag
    // positions take precedence over the computed swim/system layout. A
    // missing or unreadable layout is non-fatal — the canvas falls back to
    // its computed positions.
    if (isTauri()) {
      try {
        const layout = await loadProjectLayout();
        projectLayout.set(layout);
      } catch (err) {
        console.warn('[layout] hydrate failed:', err);
        projectLayout.set(null);
      }
    } else {
      projectLayout.set(null);
    }
    welcomeModalOpen.set(false);
    quickCreateModalOpen.set(false);
    setWelcomeDismissed(true);
  }

  async function handleWelcomeStart() {
    welcomeModalOpen.set(false);
    quickCreateModalOpen.set(true);
  }

  async function handleWelcomeOpen() {
    // Closes acceptance review MINOR-3 (2026-05-01): in browser preview the
    // native dialog is unavailable, so `openProjectDialog()` resolves to null
    // and the modal would silently no-op. Surface a friendly inline notice
    // (mirrors the sidecar "unavailable in browser preview" pattern) so the
    // user understands why nothing happened.
    if (!isTauri()) {
      welcomeNotice.set('Open is unavailable in browser preview. Launch the AIL desktop app to load a project.');
      return;
    }
    try {
      welcomeNotice.set('');
      const dir = await openProjectDialog();
      if (!dir) return;
      await loadAndCloseWelcome(dir);
    } catch (err) {
      console.warn('[welcome] open failed:', err);
      // Surface the failure in the modal instead of leaving the user staring
      // at an unchanged Welcome panel. Mirrors the QuickCreate pattern.
      welcomeNotice.set(`Couldn't open project: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  async function handleWelcomeTutorial() {
    if (!isTauri()) {
      welcomeNotice.set('Tutorial is unavailable in browser preview. Launch the AIL desktop app to load the bundled example.');
      return;
    }
    try {
      welcomeNotice.set('');
      const path = await getTutorialPath();
      await loadAndCloseWelcome(path);
    } catch (err) {
      console.warn('[welcome] tutorial failed:', err);
      welcomeNotice.set(`Couldn't load tutorial: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  // No param type annotations — esrap rejects them (invariant 16.2-E).
  // `ev` is a CustomEvent with detail = { kind, name, description }.
  async function handleQuickCreate(ev) {
    const { kind, name, description } = ev.detail;
    if (!name) {
      quickCreateNotice.set('Please enter a name before creating.');
      return;
    }
    if (!isTauri()) {
      quickCreateNotice.set('Create is unavailable in browser preview. Launch the AIL desktop app to scaffold a project.');
      return;
    }
    try {
      quickCreateNotice.set('');
      const parentDir = await openProjectDialog();
      if (!parentDir) return;
      const result = await scaffoldProject({ parentDir, kind, name, description });
      await loadAndCloseWelcome(result.projectDir);
    } catch (err) {
      console.warn('[quick-create] failed:', err);
      quickCreateNotice.set(`Couldn't create project: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  async function handleQuickCreateAi(ev) {
    const { kind, name, description } = ev.detail;
    if (!name) {
      quickCreateNotice.set('Please enter a name before asking the agent.');
      return;
    }
    if (!isTauri()) {
      quickCreateNotice.set('Create with AI is unavailable in browser preview. Launch the AIL desktop app to run the agent.');
      return;
    }
    quickCreateNotice.set('');
    quickCreateModalOpen.set(false);
    welcomeModalOpen.set(false);
    setWelcomeDismissed(true);
    // v4.0: when a project is open, agent ADDS to the open project rather than
    // scaffolding a fresh one. selectionKind reflects the additive shape so
    // the agent has the right context, not a stale 'project' scope.
    const isOpenProject = get(graph)?.project != null;
    const verb = isOpenProject ? 'Create a new' : 'Scaffold a';
    const text = description
      ? `${verb} ${kind} named "${name}": ${description}`
      : `${verb} ${kind} named "${name}".`;
    const selectionKind = isOpenProject
      ? (kind as 'function' | 'type' | 'error')
      : 'project';
    try {
      const runId = await runAgent({
        text,
        selectionKind,
        selectionId: null,
        path: [],
        lens: 'structure',
        mode: 'edit',
      });
      currentRunId.set(runId);
      isAgentRunning.set(true);
    } catch (err) {
      console.warn('[quick-create-ai] runAgent failed:', err);
      // The QuickCreate modal already closed (above) so the user has no
      // surface to see the failure. Drop a system-style assistant message
      // into the chat log so the failure is at least visible — same shape
      // the agent-complete error path uses.
      const message = err instanceof Error ? err.message : String(err);
      chatMessages.update((arr) => [
        ...arr,
        {
          id: `qc-ai-err-${++routeMsgSeq}`,
          role: 'assistant',
          text: `Could not start the agent: ${message}`,
        },
      ]);
    }
  }

  // ---------------------------------------------------------------------------
  // Phase 20 — Verify-lens action buttons (NodeTabProof.svelte)
  //
  // `Suggest fix` invokes the AI agent with a focused prompt built from the
  // counterexample. `Relax rule` and `Add handler` need MCP write-back paths
  // that arrive in v4.2; for now they surface a chat notice so the user knows
  // the affordance is acknowledged but unwired.
  // ---------------------------------------------------------------------------

  async function handleSuggestFix(ev) {
    const detail = ev?.detail;
    if (!detail || !detail.stepId) return;
    if (!isTauri()) {
      chatMessages.update((arr) => [
        ...arr,
        {
          id: `proof-fix-preview-${++routeMsgSeq}`,
          role: 'assistant',
          text: "Suggest fix isn't available in browser preview — launch the AIL desktop app to run the agent.",
        },
      ]);
      return;
    }
    const violates = detail.violates ? detail.violates : 'the failing postcondition';
    const scenario = detail.scenario ? ` Scenario: ${detail.scenario}.` : '';
    const text = `Suggest a fix for ${detail.nodeName || detail.stepId} that addresses the counterexample. Violates: ${violates}.${scenario}`;
    try {
      const runId = await runAgent({
        text,
        selectionKind: 'step',
        selectionId: detail.stepId,
        path: [],
        lens: 'verify',
        mode: 'edit',
      });
      currentRunId.set(runId);
      isAgentRunning.set(true);
    } catch (err) {
      console.warn('[proof] suggest-fix failed:', err);
      const message = err instanceof Error ? err.message : String(err);
      chatMessages.update((arr) => [
        ...arr,
        {
          id: `proof-fix-err-${++routeMsgSeq}`,
          role: 'assistant',
          text: `Could not start the agent: ${message}`,
        },
      ]);
    }
  }

  function handleRelaxRule(ev) {
    const detail = ev?.detail;
    if (!detail) return;
    chatMessages.update((arr) => [
      ...arr,
      {
        id: `proof-relax-${++routeMsgSeq}`,
        role: 'assistant',
        text: `Relax-rule for ${detail.nodeName || detail.stepId} arrives in v4.2 — the modal is wired, the MCP write-back path is the v4.2 milestone. For now, ask the agent (Suggest fix) to soften the constraint.`,
      },
    ]);
  }

  function handleAddHandler(ev) {
    const detail = ev?.detail;
    if (!detail) return;
    chatMessages.update((arr) => [
      ...arr,
      {
        id: `proof-handler-${++routeMsgSeq}`,
        role: 'assistant',
        text: `Add-handler scaffolding for ${detail.nodeName || detail.stepId} arrives in v4.2 once the QuickCreate direct-create branch is wired. For now, ask the agent to "add error handling for ${detail.violates || 'the counterexample'}" via Suggest fix.`,
      },
    ]);
  }

  async function handleSidecarCheckCore() {
    sidecarChecking.update((s) => ({ ...s, core: true }));
    try {
      const result = await healthCheckCore();
      sidecarHealth.update((s) => ({ ...s, core: result }));
    } catch (err) {
      console.warn('[sidecar] core check failed:', err);
      // Preserve any prior mode; default to `'bundled'` (production
      // assumption) when no prior payload exists. Avoids falsely claiming
      // `'dev'` mode when the IPC throws inside a packaged bundle.
      sidecarHealth.update((s) => ({
        ...s,
        core: {
          component: 'ail-core',
          ok: false,
          mode: s.core?.mode ?? 'bundled',
          error: err instanceof Error ? err.message : String(err),
        },
      }));
    } finally {
      sidecarChecking.update((s) => ({ ...s, core: false }));
    }
  }

  async function handleSidecarCheckAgent() {
    sidecarChecking.update((s) => ({ ...s, agent: true }));
    try {
      const result = await healthCheckAgent();
      sidecarHealth.update((s) => ({ ...s, agent: result }));
    } catch (err) {
      console.warn('[sidecar] agent check failed:', err);
      sidecarHealth.update((s) => ({
        ...s,
        agent: {
          component: 'ail-agent',
          ok: false,
          mode: s.agent?.mode ?? 'bundled',
          error: err instanceof Error ? err.message : String(err),
        },
      }));
    } finally {
      sidecarChecking.update((s) => ({ ...s, agent: false }));
    }
  }

  // ---------------------------------------------------------------------------
  // onMount: event subscriptions
  // ---------------------------------------------------------------------------

  onMount(() => {
    const unlistens = [] as Array<() => void>;

    // Hydrate Tweaks-panel settings (theme, density, accent) from
    // localStorage and start write-through persistence. Owns DOM
    // application (`html.light` / `html.dark` class hygiene + `--accent`
    // CSS var) so the toggle paths in TweaksPanel only set the stores.
    initTweaksState();

    // Auto-open Welcome on first launch when no project is loaded and the
    // user has not previously dismissed it. Closes review finding **N1**.
    if (get(graph) === null && !getWelcomeDismissed()) {
      welcomeModalOpen.set(true);
    }

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

    // Phase 20 — verify-lens action buttons. NodeTabProof dispatches bubbling
    // CustomEvents on the document; the route shell owns the bridge calls
    // (modals/CLAUDE.md: only routes touch the bridge).
    if (typeof document !== 'undefined') {
      document.addEventListener('suggestfix', handleSuggestFix as EventListener);
      document.addEventListener('relaxrule', handleRelaxRule as EventListener);
      document.addEventListener('addhandler', handleAddHandler as EventListener);
      unlistens.push(() => {
        document.removeEventListener('suggestfix', handleSuggestFix as EventListener);
        document.removeEventListener('relaxrule', handleRelaxRule as EventListener);
        document.removeEventListener('addhandler', handleAddHandler as EventListener);
      });
    }

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

        const sel = get(selection);

        // 16.4 — Schedule reviewer run when verify SUCCEEDS (mutually exclusive with sheaf).
        if (hasReviewerTrigger(payload)) {
          const g = get(graph);
          const moduleId = resolveReviewerNodeId(sel, g);
          if (moduleId) {
            scheduleReview(moduleId, runReviewerNow);
          }
        }

        // Lazy detail re-fetch for currently-selected node if it was affected.
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

    // Phase 16.4 — coverage-complete listener.
    // Writes: isReviewerRunning, currentReviewerRunId, lastReviewedStatus (via
    // updateLastReviewed), and chatMessages (insight only). MUST NOT write
    // graph/selection/patchEffects/activeLens/sheaf stores/node-view stores/
    // flow stores. Invariant 16.4-B.
    register(onCoverageComplete((payload) => {
      // Layer 0 — clear running flag if matching runId
      if (get(currentReviewerRunId) === payload.runId) {
        isReviewerRunning.set(false);
        currentReviewerRunId.set(null);
      } else {
        // Layer 1 — runId guard: superseded payload, ignore everything below
        return;
      }

      // Layer 2 — cancelled guard
      if (payload.cancelled) return;

      // Capture priorStatus BEFORE Layer 3 write
      const priorStatus = getLastReviewedStatus(payload.nodeId);

      // Layer 3 — write map
      updateLastReviewed(payload.nodeId, payload.status);

      // Layer 4 — Guards A and C suppress insight
      if (payload.emptyParent || payload.degenerateBasisFallback) return;

      // Layer 5 — selection-match guard
      const sel = get(selection);
      const selModuleId = resolveReviewerNodeId(sel, get(graph));
      if (selModuleId !== payload.nodeId) return;

      // Layer 6 — material status change check
      if (!hasMaterialStatusChange(priorStatus, payload.status)) return;

      // Layer 7 — emit chat insight
      if (!payload.ok) return;

      const topConcept = payload.missingConcepts[0] ?? 'coverage gaps';
      const fromLabel = priorStatus ?? 'baseline';
      const text = priorStatus === null
        ? `Coverage for \`${payload.nodeId}\` is **${payload.status}**. Top gap: ${topConcept}.`
        : `Coverage for \`${payload.nodeId}\` dropped from **${fromLabel}** to **${payload.status}**. Top gap: ${topConcept}.`;

      chatMessages.update(arr => [
        ...arr,
        {
          id: `review-${payload.runId}-${++routeMsgSeq}`,
          role: 'assistant',
          text,
          ts: Date.now(),
        },
      ]);
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
      cancelReviewerPending();
      resetReviewerState();
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
      cancelReviewerPending();
      resetReviewerState();
      watchedProjectId = newId;
      startWatchProject().catch((e) => console.warn('[watcher] start failed', e));
      runVerifyNow([]);
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
  <TitleBar on:openProject={handleWelcomeOpen} />
  <Outline />
  <main class="region-stage" data-testid="region-stage"><Stage {selectedNodeDetail} /></main>
  <RightSidebar
    on:previewapply={handlePreviewApply}
    on:previewdismiss={handlePreviewDismiss}
  />
</div>

<WelcomeModal
  on:start={handleWelcomeStart}
  on:open={handleWelcomeOpen}
  on:tutorial={handleWelcomeTutorial}
/>
<QuickCreateModal
  on:create={handleQuickCreate}
  on:createAi={handleQuickCreateAi}
/>
<TweaksPanel
  on:checkCore={handleSidecarCheckCore}
  on:checkAgent={handleSidecarCheckAgent}
/>
