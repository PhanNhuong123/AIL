/**
 * node-view-state.ts — Node-view-scoped stores, types, and helpers.
 *
 * INVARIANT 16.6-B: nodeViewActiveTab and nodeCodeLang must NOT reset on
 * graph-updated patches nor on same-kind selection changes.
 *
 * Write rules:
 *   nodeViewActiveTab — written ONLY by on:click in NodeView tab strip
 *   nodeCodeLang      — written ONLY by on:click in NodeTabCode lang toggle
 *   nodeTestResult    — written ONLY by resetTestResultForStep + runTestStub
 *
 * resetTestResultForStep touches nodeTestResult ALONE (not the other two).
 * NO subscription to the global `graph` store lives here.
 */

import { writable } from 'svelte/store';
import type { Writable } from 'svelte/store';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type NodeTab = 'code' | 'proof' | 'types' | 'rules' | 'test' | 'history';
export type CodeLang = 'python' | 'typescript';

export interface TestResult {
  passed: boolean;
  message: string;
  durationMs: number;
}

export interface HistoryEntry {
  timestamp: string;
  author: string;
  message: string;
}

// ---------------------------------------------------------------------------
// Stores
// ---------------------------------------------------------------------------

export const nodeViewActiveTab: Writable<NodeTab> = writable('code');
export const nodeCodeLang: Writable<CodeLang>      = writable('python');
export const nodeTestResult: Writable<TestResult | null> = writable(null);

// ---------------------------------------------------------------------------
// Static fixtures
// ---------------------------------------------------------------------------

export const HISTORY_FIXTURE: HistoryEntry[] = [
  {
    timestamp: '2026-04-20T14:32:00Z',
    author: 'Alice',
    message: 'Initial implementation of transfer step',
  },
  {
    timestamp: '2026-04-20T16:10:00Z',
    author: 'Bob',
    message: 'Added balance check guard',
  },
  {
    timestamp: '2026-04-21T09:05:00Z',
    author: 'Alice',
    message: 'Fixed edge case: self-transfer rejected at validation layer',
  },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Called when the selected step changes.
 * Resets ONLY nodeTestResult — does NOT touch nodeViewActiveTab or nodeCodeLang.
 */
export function resetTestResultForStep(_stepId: string): void {
  nodeTestResult.set(null);
}

/**
 * Stub test runner. Simulates a passing test run.
 * Sets nodeTestResult; does NOT touch nodeViewActiveTab or nodeCodeLang.
 */
export function runTestStub(stepId: string): void {
  nodeTestResult.set({
    passed: true,
    message: `All assertions passed for ${stepId}`,
    durationMs: 42,
  });
}
