/**
 * reviewer-scheduler — trailing-edge debounce for auto-triggered reviewer runs.
 *
 * Reviewer fires after a successful verify-complete (mutually exclusive with
 * sheaf, which fires on failure). A single nodeId (module path-like id) is
 * passed through; multiple rapid calls within the debounce window coalesce
 * to the most recent nodeId.
 *
 * Module-level state is acceptable here: one scheduler instance per page load;
 * HMR clears it on reload (matches 16.2 pattern).
 *
 * This module must not import from bridge or stores — invariant 16.4-A.
 */

export const REVIEWER_DEBOUNCE_MS = 1000;

let timer: ReturnType<typeof setTimeout> | null = null;
let pendingNodeId: string | undefined = undefined;
let pendingCallback: ((nodeId: string) => void) | null = null;

export function scheduleReview(
  nodeId: string,
  callback: (nodeId: string) => void,
): void {
  pendingNodeId = nodeId;
  pendingCallback = callback;
  if (timer !== null) clearTimeout(timer);
  timer = setTimeout(() => {
    const id = pendingNodeId;
    timer = null;
    pendingNodeId = undefined;
    const cb = pendingCallback;
    pendingCallback = null;
    if (cb && id) cb(id);
  }, REVIEWER_DEBOUNCE_MS);
}

export function cancelReviewerPending(): void {
  if (timer !== null) clearTimeout(timer);
  timer = null;
  pendingNodeId = undefined;
  pendingCallback = null;
}

export function reset(): void { cancelReviewerPending(); }
