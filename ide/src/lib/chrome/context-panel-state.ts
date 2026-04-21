/**
 * context-panel-state.ts — Context-panel-scoped stores and helpers.
 *
 * Invariant: these stores have NO subscription to the global `graph` store.
 * Selection-change resets are driven by ContextPanel.svelte's reactive block,
 * not by a store subscription here.
 *
 * Write rules:
 *   contextActiveTab    — written ONLY by tab-button on:click in ContextPanel
 *   contextTestScenario — written ONLY by textarea bind:value in ContextTabTest
 *   contextTestResult   — written ONLY by resetContextTest + runContextTestStub
 */

import { writable, type Writable } from 'svelte/store';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ContextTab = 'overview' | 'rules' | 'test';

export interface ContextTestResult {
  passed: boolean;
  message: string;
}

// ---------------------------------------------------------------------------
// Stores
// ---------------------------------------------------------------------------

export const contextActiveTab: Writable<ContextTab> = writable('overview');
export const contextTestScenario: Writable<string> = writable('');
export const contextTestResult: Writable<ContextTestResult | null> = writable(null);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Called when the selected node changes.
 * Resets contextTestResult and contextTestScenario.
 * Does NOT touch contextActiveTab.
 */
export function resetContextTest(): void {
  contextTestResult.set(null);
  contextTestScenario.set('');
}

/**
 * Phase 16 stub test runner.
 * Phase 17 will wire this to the real verifier via bridge.ts.
 */
export function runContextTestStub(nodeId: string): void {
  contextTestResult.set({
    passed: true,
    message: `Stub: verification passed for ${nodeId}.`,
  });
}
