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
export interface VerifyResultJson {
  ok: boolean;
  failures: VerifyFailureJson[];
}

export interface VerifyFailureJson {
  nodeId: string;
  message: string;
  stage?: string;
}

// --- Patch (camelCase; internally tagged "kind") ---
// Rust: #[serde(tag = "kind")] enum PatchItem { Module(ModuleJson), Function(FunctionJson) }
// Variant names serialize as "Module" / "Function" (no rename_all on enum).
export type PatchItem =
  | ({ kind: 'Module' } & ModuleJson)
  | ({ kind: 'Function' } & FunctionJson);

export interface GraphPatchJson {
  added: PatchItem[];
  modified: PatchItem[];
  removed: string[];
  // Rust u64. JS number is safe for values ≤ 2^53−1 — fine for epoch-second or
  // epoch-millisecond timestamps, would lose precision for u64 nanoseconds.
  timestamp: number;
}

// --- Error (tag = "code", content = "detail") ---
export type BridgeError =
  | { code: 'ProjectNotFound'; detail: { path: string } }
  | { code: 'PipelineError';   detail: { stage: string; detail: string } }
  | { code: 'NodeNotFound';    detail: { id: string } }
  | { code: 'InvalidInput';    detail: { reason: string } };
