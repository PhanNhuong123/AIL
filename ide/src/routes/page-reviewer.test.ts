/**
 * Phase 16.4 — Reviewer lens integration: route-level tests.
 *
 * Tests coverage-complete handler behavior by simulating Tauri events via the
 * mocked `listen` registry, following the same pattern as layout.test.ts.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

const invoke = vi.fn();
type PatchHandler = (payload: unknown) => void;
const listeners = new Map<string, PatchHandler>();
const unlistenFns = new Map<string, ReturnType<typeof vi.fn>>();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
  isTauri: () => 'isTauri' in window && (window as Window & { isTauri?: boolean }).isTauri === true,
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, cb: PatchHandler) => {
    listeners.set(event, cb);
    const un = vi.fn();
    unlistenFns.set(event, un);
    return Promise.resolve(un);
  }),
}));

import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, selection, activeLens } from '$lib/stores';
import type { GraphJson } from '$lib/types';
import { chatMessages, resetChatState } from '$lib/chat/chat-state';
import { sidebarActiveTab, resetSidebarState } from '$lib/chat/sidebar-state';
import { resetVerifyState, currentVerifyRunId } from '$lib/verify/verify-state';
import {
  isReviewerRunning,
  currentReviewerRunId,
  getLastReviewedStatus,
  resetReviewerState,
  updateLastReviewed,
  coverageVersion,
} from '$lib/reviewer/reviewer-state';
import Page from './+page.svelte';

function emptyGraph(projectId = 'p1'): GraphJson {
  return {
    project: {
      id: projectId,
      name: 'Proj',
      description: '',
      nodeCount: 1,
      moduleCount: 1,
      fnCount: 0,
      status: 'ok',
    },
    clusters: [],
    modules: [
      {
        id: 'wallet_service.src',
        name: 'src',
        description: '',
        cluster: 'default',
        clusterName: 'default',
        clusterColor: '#2997ff',
        status: 'ok',
        nodeCount: 1,
        functions: [],
      },
    ],
    externals: [],
    relations: [],
    types: [],
    errors: [],
    issues: [],
    detail: {},
  };
}

function coveragePayload(overrides: Record<string, unknown> = {}) {
  return {
    runId: 'rev-1',
    ok: true,
    status: 'Full',
    nodeId: 'wallet_service.src',
    missingConcepts: [],
    emptyParent: false,
    degenerateBasisFallback: false,
    cancelled: false,
    ...overrides,
  };
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
  listeners.clear();
  unlistenFns.clear();
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  activeLens.set('verify');
  resetChatState();
  resetSidebarState();
  resetVerifyState();
  resetReviewerState();
  sidebarActiveTab.set('chat');
});

// Helper: simulate a coverage-complete event
function fireCoverageComplete(payload: Record<string, unknown>) {
  const handler = listeners.get('coverage-complete');
  expect(handler).toBeDefined();
  handler!({ payload } as unknown as Parameters<PatchHandler>[0]);
}

describe('+page.svelte — Phase 16.4 reviewer lens integration', () => {
  it('reviewer_first_run_full_status_no_chat_insight', async () => {
    graph.set(emptyGraph());
    selection.set({ kind: 'module', id: 'wallet_service.src' });
    render(Page);
    await tick();

    // Seed current run id so the guard passes
    currentReviewerRunId.set('rev-1');
    isReviewerRunning.set(true);

    const msgsBefore = get(chatMessages).length;

    fireCoverageComplete(coveragePayload({ status: 'Full' }));
    await tick();

    // Full on first run → no insight (rank check: null → Full returns false)
    expect(get(chatMessages).length).toBe(msgsBefore);
    // But map was written
    expect(getLastReviewedStatus('wallet_service.src')).toBe('Full');
  });

  it('reviewer_first_run_weak_status_emits_chat_insight', async () => {
    graph.set(emptyGraph());
    selection.set({ kind: 'module', id: 'wallet_service.src' });
    render(Page);
    await tick();

    currentReviewerRunId.set('rev-2');
    isReviewerRunning.set(true);

    const msgsBefore = get(chatMessages).length;

    fireCoverageComplete(coveragePayload({
      runId: 'rev-2',
      status: 'Weak',
      missingConcepts: ['error handling'],
    }));
    await tick();

    // Weak on first run → insight emitted
    expect(get(chatMessages).length).toBe(msgsBefore + 1);
    const msgs = get(chatMessages);
    const last = msgs[msgs.length - 1];
    expect(last.text).toContain('Weak');
    expect(last.text).toContain('error handling');
  });

  it('reviewer_full_to_partial_emits_insight', async () => {
    graph.set(emptyGraph());
    selection.set({ kind: 'module', id: 'wallet_service.src' });
    render(Page);
    await tick();

    // Establish baseline: Full status
    currentReviewerRunId.set('rev-10');
    isReviewerRunning.set(true);
    fireCoverageComplete(coveragePayload({ runId: 'rev-10', status: 'Full' }));
    await tick();

    const msgsBefore = get(chatMessages).length;

    // Now degrade to Partial
    currentReviewerRunId.set('rev-11');
    isReviewerRunning.set(true);
    fireCoverageComplete(coveragePayload({
      runId: 'rev-11',
      status: 'Partial',
      missingConcepts: ['edge cases'],
    }));
    await tick();

    // Full→Partial is degradation → insight emitted
    expect(get(chatMessages).length).toBe(msgsBefore + 1);
    const msgs = get(chatMessages);
    const last = msgs[msgs.length - 1];
    expect(last.text).toContain('Partial');
    expect(last.text).toContain('Full');
  });

  it('reviewer_partial_to_full_no_insight', async () => {
    graph.set(emptyGraph());
    selection.set({ kind: 'module', id: 'wallet_service.src' });
    render(Page);
    await tick();

    // Establish baseline: Partial
    currentReviewerRunId.set('rev-20');
    isReviewerRunning.set(true);
    fireCoverageComplete(coveragePayload({ runId: 'rev-20', status: 'Partial' }));
    await tick();

    const msgsBefore = get(chatMessages).length;

    // Improve to Full
    currentReviewerRunId.set('rev-21');
    isReviewerRunning.set(true);
    fireCoverageComplete(coveragePayload({ runId: 'rev-21', status: 'Full' }));
    await tick();

    // Partial→Full is improvement → no insight
    expect(get(chatMessages).length).toBe(msgsBefore);
    expect(getLastReviewedStatus('wallet_service.src')).toBe('Full');
  });

  it('reviewer_cancelled_payload_does_not_update_lastReviewedStatus', async () => {
    graph.set(emptyGraph());
    selection.set({ kind: 'module', id: 'wallet_service.src' });
    render(Page);
    await tick();

    currentReviewerRunId.set('rev-30');
    isReviewerRunning.set(true);

    fireCoverageComplete(coveragePayload({
      runId: 'rev-30',
      status: 'Weak',
      cancelled: true,
    }));
    await tick();

    // Cancelled → map must NOT be updated
    expect(getLastReviewedStatus('wallet_service.src')).toBeNull();
    // Running flag cleared
    expect(get(isReviewerRunning)).toBe(false);
    expect(get(currentReviewerRunId)).toBeNull();
  });

  it('reviewer_superseded_runId_dropped_no_state_change', async () => {
    graph.set(emptyGraph());
    selection.set({ kind: 'module', id: 'wallet_service.src' });
    render(Page);
    await tick();

    // Active run is rev-40; fire payload for rev-99 (superseded/unknown)
    currentReviewerRunId.set('rev-40');
    isReviewerRunning.set(true);

    const versionBefore = get(coverageVersion);

    fireCoverageComplete(coveragePayload({ runId: 'rev-99', status: 'Weak' }));
    await tick();

    // Layer 1 guard: dropped → nothing changes
    expect(get(isReviewerRunning)).toBe(true);
    expect(get(currentReviewerRunId)).toBe('rev-40');
    expect(getLastReviewedStatus('wallet_service.src')).toBeNull();
    expect(get(coverageVersion)).toBe(versionBefore);
  });

  it('reviewer_emptyParent_payload_suppresses_insight_but_updates_map', async () => {
    graph.set(emptyGraph());
    selection.set({ kind: 'module', id: 'wallet_service.src' });
    render(Page);
    await tick();

    currentReviewerRunId.set('rev-50');
    isReviewerRunning.set(true);

    const msgsBefore = get(chatMessages).length;

    fireCoverageComplete(coveragePayload({
      runId: 'rev-50',
      status: 'Weak',
      emptyParent: true,
      missingConcepts: ['something'],
    }));
    await tick();

    // emptyParent → insight suppressed (Layer 4), but map still written (Layer 3)
    expect(get(chatMessages).length).toBe(msgsBefore);
    expect(getLastReviewedStatus('wallet_service.src')).toBe('Weak');
  });

  it('reviewer_handler_only_writes_allowlisted_stores', async () => {
    graph.set(emptyGraph());
    selection.set({ kind: 'module', id: 'wallet_service.src' });
    activeLens.set('rules');
    render(Page);
    await tick();

    currentReviewerRunId.set('rev-60');
    isReviewerRunning.set(true);

    // Trigger a Weak insight (first run)
    fireCoverageComplete(coveragePayload({
      runId: 'rev-60',
      status: 'Weak',
      missingConcepts: ['rollback logic'],
    }));
    await tick();

    // activeLens MUST NOT be touched
    expect(get(activeLens)).toBe('rules');
    // graph MUST NOT be touched
    expect(get(graph)?.modules.length).toBe(1);
    // selection MUST NOT be touched
    expect(get(selection).id).toBe('wallet_service.src');
  });

  it('outline_marker_visible_when_activeLens_tests_lens', async () => {
    activeLens.set('tests');
    const { container } = render(Page);
    // Set graph after render so reactive project-reload block fires first,
    // then seed coverage data after it clears state.
    graph.set(emptyGraph());
    selection.set({ kind: 'module', id: 'wallet_service.src' });
    await tick();

    // Seed a status into the map AFTER project-reload reactive block has run
    updateLastReviewed('wallet_service.src', 'Weak');
    await tick();

    // Find the module row wrapper — it should carry data-coverage-status
    const el = container.querySelector('[data-coverage-status="Weak"]');
    expect(el).not.toBeNull();
  });

  it('outline_marker_invisible_when_activeLens_verify_lens', async () => {
    activeLens.set('verify');  // verify lens → markers suppressed
    const { container } = render(Page);
    graph.set(emptyGraph());
    selection.set({ kind: 'module', id: 'wallet_service.src' });
    await tick();

    // Seed a status into the map AFTER project-reload reactive block
    updateLastReviewed('wallet_service.src', 'Weak');
    await tick();

    // With verify lens, coverageMarkerFor returns null → attribute not set
    const el = container.querySelector('[data-coverage-status]');
    expect(el).toBeNull();
  });
});
