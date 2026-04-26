// Pure merge helpers for fine-grained GraphPatchJson coalescence.
//
// This module is intentionally store-independent and Tauri-free.
// It must not import from $lib/bridge.ts, $lib/stores.ts, or any Svelte file.
//
// Invariant: mergePatches MUST NOT mutate either input. Always returns fresh arrays.

import type {
  GraphPatchJson,
  ModuleJson,
  FunctionPatchEntry,
  FunctionRemoval,
  StepPatchEntry,
  StepRemoval,
} from './types';

// ---------------------------------------------------------------------------
// emptyPatch / isEmptyPatch
// ---------------------------------------------------------------------------

export function emptyPatch(): GraphPatchJson {
  return {
    modulesAdded: [],
    modulesModified: [],
    modulesRemoved: [],
    functionsAdded: [],
    functionsModified: [],
    functionsRemoved: [],
    stepsAdded: [],
    stepsModified: [],
    stepsRemoved: [],
    timestamp: 0,
  };
}

export function isEmptyPatch(p: GraphPatchJson): boolean {
  return (
    p.modulesAdded.length === 0 &&
    p.modulesModified.length === 0 &&
    p.modulesRemoved.length === 0 &&
    p.functionsAdded.length === 0 &&
    p.functionsModified.length === 0 &&
    p.functionsRemoved.length === 0 &&
    p.stepsAdded.length === 0 &&
    p.stepsModified.length === 0 &&
    p.stepsRemoved.length === 0
  );
}

// ---------------------------------------------------------------------------
// mergePatches — P1=a applied first, P2=b applied second
// ---------------------------------------------------------------------------

/**
 * Merge two patches into a single coalesced patch.
 *
 * Assumes each input patch has each id in at most one of {added, modified,
 * removed} per entity level. Backend diffs always produce single-presence
 * patches. For malformed inputs where the same id appears in both `added` and
 * `removed` within the same patch, `removed` wins — the defensive cleanup at
 * the end of each entity section enforces this by dropping any id from
 * `addedMap` that also appears in `removedSet`.
 *
 * Cancellation rule: when `a.add[X]` AND `b.remove[X]` within the same merge
 * window, both are dropped (the entity briefly existed and was gone before any
 * consumer saw it). When `a.remove[X]` AND `b.add[X]`, the re-add wins.
 */
