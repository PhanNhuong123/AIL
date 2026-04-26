/**
 * sheaf-scheduler — trailing-edge debounce for auto-triggered sheaf runs.
 *
 * The watcher already debounces patches at 250 ms (Phase 16.2), and the
 * verifier scheduler debounces verify at 1 s (Phase 16.3). Sheaf scheduler
 * adds another 1 s debounce on top so rapid verify-complete bursts only
 * trigger one sheaf run.
 */

export const SHEAF_DEBOUNCE_MS = 1000;

let pendingTimer: ReturnType<typeof setTimeout> | null = null;
let pendingNodeId: string | undefined = undefined;

/**
 * Schedule a sheaf run, debounced. Multiple calls in the debounce window
 * coalesce to a single run with the most recent `nodeId`.
 */
export function scheduleSheaf(
  nodeId: string | undefined,
  runFn: (nodeId: string | undefined) => void | Promise<void>
): void {
  pendingNodeId = nodeId;
  if (pendingTimer !== null) {
    clearTimeout(pendingTimer);
  }
  pendingTimer = setTimeout(() => {
    pendingTimer = null;
    const id = pendingNodeId;
    pendingNodeId = undefined;
    void runFn(id);
  }, SHEAF_DEBOUNCE_MS);
}

/** Cancel any pending scheduled sheaf run without firing it. */
export function cancelSheafPending(): void {
  if (pendingTimer !== null) {
    clearTimeout(pendingTimer);
    pendingTimer = null;
  }
  pendingNodeId = undefined;
}

/** Alias of `cancelSheafPending` for symmetry with verify scheduler. */
export function resetSheaf(): void {
  cancelSheafPending();
}
