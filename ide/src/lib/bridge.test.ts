import { describe, it, expect, vi, beforeEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import {
  loadProject,
  getNodeDetail,
  getFlowchart,
  verifyProject,
  saveFlowchart,
  computeLensMetrics,
} from './bridge';
import type { FlowchartJson } from './types';

describe('bridge.ts invoke wrappers', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(undefined);
  });

  it('loadProject forwards path', async () => {
    await loadProject('/tmp/project');
    expect(invoke).toHaveBeenCalledWith('load_project', { path: '/tmp/project' });
  });

  it('getNodeDetail forwards nodeId', async () => {
    await getNodeDetail('n-42');
    expect(invoke).toHaveBeenCalledWith('get_node_detail', { nodeId: 'n-42' });
  });

  it('getFlowchart forwards functionId', async () => {
    await getFlowchart('fn-7');
    expect(invoke).toHaveBeenCalledWith('get_flowchart', { functionId: 'fn-7' });
  });

  it('verifyProject takes no args', async () => {
    await verifyProject();
    expect(invoke).toHaveBeenCalledWith('verify_project');
  });

  it('saveFlowchart forwards functionId + chart', async () => {
    const chart: FlowchartJson = { nodes: [], edges: [] };
    await saveFlowchart('fn-9', chart);
    expect(invoke).toHaveBeenCalledWith('save_flowchart', { functionId: 'fn-9', chart });
  });

  it('computeLensMetrics forwards lens and scopeId', async () => {
    await computeLensMetrics('verify', 'mod1');
    expect(invoke).toHaveBeenCalledWith('compute_lens_metrics', { lens: 'verify', scopeId: 'mod1' });
  });

  it('computeLensMetrics accepts null scope', async () => {
    await computeLensMetrics('structure', null);
    expect(invoke).toHaveBeenCalledWith('compute_lens_metrics', { lens: 'structure', scopeId: null });
  });
});
