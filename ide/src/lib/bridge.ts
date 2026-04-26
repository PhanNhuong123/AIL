import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  GraphJson, NodeDetail, FlowchartJson,
  VerifyResultJson, VerifyCompletePayload, VerifyCancelResult, VerifierScopeRequest,
  GraphPatchJson, Lens, LensStats,
  AgentRunRequest, AgentStepPayload, AgentMessagePayload,
  AgentCompletePayload, AgentCancelResult,
  SheafCompletePayload, SheafCancelResult,
} from './types';

// Commands
export const loadProject    = (path: string) =>
  invoke<GraphJson>('load_project', { path });

export const getNodeDetail  = (nodeId: string) =>
  invoke<NodeDetail>('get_node_detail', { nodeId });

export const getFlowchart   = (functionId: string) =>
  invoke<FlowchartJson>('get_flowchart', { functionId });

export const verifyProject  = () =>
  invoke<VerifyResultJson>('verify_project');

export const saveFlowchart  = (functionId: string, chart: FlowchartJson) =>
  invoke<void>('save_flowchart', { functionId, chart });

export const computeLensMetrics = (lens: Lens, scopeId: string | null) =>
  invoke<LensStats>('compute_lens_metrics', { lens, scopeId });

// Start the `.ail` file-system watcher for the currently loaded project. The
// command reads the project path from bridge state; requires a prior
// `loadProject` call. Zero-arg by design — the frontend does not track the
// canonical project path.
export const startWatchProject = () =>
  invoke<void>('start_watch_project');

// Run a single agent turn. Returns the `runId` that the frontend must pass
// back into `cancelAgentRun` and must compare against every incoming agent
// event before mutating chat state (invariant 16.1-B layer 4).
export const runAgent = (req: AgentRunRequest) =>
  invoke<string>('run_agent', { req });

// Cancel the active agent run if it matches `runId`. Resolves to
// `{cancelled:false}` when the run is already done / never existed /
// superseded.
export const cancelAgentRun = (runId: string) =>
  invoke<AgentCancelResult>('cancel_agent_run', { runId });

// Phase 16.3 — Verifier commands.
// Run the verifier over the given scope. Returns a runId string that the
// frontend must compare against incoming `verify-complete` payloads.
export const runVerifier = (req: VerifierScopeRequest) =>
  invoke<string>('run_verifier', { scope: req.scope, scopeId: req.scopeId ?? null, nodeIds: req.nodeIds });

// Cancel the active verifier run. Resolves to `{cancelled:false}` when the
// run is already done / never existed / superseded.
export const cancelVerifierRun = (runId: string) =>
  invoke<VerifyCancelResult>('cancel_verifier_run', { runId });

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
): Promise<UnlistenFn> =>
  listen<GraphPatchJson>(EVENTS.GRAPH_UPDATED, (e) => h(e.payload));

export const onVerifyComplete = (
  h: (p: VerifyCompletePayload) => void,
): Promise<UnlistenFn> =>
  listen<VerifyCompletePayload>(EVENTS.VERIFY_COMPLETE, (e) => h(e.payload));

export const onCoverageComplete = (h: (p: unknown) => void) =>
  listen(EVENTS.COVERAGE_COMPLETE, (e) => h(e.payload));

export const onAgentStep = (
  h: (p: AgentStepPayload) => void,
): Promise<UnlistenFn> =>
  listen<AgentStepPayload>(EVENTS.AGENT_STEP, (e) => h(e.payload));

export const onAgentMessage = (
  h: (p: AgentMessagePayload) => void,
): Promise<UnlistenFn> =>
  listen<AgentMessagePayload>(EVENTS.AGENT_MESSAGE, (e) => h(e.payload));

export const onAgentComplete = (
  h: (p: AgentCompletePayload) => void,
): Promise<UnlistenFn> =>
  listen<AgentCompletePayload>(EVENTS.AGENT_COMPLETE, (e) => h(e.payload));

// Phase 17.4 — Sheaf analysis commands and event listener.
// Mirror runVerifier / cancelVerifierRun / onVerifyComplete patterns.

export interface SheafRequest {
  nodeId?: string;
}

// Run sheaf analysis over the current project. Returns the runId (string on
// the wire — same JS number-precision guard as agent/verifier run ids).
export const runSheafAnalysis = (req: SheafRequest = {}): Promise<string> =>
  invoke<string>('run_sheaf_analysis', { nodeId: req.nodeId ?? null });

// Cancel a sheaf analysis run. Resolves to `{cancelled:false}` when the run
// is already done / never existed / superseded.
export const cancelSheafAnalysis = (runId: string): Promise<SheafCancelResult> =>
  invoke<SheafCancelResult>('cancel_sheaf_analysis', { runId });

export const onSheafComplete = (
  handler: (payload: SheafCompletePayload) => void,
): Promise<UnlistenFn> =>
  listen<SheafCompletePayload>(EVENTS.SHEAF_COMPLETE, (e) => handler(e.payload));
