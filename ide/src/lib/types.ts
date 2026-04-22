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
  severity?: 'fail' | 'warn';
  source?: 'verify' | 'rule' | 'type' | 'parse';
}

export type IssueJson = VerifyFailureJson;

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
