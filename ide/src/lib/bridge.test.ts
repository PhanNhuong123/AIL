import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
const listen = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
  isTauri: () => 'isTauri' in window && (window as Window & { isTauri?: boolean }).isTauri === true,
}));
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
  runVerifier,
  cancelVerifierRun,
  runSheafAnalysis,
  cancelSheafAnalysis,
  runReviewer,
  cancelReviewerRun,
  scaffoldProject,
  getTutorialPath,
  healthCheckCore,
  healthCheckAgent,
  SIDECAR_BROWSER_PREVIEW_MESSAGE,
  BRIDGE_BROWSER_PREVIEW_MESSAGE,
  onGraphUpdated,
  onVerifyComplete,
  onCoverageComplete,
  onAgentStep,
  onAgentMessage,
  onAgentComplete,
  onSheafComplete,
  EVENTS,
} from './bridge';
import type { AgentRunRequest, FlowchartJson, ProjectScaffoldRequest, VerifierScopeRequest, VerifyCompletePayload } from './types';

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

  it('runVerifier invokes run_verifier with scope/scopeId/nodeIds', async () => {
    invoke.mockResolvedValueOnce('vr-1');
    const req: VerifierScopeRequest = { scope: 'module', scopeId: 'module:m_wallet', nodeIds: ['module:m_wallet', 'function:fn_transfer'] };
    const runId = await runVerifier(req);
    expect(invoke).toHaveBeenCalledWith('run_verifier', { scope: 'module', scopeId: 'module:m_wallet', nodeIds: ['module:m_wallet', 'function:fn_transfer'] });
    expect(runId).toBe('vr-1');
  });

  it('runVerifier maps undefined scopeId to null', async () => {
    invoke.mockResolvedValueOnce('vr-2');
    const req: VerifierScopeRequest = { scope: 'project', nodeIds: [] };
    await runVerifier(req);
    expect(invoke).toHaveBeenCalledWith('run_verifier', { scope: 'project', scopeId: null, nodeIds: [] });
  });

  it('cancelVerifierRun invokes cancel_verifier_run with runId', async () => {
    invoke.mockResolvedValueOnce({ cancelled: true });
    const result = await cancelVerifierRun('vr-7');
    expect(invoke).toHaveBeenCalledWith('cancel_verifier_run', { runId: 'vr-7' });
    expect(result).toEqual({ cancelled: true });
  });

  it('scaffoldProject forwards request under key "request"', async () => {
    const request: ProjectScaffoldRequest = {
      parentDir: '/tmp',
      kind: 'module',
      name: 'wallet',
      description: 'demo',
    };
    invoke.mockResolvedValueOnce({
      projectDir: '/tmp/wallet',
      ailFile: '/tmp/wallet/src/wallet.ail',
    });
    const result = await scaffoldProject(request);
    expect(invoke).toHaveBeenCalledWith('scaffold_project', { request });
    expect(result.projectDir).toBe('/tmp/wallet');
    expect(result.ailFile).toBe('/tmp/wallet/src/wallet.ail');
  });

  it('getTutorialPath invokes get_tutorial_path with no args', async () => {
    invoke.mockResolvedValueOnce('/tmp/examples/wallet_service');
    const path = await getTutorialPath();
    expect(invoke).toHaveBeenCalledWith('get_tutorial_path');
    expect(path).toBe('/tmp/examples/wallet_service');
  });
});

