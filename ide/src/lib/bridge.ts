import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

// Re-export `isTauri` so routes/components can read the runtime marker
// without importing `@tauri-apps/api/core` directly. Bridge is the single
// allowed Tauri import surface (`ide/src/lib/CLAUDE.md`).
export { isTauri };
import type {
  GraphJson, NodeDetail, FlowchartJson,
  VerifyResultJson, VerifyCompletePayload, VerifyCancelResult, VerifierScopeRequest,
  GraphPatchJson, Lens, LensStats,
  AgentRunRequest, AgentStepPayload, AgentMessagePayload,
  AgentCompletePayload, AgentCancelResult,
  SheafCompletePayload, SheafCancelResult,
  CoverageCompletePayload, ReviewerCancelResult, ReviewerScopeRequest,
  HealthCheckPayload,
  ProjectScaffoldRequest, ProjectScaffoldResult,
} from './types';

// Tauri WebView detection. Outside Tauri (e.g., browsing localhost:1420
// directly during frontend dev), `listen()` and `invoke()` would throw a
// `transformCallback` TypeError because the Tauri runtime markers are not
// injected. Each `on*` listener wrapper short-circuits to a no-op
// `UnlistenFn`; each `invoke()` wrapper rejects with
// `BRIDGE_BROWSER_PREVIEW_MESSAGE` instead of leaking the raw TypeError into
// the UI. We use the SDK's official `isTauri()` (reads `window.isTauri`)
// rather than the internal `__TAURI_INTERNALS__` marker, so the contract
// follows whatever the SDK ships with. Real Tauri runtime sets both markers
// before any frontend JS runs, so the production path is unaffected.
const noopUnlisten: UnlistenFn = () => {};

// Sentinel surfaced when an `invoke()` wrapper runs outside a real Tauri
// WebView. Route handlers map this Error back to friendly UI messages
// (status badges, toasts, inline notes) instead of leaking the raw
// `TypeError: Cannot read properties of undefined (reading 'invoke')` into
// the chat panel / sidecar status / Welcome flow.
//
// `SIDECAR_BROWSER_PREVIEW_MESSAGE` is the original 16.5 export and remains
// the alias used by sidecar health checks; `BRIDGE_BROWSER_PREVIEW_MESSAGE`
// is the canonical name for the broader 16.5+ guard pattern that now covers
// every command wrapper.
export const BRIDGE_BROWSER_PREVIEW_MESSAGE = 'AIL backend unavailable in browser preview';

// Commands
export const loadProject    = (path: string) => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<GraphJson>('load_project', { path });
};

export const getNodeDetail  = (nodeId: string) => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<NodeDetail>('get_node_detail', { nodeId });
};

export const getFlowchart   = (functionId: string) => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<FlowchartJson>('get_flowchart', { functionId });
};

export const verifyProject  = () => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<VerifyResultJson>('verify_project');
};

export const saveFlowchart  = (functionId: string, chart: FlowchartJson) => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<void>('save_flowchart', { functionId, chart });
};

export const computeLensMetrics = (lens: Lens, scopeId: string | null) => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<LensStats>('compute_lens_metrics', { lens, scopeId });
};

// Start the `.ail` file-system watcher for the currently loaded project. The
// command reads the project path from bridge state; requires a prior
// `loadProject` call. Zero-arg by design — the frontend does not track the
// canonical project path.
export const startWatchProject = () => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<void>('start_watch_project');
};

// Run a single agent turn. Returns the `runId` that the frontend must pass
// back into `cancelAgentRun` and must compare against every incoming agent
// event before mutating chat state (invariant 16.1-B layer 4).
export const runAgent = (req: AgentRunRequest) => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<string>('run_agent', { req });
};

// Cancel the active agent run if it matches `runId`. Resolves to
// `{cancelled:false}` when the run is already done / never existed /
// superseded.
export const cancelAgentRun = (runId: string) => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<AgentCancelResult>('cancel_agent_run', { runId });
};

