import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  nodeViewActiveTab,
  nodeCodeLang,
  nodeTestResult,
  resetTestResultForStep,
  runTestStub,
} from './node-view-state';
import { graph } from '$lib/stores';
import { multiClusterFixture } from './fixtures';

beforeEach(() => {
  nodeViewActiveTab.set('code');
  nodeCodeLang.set('python');
  nodeTestResult.set(null);
  graph.set(null);
});

describe('node-view-state (invariant 16.6-B)', () => {
  it('test_16_6_B_graph_set_does_not_reset_active_tab', () => {
    // Simulate user switching to proof tab
    nodeViewActiveTab.set('proof');
    expect(get(nodeViewActiveTab)).toBe('proof');

    // Graph store update (simulates file-watcher patch)
    graph.set(multiClusterFixture());

    // Tab must not have changed
    expect(get(nodeViewActiveTab)).toBe('proof');
    expect(get(nodeCodeLang)).toBe('python'); // also unchanged
  });

  it('test_16_6_B_same_kind_selection_does_not_reset_active_tab', () => {
    // Simulate user switching to rules tab
    nodeViewActiveTab.set('rules');
    nodeCodeLang.set('typescript');
    expect(get(nodeViewActiveTab)).toBe('rules');

    // Simulating a same-kind selection change: call resetTestResultForStep
    // (what NodeView does on stepId change) — must NOT reset tab or lang.
    resetTestResultForStep('step:s_other');

    expect(get(nodeViewActiveTab)).toBe('rules');    // unchanged
    expect(get(nodeCodeLang)).toBe('typescript');    // unchanged
    expect(get(nodeTestResult)).toBeNull();          // only this was reset
  });

  it('test_runTestStub_sets_result', () => {
    expect(get(nodeTestResult)).toBeNull();
    runTestStub('step:s_transfer');
    const result = get(nodeTestResult);
    expect(result).not.toBeNull();
    expect(result?.passed).toBe(true);
    expect(result?.message).toContain('All assertions passed');
    // Active tab and lang remain untouched
    expect(get(nodeViewActiveTab)).toBe('code');
    expect(get(nodeCodeLang)).toBe('python');
  });
});
