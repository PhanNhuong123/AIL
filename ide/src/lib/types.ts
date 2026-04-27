// Mirrors crates/ail-ui-bridge/src/types/*.rs. Keep field-for-field synced.

// --- Status (lowercase serde enum) ---
export type Status = 'ok' | 'warn' | 'fail';

// --- Project / Graph ---
// GraphJson: serde rename_all = "camelCase"
export interface GraphJson {
  project: ProjectJson;
  clusters: ClusterJson[];
  modules: ModuleJson[];
  externals: ExternalJson[];
  relations: RelationJson[];
  types: TypeRefJson[];
  errors: ErrorRefJson[];
  issues: IssueJson[];
  detail: Record<string, NodeDetail>; // BTreeMap<String, NodeDetail>
}

// ProjectJson: camelCase
export interface ProjectJson {
  id: string;
  name: string;
  description: string;
  nodeCount: number;
  moduleCount: number;
  fnCount: number;
  status: Status;
}

// ClusterJson: no rename_all → snake_case default (but no multi-word fields)
export interface ClusterJson {
  id: string;
  name: string;
  color: string;
}

// ModuleJson: camelCase
export interface ModuleJson {
  id: string;
  name: string;
  description: string;
  cluster: string;
  clusterName: string;
  clusterColor: string;
  status: Status;
  nodeCount: number;
  functions: FunctionJson[];
}

// FunctionJson: no rename_all
export interface FunctionJson {
  id: string;
  name: string;
  status: Status;
  steps?: StepJson[]; // skip_serializing_if = "Option::is_none"
}

// StepJson: no rename_all
export interface StepJson {
  id: string;
  name: string;
  status: Status;
  intent: string;
  branch?: string;
}

// RelationJson: no rename_all
export interface RelationJson {
  from: string;
  to: string;
  label: string;
  style?: string;
}

export interface TypeRefJson  { id: string; name: string; status: Status; }
export interface ErrorRefJson { id: string; name: string; status: Status; }
export interface ExternalJson { id: string; name: string; description?: string; }

// --- NodeDetail (no rename_all) ---
export interface NodeDetail {
  name: string;
  status: Status;
  description: string;
  receives: ReceivesEntry[];
  returns: ReturnsEntry[];
  rules: RuleEntry[];
  inherited: InheritedRule[];
  proven: string[];
  verification: VerificationDetail;
  code?: CodeBlob;
}

export interface ReceivesEntry { name: string; desc: string; }
export interface ReturnsEntry  { name: string; desc: string; }

export type RuleSource = 'own' | 'inherited';
export interface RuleEntry     { text: string; source: RuleSource; }
export interface InheritedRule { text: string; from: string; }

export interface VerificationDetail {
  ok: boolean;
  counterexample?: CounterexampleDetail;
  /** Phase 16.3 schema lock — backend MVP always emits None; classification
   *  is deferred to a follow-up that wires the z3-verify feature. Frontend
   *  rendering (VerifyOutcomePill, lens.ts) is ready when the field appears. */
  outcome?: VerifyOutcome;
}

export interface CounterexampleDetail {
  scenario: string;
  effect: string;
  violates: string;
}

export interface CodeBlob {
  python?: string;
  typescript?: string;
}

// --- Flowchart (no rename_all on root; lowercase enum) ---
export interface FlowchartJson {
  nodes: FlowNodeJson[];
  edges: FlowEdgeJson[];
}

export type FlowNodeKind =
  | 'start' | 'end' | 'process' | 'decision' | 'io' | 'sub' | 'storage';

export interface FlowNodeJson {
  id: string;
  kind: FlowNodeKind;
  label: string;
  x: number;
  y: number;
  status?: Status;
}

export interface FlowEdgeJson {
  from: string;
  to: string;
  label?: string;
  style?: string;
}

// --- Verify (camelCase) ---
export type VerifyOutcome = 'fail' | 'timeout' | 'unknown';

export interface VerifyResultJson {
  ok: boolean;
  failures: VerifyFailureJson[];
}

export interface VerifyFailureJson {
  nodeId: string;
  message: string;
  stage?: string;
  severity?: 'fail' | 'warn';
  source?: 'verify' | 'rule' | 'type' | 'parse';
  /** Phase 16.3: outcome subtype. Undefined defaults to 'fail'. */
  outcome?: VerifyOutcome;
}

export type IssueJson = VerifyFailureJson;

/** Phase 16.3: payload for the `verify-complete` Tauri event (superset of VerifyResultJson). */
export interface VerifyCompletePayload {
  ok: boolean;
  failures: VerifyFailureJson[];
  runId: string;
  /** 'project' | 'module' | 'function' | 'step' (or 'cancelled' for cancel emit) */
  scope: string;
  scopeId?: string;
  nodeIds: string[];
  cancelled?: boolean;
}

export interface VerifyCancelResult {
  cancelled: boolean;
}

/** Phase 16.3: request shape for runVerifier. */
export interface VerifierScopeRequest {
  /** 'project' | 'module' | 'function' | 'step' */
  scope: string;
  scopeId?: string;
  nodeIds: string[];
}

// --- Patch (camelCase; fine-grained 9-array shape) ---
export interface FunctionPatchEntry { moduleId: string; function: FunctionJson; }
export interface FunctionRemoval    { moduleId: string; functionId: string; }
export interface StepPatchEntry     { functionId: string; step: StepJson; }
export interface StepRemoval        { functionId: string; stepId: string; }

