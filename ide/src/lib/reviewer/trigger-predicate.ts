/**
 * trigger-predicate — pure helpers for Phase 16.4 reviewer auto-trigger logic.
 *
 * Extracted from routes/+page.svelte so predicates can be unit-tested directly
 * without importing a Svelte component. No store or bridge imports here
 * (invariant 16.4-D mirrors 17.4-D pattern).
 */

import type { VerifyCompletePayload } from '$lib/types';

const STATUS_RANK: Record<string, number> = {
  Full: 3, Partial: 2, Weak: 1, 'N/A': 0, Unavailable: 0,
};

/**
 * Returns true when the verify-complete payload warrants triggering a
 * reviewer run. Reviewer fires on SUCCESS (!cancelled && ok); sheaf fires
 * on failure. Mutually exclusive with hasSheafTriggerFailure.
 */
export function hasReviewerTrigger(payload: VerifyCompletePayload): boolean {
  return !payload.cancelled && payload.ok;
}

/**
 * Returns true when coverage status warrants a chat insight emission.
 * - prev === null (first run): emit only if next is 'Weak' or 'Unavailable'.
 * - Both ranks defined: emit when next rank < prev rank (degradation).
 * - Improvement or same: suppress.
 */
export function hasMaterialStatusChange(
  prev: string | null,
  next: string,
): boolean {
  if (prev === null) return next === 'Weak' || next === 'Unavailable';
  const prevRank = STATUS_RANK[prev] ?? 0;
  const nextRank = STATUS_RANK[next] ?? 0;
  return nextRank < prevRank;
}
