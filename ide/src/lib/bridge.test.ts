import { describe, it, expect, vi, beforeEach } from 'vitest';

const invoke = vi.fn();
const listen = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: (...args: unknown[]) => listen(...args) }));

import {
  loadProject,
  getNodeDetail,
  getFlowchart,
  verifyProject,
  saveFlowchart,
  computeLensMetrics,
  startWatchProject,
  runAgent,
  cancelAgentRun,
  onAgentStep,
  onAgentMessage,
  onAgentComplete,
  EVENTS,
} from './bridge';
import type { AgentRunRequest, FlowchartJson } from './types';

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

  it('startWatchProject invokes start_watch_project with no args', async () => {
    await startWatchProject();
    expect(invoke).toHaveBeenCalledWith('start_watch_project');
  });

  it('runAgent forwards request under key "req"', async () => {
    const req: AgentRunRequest = {
      text: 'add rate limiter',
      selectionKind: 'function',
      selectionId: 'mod1/fn1',
      path: ['mod1', 'fn1'],
      lens: 'verify',
      mode: 'edit',
    };
    invoke.mockResolvedValueOnce('r-42');
    const runId = await runAgent(req);
    expect(invoke).toHaveBeenCalledWith('run_agent', { req });
    expect(runId).toBe('r-42');
  });

  it('cancelAgentRun forwards runId', async () => {
    invoke.mockResolvedValueOnce({ cancelled: true });
    const result = await cancelAgentRun('r-42');
    expect(invoke).toHaveBeenCalledWith('cancel_agent_run', { runId: 'r-42' });
    expect(result).toEqual({ cancelled: true });
  });
});

describe('bridge.ts event listeners', () => {
  beforeEach(() => {
    listen.mockReset();
    listen.mockImplementation((_event: string, _cb: unknown) => Promise.resolve(() => {}));
  });

  it('onAgentStep subscribes to agent-step event', async () => {
    const h = vi.fn();
    await onAgentStep(h);
    expect(listen).toHaveBeenCalledWith(EVENTS.AGENT_STEP, expect.any(Function));
  });

  it('onAgentMessage subscribes to agent-message event', async () => {
    const h = vi.fn();
    await onAgentMessage(h);
    expect(listen).toHaveBeenCalledWith(EVENTS.AGENT_MESSAGE, expect.any(Function));
  });

  it('onAgentComplete subscribes to agent-complete event', async () => {
    const h = vi.fn();
    await onAgentComplete(h);
    expect(listen).toHaveBeenCalledWith(EVENTS.AGENT_COMPLETE, expect.any(Function));
  });

  it('listener callback unwraps event payload', async () => {
    let captured: ((e: { payload: unknown }) => void) | null = null;
    listen.mockImplementationOnce((_e: string, cb: (e: { payload: unknown }) => void) => {
      captured = cb;
      return Promise.resolve(() => {});
    });
    const h = vi.fn();
    await onAgentStep(h);
    expect(captured).not.toBeNull();
    captured!({ payload: { runId: 'r-7', index: 1, phase: 'plan', text: 'hi' } });
    expect(h).toHaveBeenCalledWith({ runId: 'r-7', index: 1, phase: 'plan', text: 'hi' });
  });
});