// Phase 16.3 — Verifier commands.
// Run the verifier over the given scope. Returns a runId string that the
// frontend must compare against incoming `verify-complete` payloads.
export const runVerifier = (req: VerifierScopeRequest) => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<string>('run_verifier', { scope: req.scope, scopeId: req.scopeId ?? null, nodeIds: req.nodeIds });
};

// Cancel the active verifier run. Resolves to `{cancelled:false}` when the
// run is already done / never existed / superseded.
export const cancelVerifierRun = (runId: string) => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<VerifyCancelResult>('cancel_verifier_run', { runId });
};

// Events (constants mirror crates/ail-ui-bridge/src/events.rs)
export const EVENTS = {
  GRAPH_UPDATED:     'graph-updated',
  VERIFY_COMPLETE:   'verify-complete',
  COVERAGE_COMPLETE: 'coverage-complete',
  AGENT_STEP:        'agent-step',
  AGENT_MESSAGE:     'agent-message',
  AGENT_COMPLETE:    'agent-complete',
  SHEAF_COMPLETE:    'sheaf-complete',
} as const;

export const onGraphUpdated = (
  h: (p: GraphPatchJson) => void,
): Promise<UnlistenFn> => {
  if (!isTauri()) return Promise.resolve(noopUnlisten);
  return listen<GraphPatchJson>(EVENTS.GRAPH_UPDATED, (e) => h(e.payload));
};

export const onVerifyComplete = (
  h: (p: VerifyCompletePayload) => void,
): Promise<UnlistenFn> => {
  if (!isTauri()) return Promise.resolve(noopUnlisten);
  return listen<VerifyCompletePayload>(EVENTS.VERIFY_COMPLETE, (e) => h(e.payload));
};

export const onCoverageComplete = (
  h: (p: CoverageCompletePayload) => void,
): Promise<UnlistenFn> => {
  if (!isTauri()) return Promise.resolve(noopUnlisten);
  return listen<CoverageCompletePayload>(EVENTS.COVERAGE_COMPLETE, (e) => h(e.payload));
};

export const onAgentStep = (
  h: (p: AgentStepPayload) => void,
): Promise<UnlistenFn> => {
  if (!isTauri()) return Promise.resolve(noopUnlisten);
  return listen<AgentStepPayload>(EVENTS.AGENT_STEP, (e) => h(e.payload));
};

export const onAgentMessage = (
  h: (p: AgentMessagePayload) => void,
): Promise<UnlistenFn> => {
  if (!isTauri()) return Promise.resolve(noopUnlisten);
  return listen<AgentMessagePayload>(EVENTS.AGENT_MESSAGE, (e) => h(e.payload));
};

export const onAgentComplete = (
  h: (p: AgentCompletePayload) => void,
): Promise<UnlistenFn> => {
  if (!isTauri()) return Promise.resolve(noopUnlisten);
  return listen<AgentCompletePayload>(EVENTS.AGENT_COMPLETE, (e) => h(e.payload));
};

// Phase 17.4 — Sheaf analysis commands and event listener.
// Mirror runVerifier / cancelVerifierRun / onVerifyComplete patterns.

export interface SheafRequest {
  nodeId?: string;
}

// Run sheaf analysis over the current project. Returns the runId (string on
// the wire — same JS number-precision guard as agent/verifier run ids).
export const runSheafAnalysis = (req: SheafRequest = {}): Promise<string> => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<string>('run_sheaf_analysis', { nodeId: req.nodeId ?? null });
};

// Cancel a sheaf analysis run. Resolves to `{cancelled:false}` when the run
// is already done / never existed / superseded.
export const cancelSheafAnalysis = (runId: string): Promise<SheafCancelResult> => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<SheafCancelResult>('cancel_sheaf_analysis', { runId });
};

