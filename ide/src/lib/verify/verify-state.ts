import { writable, type Writable } from 'svelte/store';

export const isVerifyRunning: Writable<boolean> = writable(false);
export const currentVerifyRunId: Writable<string | null> = writable(null);

/**
 * Monotonic counter incremented on each `verify-complete` event whose runId
 * matches the active run. `LensBanner` adds this as a reactive dependency so
 * metrics refetch when verification finishes (per-scope `computeLensMetrics`
 * carryover from 16.2).
 */
export const verifyTick: Writable<number> = writable(0);

export function resetVerifyState(): void {
  isVerifyRunning.set(false);
  currentVerifyRunId.set(null);
  verifyTick.set(0);
}
