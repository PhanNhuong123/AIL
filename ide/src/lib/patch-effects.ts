// patch-effects.ts — Store and helpers for transient CSS patch-state markers.
//
// `patchEffects` is written by the watcher-flush path in +page.svelte
// immediately after applyGraphPatch and cleared after CLEAR_DELAY_MS.
//
// Invariant 16.2: patchEffects is an allowlisted writer alongside graph +
// selection. No other store may be written by the patch path.

import { writable, type Writable } from 'svelte/store';
import type { GraphPatchJson } from './types';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PatchEffects {
  addedIds:    string[];
  modifiedIds: string[];
  removedIds:  string[];
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const CLEAR_DELAY_MS = 650;

export const patchEffects: Writable<PatchEffects> = writable({
  addedIds:    [],
  modifiedIds: [],
  removedIds:  [],
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

export function clearPatchEffects(): void {
  patchEffects.set({ addedIds: [], modifiedIds: [], removedIds: [] });
}

export function computePatchEffects(patch: GraphPatchJson): PatchEffects {
  const addedIds: string[] = [
    ...patch.modulesAdded.map((m) => m.id),
    ...patch.functionsAdded.map((fa) => fa.function.id),
    ...patch.stepsAdded.map((sa) => sa.step.id),
  ];

  const modifiedIds: string[] = [
    ...patch.modulesModified.map((m) => m.id),
    ...patch.functionsModified.map((fm) => fm.function.id),
    ...patch.stepsModified.map((sm) => sm.step.id),
  ];

  const removedIds: string[] = [
    ...patch.modulesRemoved,
    ...patch.functionsRemoved.map((fr) => fr.functionId),
    ...patch.stepsRemoved.map((sr) => sr.stepId),
  ];

  return { addedIds, modifiedIds, removedIds };
}