export function mergePatches(a: GraphPatchJson, b: GraphPatchJson): GraphPatchJson {
  // -------------------------------------------------------------------------
  // Modules
  // -------------------------------------------------------------------------

  // Cancellation rule: when a.add[X] is followed by b.remove[X] within the
  // same merge window, both are dropped (the entity briefly existed and was
  // gone before any consumer saw it). When a.remove[X] is followed by
  // b.add[X], the re-add wins and the remove is dropped.
  const aAddedModIds = new Set(a.modulesAdded.map((m) => m.id));
  const bAddedModIds = new Set(b.modulesAdded.map((m) => m.id));
  const cancelledByBRemove = new Set<string>(
    b.modulesRemoved.filter((id) => aAddedModIds.has(id)),
  );

  // modulesRemoved: union, minus re-adds (b.add cancels a.remove) and minus
  // cancellations (b.remove of a freshly a.add'd id drops the remove too).
  const removedModSet = new Set<string>(
    [
      ...a.modulesRemoved.filter((id) => !bAddedModIds.has(id)),
      ...b.modulesRemoved.filter((id) => !aAddedModIds.has(id)),
    ],
  );

  // modulesAdded: start with a's adds (minus those cancelled by b.remove),
  // then upsert b's adds.
  const addedModMap = new Map<string, ModuleJson>();
  for (const m of a.modulesAdded) {
    if (!cancelledByBRemove.has(m.id)) addedModMap.set(m.id, m);
  }
  for (const m of b.modulesAdded) {
    addedModMap.set(m.id, m);
  }

  // modulesModified: start with a; upsert from b. Drop if id is in merged
  // removed (removed wins). If id is in merged added, fold b.modified payload
  // into the add (add carries authoritative state).
  const modifiedModMap = new Map<string, ModuleJson>(
    a.modulesModified.map((m) => [m.id, m]),
  );
  for (const bm of b.modulesModified) {
    if (removedModSet.has(bm.id)) continue;
    if (addedModMap.has(bm.id)) {
      addedModMap.set(bm.id, bm);
      modifiedModMap.delete(bm.id);
    } else {
      modifiedModMap.set(bm.id, bm);
    }
  }
  for (const id of removedModSet) modifiedModMap.delete(id);
  for (const id of addedModMap.keys()) modifiedModMap.delete(id);
  // Defensive: if an id ended up in BOTH addedModMap and removedModSet (which
  // can only happen for malformed inputs), drop the add — the removed entry
  // represents the more recent/authoritative state.
  for (const id of removedModSet) addedModMap.delete(id);

  // -------------------------------------------------------------------------
  // Functions
  // -------------------------------------------------------------------------

  const aAddedFnIds = new Set(a.functionsAdded.map((fa) => fa.function.id));
  const bAddedFnIds = new Set(b.functionsAdded.map((fa) => fa.function.id));
  const cancelledFnByBRemove = new Set<string>(
    b.functionsRemoved.filter((r) => aAddedFnIds.has(r.functionId)).map((r) => r.functionId),
  );

  const removedFnEntries = [
    ...a.functionsRemoved.filter((r) => !bAddedFnIds.has(r.functionId)),
    ...b.functionsRemoved.filter((r) => !aAddedFnIds.has(r.functionId)),
  ];
  const removedFnSet = new Set<string>(removedFnEntries.map((r) => r.functionId));
  const removedFnMap = new Map<string, FunctionRemoval>(
    removedFnEntries.map((r) => [r.functionId, r]),
  );

  const addedFnMap = new Map<string, FunctionPatchEntry>();
  for (const fa of a.functionsAdded) {
    if (!cancelledFnByBRemove.has(fa.function.id)) addedFnMap.set(fa.function.id, fa);
  }
  for (const fa of b.functionsAdded) {
    addedFnMap.set(fa.function.id, fa);
  }

  const modifiedFnMap = new Map<string, FunctionPatchEntry>(
    a.functionsModified.map((fm) => [fm.function.id, fm]),
  );
  for (const bfm of b.functionsModified) {
    if (removedFnSet.has(bfm.function.id)) continue;
    if (addedFnMap.has(bfm.function.id)) {
      const origAdd = addedFnMap.get(bfm.function.id)!;
      addedFnMap.set(bfm.function.id, { moduleId: origAdd.moduleId, function: bfm.function });
      modifiedFnMap.delete(bfm.function.id);
    } else {
      modifiedFnMap.set(bfm.function.id, bfm);
    }
  }
  for (const id of removedFnSet) modifiedFnMap.delete(id);
  for (const id of addedFnMap.keys()) modifiedFnMap.delete(id);
  // Defensive: malformed input where the same functionId appears in both added
  // and removed — removed wins.
  for (const id of removedFnSet) addedFnMap.delete(id);

  // -------------------------------------------------------------------------
  // Steps
  // -------------------------------------------------------------------------

  const aAddedStepIds = new Set(a.stepsAdded.map((sa) => sa.step.id));
  const bAddedStepIds = new Set(b.stepsAdded.map((sa) => sa.step.id));
  const cancelledStepByBRemove = new Set<string>(
    b.stepsRemoved.filter((r) => aAddedStepIds.has(r.stepId)).map((r) => r.stepId),
  );

  const removedStepEntries = [
    ...a.stepsRemoved.filter((r) => !bAddedStepIds.has(r.stepId)),
    ...b.stepsRemoved.filter((r) => !aAddedStepIds.has(r.stepId)),
  ];
  const removedStepSet = new Set<string>(removedStepEntries.map((r) => r.stepId));
  const removedStepMap = new Map<string, StepRemoval>(
    removedStepEntries.map((r) => [r.stepId, r]),
  );

  const addedStepMap = new Map<string, StepPatchEntry>();
  for (const sa of a.stepsAdded) {
    if (!cancelledStepByBRemove.has(sa.step.id)) addedStepMap.set(sa.step.id, sa);
  }
  for (const sa of b.stepsAdded) {
    addedStepMap.set(sa.step.id, sa);
  }

  const modifiedStepMap = new Map<string, StepPatchEntry>(
    a.stepsModified.map((sm) => [sm.step.id, sm]),
  );
  for (const bsm of b.stepsModified) {
    if (removedStepSet.has(bsm.step.id)) continue;
    if (addedStepMap.has(bsm.step.id)) {
      const origAdd = addedStepMap.get(bsm.step.id)!;
      addedStepMap.set(bsm.step.id, { functionId: origAdd.functionId, step: bsm.step });
      modifiedStepMap.delete(bsm.step.id);
    } else {
      modifiedStepMap.set(bsm.step.id, bsm);
    }
  }
  for (const id of removedStepSet) modifiedStepMap.delete(id);
  for (const id of addedStepMap.keys()) modifiedStepMap.delete(id);
  // Defensive: malformed input where the same stepId appears in both added
  // and removed — removed wins.
  for (const id of removedStepSet) addedStepMap.delete(id);

  // -------------------------------------------------------------------------
  // Assemble result
  // -------------------------------------------------------------------------

  return {
    modulesAdded:      [...addedModMap.values()],
    modulesModified:   [...modifiedModMap.values()],
    modulesRemoved:    [...removedModSet],
    functionsAdded:    [...addedFnMap.values()],
    functionsModified: [...modifiedFnMap.values()],
    functionsRemoved:  [...removedFnMap.values()],
    stepsAdded:        [...addedStepMap.values()],
    stepsModified:     [...modifiedStepMap.values()],
    stepsRemoved:      [...removedStepMap.values()],
    timestamp: b.timestamp,
  };
}
