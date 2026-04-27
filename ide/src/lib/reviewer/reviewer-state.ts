import { writable } from 'svelte/store';
import type { Writable } from 'svelte/store';

export const isReviewerRunning: Writable<boolean> = writable(false);
export const currentReviewerRunId: Writable<string | null> = writable(null);

// Plain Map (not Writable). Mutation lives only in updateLastReviewed().
const _lastReviewedStatus = new Map<string, string>();

// Counter store: incremented on every updateLastReviewed write so that
// $: blocks subscribed to it re-evaluate (Map itself is not reactive).
export const coverageVersion: Writable<number> = writable(0);

export function updateLastReviewed(nodeId: string, status: string): void {
  _lastReviewedStatus.set(nodeId, status);
  coverageVersion.update(v => v + 1);
}

export function getLastReviewedStatus(nodeId: string): string | null {
  return _lastReviewedStatus.get(nodeId) ?? null;
}

export function resetReviewerState(): void {
  isReviewerRunning.set(false);
  currentReviewerRunId.set(null);
  _lastReviewedStatus.clear();
  coverageVersion.update(v => v + 1);
}
