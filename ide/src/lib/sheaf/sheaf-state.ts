import { writable } from 'svelte/store';
import type { Writable } from 'svelte/store';
import type { SheafConflictEntry } from '$lib/types';

export const isSheafRunning: Writable<boolean> = writable(false);
export const currentSheafRunId: Writable<string | null> = writable(null);
export const sheafConflicts: Writable<SheafConflictEntry[]> = writable([]);

/**
 * Reset all sheaf-feature stores to their initial values.
 * Called on project unload, app reset, or test teardown.
 */
export function resetSheafState(): void {
  isSheafRunning.set(false);
  currentSheafRunId.set(null);
  sheafConflicts.set([]);
}