export interface GraphPatchJson {
  modulesAdded: ModuleJson[];
  modulesModified: ModuleJson[];
  modulesRemoved: string[];
  functionsAdded: FunctionPatchEntry[];
  functionsModified: FunctionPatchEntry[];
  functionsRemoved: FunctionRemoval[];
  stepsAdded: StepPatchEntry[];
  stepsModified: StepPatchEntry[];
  stepsRemoved: StepRemoval[];
  timestamp: number;
}

// --- Lens ---
export type Lens = 'structure' | 'rules' | 'verify' | 'data' | 'tests';

export type LensStats =
  | { lens: 'structure'; modules: number; functions: number; steps: number; nodes: number }
  | { lens: 'rules';     total: number; unproven: number; broken: number }
  | { lens: 'verify';    proven: number; unproven: number; counterexamples: number }
  | { lens: 'data';      types: string[]; signals: number }
  | { lens: 'tests';     total: number; passing: number; failing: number };

// --- Error (tag = "code", content = "detail") ---
export type BridgeError =
  | { code: 'ProjectNotFound'; detail: { path: string } }
  | { code: 'PipelineError';   detail: { stage: string; detail: string } }
  | { code: 'NodeNotFound';    detail: { id: string } }
  | { code: 'InvalidInput';    detail: { reason: string } };

// --- Agent (Phase 16 task 16.1) ---------------------------------------------
// Mirrors crates/ail-ui-bridge/src/types/agent.rs. camelCase.
//
// `runId` is a STRING on the wire: the Rust side composes it from a u64
// counter XOR'd with a session nonce and stringifies the result, so JS's
// IEEE-754 `number` type cannot silently collide high-bit ids at 2^53.

export type AgentMode = 'edit' | 'ask' | 'test';

export type SelectionKindWire =
  | 'project' | 'module' | 'function' | 'step'
  | 'type'    | 'error'  | 'none';

export interface AgentRunRequest {
  text: string;
  selectionKind: SelectionKindWire;
  selectionId: string | null;
  path: string[];
  lens: Lens;
  mode: AgentMode;
  model?: string;
}

export interface AgentStepPayload {
  runId: string;
  index: number;
  phase: string; // 'plan' | 'code' | 'verify' by convention; free-form string on the wire
  text: string;
}

export interface AgentPreview {
  title: string;
  summary: string;
  patch?: GraphPatchJson;
}

export interface AgentMessagePayload {
  runId: string;
  messageId: string;
  text: string;
  preview?: AgentPreview;
}

export interface AgentCompletePayload {
  runId: string;
  status: 'done' | 'error' | 'cancelled';
  error?: string;
}

export interface AgentCancelResult {
  cancelled: boolean;
}

// --- Sheaf (Phase 17.4) ---
// Mirrors crates/ail-ui-bridge/src/types/sheaf.rs. camelCase wire format.
// `runId` stays as string on the wire (JS number precision guard).

/** One contradictory overlap pair in the Čech nerve. */
export interface SheafConflictEntry {
  overlapIndex: number;
  /** Path-like step ID (e.g. "wallet_service.src.transfer.s1") matching StepJson.id in GraphJson */
  nodeA: string;
  /** Path-like step ID (e.g. "wallet_service.src.transfer.s1") matching StepJson.id in GraphJson */
  nodeB: string;
  /** Minimized UNSAT-core constraints attributed to nodeA */
  conflictingA: string[];
  /** Minimized UNSAT-core constraints attributed to nodeB */
  conflictingB: string[];
}

/** Payload emitted on the `sheaf-complete` Tauri event. */
export interface SheafCompletePayload {
  runId: string;
  ok: boolean;
  /** false when the z3-verify feature is absent in the production IDE build */
  z3Available: boolean;
  /** Empty when: z3 absent, pipeline failed, or run cancelled */
  conflicts: SheafConflictEntry[];
  cancelled?: boolean;
  error?: string;
}

export interface SheafCancelResult {
  cancelled: boolean;
}

// === Phase 16.4 — Reviewer (coverage scoring) ===
// Mirror of crates/ail-ui-bridge/src/types/reviewer_result.rs.
// camelCase wire format. runId is string (precision-safe).

export type CoverageStatus = 'Full' | 'Partial' | 'Weak' | 'N/A' | 'Unavailable';

export interface CoverageCompletePayload {
  runId: string;
  ok: boolean;
  status: CoverageStatus;
  score?: number;
  /** PATH-LIKE node id. Empty for cancel emits. Invariant 16.4-R. */
  nodeId: string;
  missingConcepts: string[];
  emptyParent: boolean;
  degenerateBasisFallback: boolean;
  cancelled?: boolean;
}

export interface ReviewerCancelResult { cancelled: boolean; runId: string; }
export interface ReviewerScopeRequest { nodeId?: string; }

// === Phase 16.5 — Sidecar health checks ===
// Mirror of crates/ail-ui-bridge/src/types/sidecar_result.rs.
// camelCase wire format.

/** Whether the sidecar binary was resolved from the bundle or dev-mode path. */
export type SidecarMode = 'bundled' | 'dev';

/** Result returned by `healthCheckCore` and `healthCheckAgent`. */
export interface HealthCheckPayload {
  /** Sidecar component name: `"ail-core"` or `"ail-agent"`. */
  component: string;
  /** `true` if the binary was found and `--version` parsed successfully. */
  ok: boolean;
  /** Bundle path vs dev-mode resolution. */
  mode: SidecarMode;
  /** Parsed version string. Absent when `ok` is `false`. */
  version?: string;
  /** Human-readable error. Absent when `ok` is `true`. */
  error?: string;
}
