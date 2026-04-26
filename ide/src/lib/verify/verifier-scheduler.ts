/**
 * Phase 16.3 — Verifier scheduler.
 *
 * Trailing-edge debounce (`VERIFY_DEBOUNCE_MS=1000`) coupled to patch FLUSH
 * completion (called from `flushPatch()` in routes/+page.svelte after
 * `applyPatchAndAnimate` returns its `PatchEffects`). Multiple rapid
 * `scheduleVerify` calls within the debounce window coalesce: the union of
 * all `affectedIds` is passed to the trigger callback exactly once when the
 * timer fires.
 *
 * Empty `affectedIds` arrays are no-ops (do not arm the timer; do not
 * coalesce). Use `cancelPending` to clear without firing; `reset` is the
 * unmount-safe alias.
 *
 * Module-level state is acceptable here: there is one scheduler instance per
 * page load, and HMR clears it on reload (matches the +page.svelte
 * patchBuffer / debounceTimer pattern from 16.2).
 */

export const VERIFY_DEBOUNCE_MS = 1000;

let timer: ReturnType<typeof setTimeout> | null = null;
let accumulator: Set<string> = new Set();
let pendingCallback: ((ids: string[]) => void) | null = null;

export function scheduleVerify(
  affectedIds: string[],
  callback: (ids: string[]) => void,
): void {
  if (affectedIds.length === 0) return;
  for (const id of affectedIds) accumulator.add(id);
  pendingCallback = callback;
  if (timer !== null) clearTimeout(timer);
  timer = setTimeout(() => {
    const ids = Array.from(accumulator);
    accumulator.clear();
    timer = null;
    const cb = pendingCallback;
    pendingCallback = null;
    if (cb) cb(ids);
  }, VERIFY_DEBOUNCE_MS);
}

export function cancelPending(): void {
  if (timer !== null) clearTimeout(timer);
  timer = null;
  accumulator.clear();
  pendingCallback = null;
}

export function reset(): void {
  cancelPending();
}
