/**
 * trigger-predicate — pure helper deciding whether a verify-complete payload
 * warrants auto-triggering a sheaf analysis run (Phase 17.4).
 *
 * Extracted from routes/+page.svelte so it can be unit-tested directly without
 * importing a Svelte component file. Invariant 17.4-D: single named
 * module-level helper; no store or bridge imports here.
 */

import type { VerifyCompletePayload } from '$lib/types';

/**
 * Returns true when the payload represents a non-cancelled verification run
 * that either failed overall (`ok = false`) or has at least one explicit
 * failure entry. Cancelled payloads always return false.
 */
export function hasSheafTriggerFailure(payload: VerifyCompletePayload): boolean {
  return !payload.cancelled && (!payload.ok || payload.failures.length > 0);
}
