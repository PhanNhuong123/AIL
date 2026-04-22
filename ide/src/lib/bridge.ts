import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  GraphJson, NodeDetail, FlowchartJson,
  VerifyResultJson, GraphPatchJson, Lens, LensStats,
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

// Events (constants mirror crates/ail-ui-bridge/src/events.rs)
export const EVENTS = {
  GRAPH_UPDATED:     'graph-updated',
  VERIFY_COMPLETE:   'verify-complete',
  COVERAGE_COMPLETE: 'coverage-complete',
  AGENT_STEP:        'agent-step',
} as const;

export const onGraphUpdated = (
  h: (p: GraphPatchJson) => void,
): Promise<UnlistenFn> =>
  listen<GraphPatchJson>(EVENTS.GRAPH_UPDATED, (e) => h(e.payload));

export const onVerifyComplete = (
  h: (r: VerifyResultJson) => void,
): Promise<UnlistenFn> =>
  listen<VerifyResultJson>(EVENTS.VERIFY_COMPLETE, (e) => h(e.payload));

export const onCoverageComplete = (h: (p: unknown) => void) =>
  listen(EVENTS.COVERAGE_COMPLETE, (e) => h(e.payload));

export const onAgentStep = (h: (p: unknown) => void) =>
  listen(EVENTS.AGENT_STEP, (e) => h(e.payload));