export const onSheafComplete = (
  handler: (payload: SheafCompletePayload) => void,
): Promise<UnlistenFn> => {
  if (!isTauri()) return Promise.resolve(noopUnlisten);
  return listen<SheafCompletePayload>(EVENTS.SHEAF_COMPLETE, (e) => handler(e.payload));
};

// Phase 16.4 — Reviewer (coverage scoring) commands.
export const runReviewer = (req: ReviewerScopeRequest): Promise<string> => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<string>('run_reviewer', { nodeId: req.nodeId ?? null });
};

export const cancelReviewerRun = (runId: string): Promise<ReviewerCancelResult> => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<ReviewerCancelResult>('cancel_reviewer_run', { runId });
};

// Phase 16.5 — Sidecar health checks.
// Both commands are zero-arg from the frontend perspective; all state is held
// in BridgeStateInner (sidecar_health_seq / sidecar_id_nonce).
//
// Sentinel surfaced when the sidecar wrappers run outside a real Tauri
// WebView (e.g., `vite dev` opened directly in a browser). The Tweaks
// route handler maps this back to a friendly status string instead of
// leaking the raw `TypeError: Cannot read properties of undefined
// (reading 'invoke')` from `invoke()` into the Tweaks panel UI.
//
// Kept as a distinct alias for the sidecar-specific wording while the
// broader pattern is governed by `BRIDGE_BROWSER_PREVIEW_MESSAGE`.
export const SIDECAR_BROWSER_PREVIEW_MESSAGE = 'Sidecar unavailable in browser preview';

/** Check the ail-cli sidecar health by invoking `ail --version`. */
export const healthCheckCore = (): Promise<HealthCheckPayload> => {
  if (!isTauri()) return Promise.reject(new Error(SIDECAR_BROWSER_PREVIEW_MESSAGE));
  return invoke<HealthCheckPayload>('health_check_core');
};

/** Check the ail-agent sidecar health by invoking `--version`. */
export const healthCheckAgent = (): Promise<HealthCheckPayload> => {
  if (!isTauri()) return Promise.reject(new Error(SIDECAR_BROWSER_PREVIEW_MESSAGE));
  return invoke<HealthCheckPayload>('health_check_agent');
};

// Project scaffolding + tutorial path (closes review findings N1.b + N2).
// `scaffoldProject` writes a minimal `.ail` skeleton; `getTutorialPath`
// returns the bundled `examples/wallet_service` absolute path so the
// Welcome modal can hand a real path to `loadProject`.

/** Write a minimal `.ail` project skeleton at `<parentDir>/<name>`. */
export const scaffoldProject = (
  request: ProjectScaffoldRequest,
): Promise<ProjectScaffoldResult> => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<ProjectScaffoldResult>('scaffold_project', { request });
};

/** Resolve the bundled tutorial project path. */
export const getTutorialPath = (): Promise<string> => {
  if (!isTauri()) return Promise.reject(new Error(BRIDGE_BROWSER_PREVIEW_MESSAGE));
  return invoke<string>('get_tutorial_path');
};

/**
 * Open the native directory-picker via `tauri-plugin-dialog` and return the
 * absolute path the user chose, or `null` when the dialog is dismissed or
 * when the frontend is running outside Tauri (jsdom / Vite-only browser).
 *
 * Dynamic import keeps the plugin off the synchronous import graph so vitest
 * jsdom (which lacks `__TAURI_INTERNALS__`) can still load `bridge.ts` for
 * unit tests of the other wrappers. Bridge.ts is the single allowed Tauri
 * import surface (`ide/src/lib/CLAUDE.md`); routes/components must call this
 * helper instead of importing `@tauri-apps/plugin-dialog` themselves.
 */
export const openProjectDialog = async (): Promise<string | null> => {
  if (!isTauri()) return null;
  const { open } = await import('@tauri-apps/plugin-dialog');
  const result = await open({ directory: true, multiple: false });
  return typeof result === 'string' ? result : null;
};