describe('bridge.ts event listeners', () => {
  beforeEach(() => {
    listen.mockReset();
    listen.mockImplementation((_event: string, _cb: unknown) => Promise.resolve(() => {}));
  });

  it('onVerifyComplete subscribes to verify-complete event and unwraps payload', async () => {
    let captured: ((e: { payload: unknown }) => void) | null = null;
    listen.mockImplementationOnce((_e: string, cb: (e: { payload: unknown }) => void) => {
      captured = cb;
      return Promise.resolve(() => {});
    });
    const h = vi.fn();
    await onVerifyComplete(h);
    expect(listen).toHaveBeenCalledWith(EVENTS.VERIFY_COMPLETE, expect.any(Function));
    expect(captured).not.toBeNull();
    const payload: VerifyCompletePayload = {
      ok: true,
      failures: [],
      runId: 'vr-9',
      scope: 'module',
      scopeId: 'module:m_wallet',
      nodeIds: ['module:m_wallet'],
    };
    captured!({ payload });
    expect(h).toHaveBeenCalledWith(payload);
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

// M2 — when running outside the Tauri WebView (e.g., dev preview at
// localhost:1420 in a regular browser), `window.isTauri` is undefined.
// Subscriptions must short-circuit to a no-op `UnlistenFn` so the route
// `register()` lifecycle stays valid and the console stays clean.
describe('bridge.ts event listeners outside Tauri (M2 guard)', () => {
  type TauriWindow = Window & { __TAURI_INTERNALS__?: unknown; isTauri?: boolean };

  beforeEach(() => {
    delete (window as TauriWindow).__TAURI_INTERNALS__;
    delete (window as TauriWindow).isTauri;
    listen.mockReset();
    listen.mockImplementation((_event: string, _cb: unknown) => Promise.resolve(() => {}));
  });

  // Restore the global markers set by `src/test-setup.ts` so subsequent test
  // suites (which assume Tauri-like env) are not affected by these deletions.
  afterEach(() => {
    (window as TauriWindow).__TAURI_INTERNALS__ = {};
    (window as TauriWindow).isTauri = true;
  });

  it.each([
    ['onGraphUpdated', onGraphUpdated],
    ['onVerifyComplete', onVerifyComplete],
    ['onCoverageComplete', onCoverageComplete],
    ['onAgentStep', onAgentStep],
    ['onAgentMessage', onAgentMessage],
    ['onAgentComplete', onAgentComplete],
    ['onSheafComplete', onSheafComplete],
  ] as const)('%s does not call listen() and returns a no-op UnlistenFn', async (_name, fn) => {
    const h = vi.fn();
    const unlisten = await fn(h);
    expect(listen).not.toHaveBeenCalled();
    expect(typeof unlisten).toBe('function');
    expect(() => unlisten()).not.toThrow();
  });

  // M2 extension — sidecar health wrappers must reject with a friendly message
  // instead of letting `invoke()` throw a raw `TypeError: Cannot read
  // properties of undefined (reading 'invoke')` that leaks into the Tweaks UI.
  it.each([
    ['healthCheckCore', healthCheckCore],
    ['healthCheckAgent', healthCheckAgent],
  ] as const)('%s rejects with friendly message and does not call invoke()', async (_name, fn) => {
    invoke.mockReset();
    await expect(fn()).rejects.toThrow(SIDECAR_BROWSER_PREVIEW_MESSAGE);
    expect(invoke).not.toHaveBeenCalled();
  });

  // Closes IMPORTANT finding from acceptance review 2026-05-01:
  // Every `invoke()` wrapper — not just the sidecar health checks — must
  // reject with `BRIDGE_BROWSER_PREVIEW_MESSAGE` when running outside Tauri.
  // Previously only `healthCheck*` carried the guard; calling `runAgent` from
  // the chat panel in Vite preview leaked the raw `invoke` TypeError into the
  // chat log. Cover every `invoke()` wrapper exported from `bridge.ts` that
  // is not the dynamic-import-based `openProjectDialog` (which already
  // returns null in non-Tauri).
  describe('every invoke() wrapper rejects with friendly message in browser preview', () => {
    type Wrapper = readonly [name: string, fn: () => Promise<unknown>];
    const wrappers: ReadonlyArray<Wrapper> = [
      ['loadProject', () => loadProject('/tmp/x')],
      ['getNodeDetail', () => getNodeDetail('n-1')],
      ['getFlowchart', () => getFlowchart('fn-1')],
      ['verifyProject', () => verifyProject()],
      ['saveFlowchart', () => saveFlowchart('fn-1', { nodes: [], edges: [] })],
      ['computeLensMetrics', () => computeLensMetrics('verify', null)],
      ['startWatchProject', () => startWatchProject()],
      ['runAgent', () => runAgent({
        text: 'x', selectionKind: 'function', selectionId: 'm/f',
        path: ['m','f'], lens: 'verify', mode: 'edit',
      })],
      ['cancelAgentRun', () => cancelAgentRun('r-1')],
      ['runVerifier', () => runVerifier({ scope: 'project', nodeIds: [] })],
      ['cancelVerifierRun', () => cancelVerifierRun('vr-1')],
      ['runSheafAnalysis', () => runSheafAnalysis()],
      ['cancelSheafAnalysis', () => cancelSheafAnalysis('sr-1')],
      ['runReviewer', () => runReviewer({ nodeId: 'm:wallet' })],
      ['cancelReviewerRun', () => cancelReviewerRun('rv-1')],
      ['scaffoldProject', () => scaffoldProject({
        parentDir: '/tmp', kind: 'module', name: 'wallet', description: 'x',
      })],
      ['getTutorialPath', () => getTutorialPath()],
    ];

    it.each(wrappers)(
      '%s rejects with BRIDGE_BROWSER_PREVIEW_MESSAGE and does not call invoke()',
      async (_name, fn) => {
        invoke.mockReset();
        await expect(fn()).rejects.toThrow(BRIDGE_BROWSER_PREVIEW_MESSAGE);
        expect(invoke).not.toHaveBeenCalled();
      },
    );
  });
});
