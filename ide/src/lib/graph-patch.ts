// Pure graph-patch helpers for the `.ail` file watcher pipeline.
//
// `applyGraphPatch` merges a fine-grained `GraphPatchJson` (9 arrays) into an
// existing `GraphJson` and returns a NEW value — inputs are never mutated.
//
// `reconcileSelectionAfterPatch` walks the patch to detect whether the
// current `$selection` node was removed, and returns a replacement selection
// that falls back to the closest surviving parent. The caller wires both.
//
// Invariant 15.11-C: this module writes ONLY graph/selection. It MUST NOT
// import `$lib/bridge.ts` or `$lib/stores.ts` — the `Selection` type is
// re-declared locally (mirrors 15.10-B precedent).

import type {
  FunctionJson,
  GraphJson,
  GraphPatchJson,
  ModuleJson,
  StepJson,
} from './types';

// Local re-declaration of the two Selection types so this module stays store-
// independent and Tauri-mock-free in tests. Keep byte-compatible with
// `$lib/stores.ts` exports.
export type SelectionKind =
  | 'project'
  | 'module'
  | 'function'
  | 'step'
  | 'type'
  | 'error'
  | 'none';
export interface Selection {
  kind: SelectionKind;
  id: string | null;
}

// ---------------------------------------------------------------------------
// applyGraphPatch
// ---------------------------------------------------------------------------

/**
 * Apply a fine-grained patch to a graph, returning a new `GraphJson`.
 *
 * Apply order (invariant 15.11-A):
 *   1. stepsRemoved     → drop steps from their parent function's `steps`
 *   2. functionsRemoved → drop functions from their parent module
 *   3. modulesRemoved   → drop modules from graph
 *   4. modulesAdded     → append new modules
 *   5. functionsAdded   → append to parent module's `functions`
 *   6. stepsAdded       → append to parent function's `steps`
 *   7. modulesModified  → replace module in place, preserving existing
 *                         `functions` array (trust `functions_*` arrays only;
 *                         the `functions` field inside `modulesModified`
 *                         payload is ignored)
 *   8. functionsModified → same pattern: preserve `steps`
 *   9. stepsModified    → replace step in place
 *
 * Fields not carried by the patch (`detail`, `issues`, `externals`,
 * `relations`, `clusters`, `types`, `errors`) are preserved from `prev`.
 * A full `loadProject` reconciles them on the next project load.
 */
export function applyGraphPatch(prev: GraphJson, patch: GraphPatchJson): GraphJson {
  // Shallow-clone the module array; we will replace individual modules via
  // .map() below so reference equality only changes on touched entries.
  let modules: ModuleJson[] = prev.modules.map(cloneModule);

  // 1. steps removed
  for (const r of patch.stepsRemoved) {
    modules = modules.map((m) => dropStepInModule(m, r.functionId, r.stepId));
  }
  // 2. functions removed
  for (const r of patch.functionsRemoved) {
    modules = modules.map((m) =>
      m.id === r.moduleId
        ? { ...m, functions: m.functions.filter((f) => f.id !== r.functionId) }
        : m,
    );
  }
  // 3. modules removed
  if (patch.modulesRemoved.length > 0) {
    const removed = new Set(patch.modulesRemoved);
    modules = modules.filter((m) => !removed.has(m.id));
  }
  // 4. modules added. `diff_graph` clones the full ModuleJson (including the
  //    functions subtree) and ALSO emits each of those functions into
  //    `functionsAdded` with its steps into `stepsAdded`. To avoid double
  //    application we strip children here and trust the `functions_*` /
  //    `steps_*` arrays as the single source of truth for child content.
  for (const added of patch.modulesAdded) {
    modules.push({ ...added, functions: [] });
  }
  // 5. functions added. Same rule: strip `steps` from the payload and trust
  //    `stepsAdded` for child content.
  for (const fa of patch.functionsAdded) {
    modules = modules.map((m) =>
      m.id === fa.moduleId
        ? {
            ...m,
            functions: [...m.functions, { ...fa.function, steps: fa.function.steps ? [] : fa.function.steps }],
          }
        : m,
    );
  }
  // 6. steps added
  for (const sa of patch.stepsAdded) {
    modules = modules.map((m) => appendStepInModule(m, sa.functionId, sa.step));
  }
  // 7. modules modified — replace the module's metadata but PRESERVE
  //    its existing `functions` array. `functions` inside the modified
  //    payload is ignored; fn-level changes come exclusively through
  //    `functions_*` arrays.
  for (const mm of patch.modulesModified) {
    modules = modules.map((m) =>
      m.id === mm.id ? { ...cloneModule(mm), functions: m.functions } : m,
    );
  }
  // 8. functions modified — preserve `steps`.
  for (const fm of patch.functionsModified) {
    modules = modules.map((m) =>
      m.id === fm.moduleId
        ? {
            ...m,
            functions: m.functions.map((f) =>
              f.id === fm.function.id ? { ...cloneFn(fm.function), steps: f.steps } : f,
            ),
          }
        : m,
    );
  }
  // 9. steps modified
  for (const sm of patch.stepsModified) {
    modules = modules.map((m) => replaceStepInModule(m, sm.functionId, sm.step));
  }

  const moduleCount = modules.length;
  const fnCount = modules.reduce((n, m) => n + m.functions.length, 0);
  const stepCount = modules.reduce(
    (n, m) => n + m.functions.reduce((k, f) => k + (f.steps?.length ?? 0), 0),
    0,
  );

  return {
    ...prev,
    project: {
      ...prev.project,
      moduleCount,
      fnCount,
      nodeCount: moduleCount + fnCount + stepCount,
    },
    modules,
  };
}

function cloneModule(m: ModuleJson): ModuleJson {
  return { ...m, functions: m.functions.map(cloneFn) };
}

function cloneFn(f: FunctionJson): FunctionJson {
  return { ...f, steps: f.steps ? f.steps.map((s) => ({ ...s })) : f.steps };
}

function dropStepInModule(m: ModuleJson, functionId: string, stepId: string): ModuleJson {
  const idx = m.functions.findIndex((f) => f.id === functionId);
  if (idx < 0) return m;
  const fn = m.functions[idx];
  if (!fn.steps) return m;
  const nextSteps = fn.steps.filter((s) => s.id !== stepId);
  if (nextSteps.length === fn.steps.length) return m;
  const nextFns = m.functions.slice();
  nextFns[idx] = { ...fn, steps: nextSteps };
  return { ...m, functions: nextFns };
}

function appendStepInModule(m: ModuleJson, functionId: string, step: StepJson): ModuleJson {
  const idx = m.functions.findIndex((f) => f.id === functionId);
  if (idx < 0) return m;
  const fn = m.functions[idx];
  const existing = fn.steps ?? [];
  const nextFns = m.functions.slice();
  nextFns[idx] = { ...fn, steps: [...existing, { ...step }] };
  return { ...m, functions: nextFns };
}

function replaceStepInModule(m: ModuleJson, functionId: string, step: StepJson): ModuleJson {
  const idx = m.functions.findIndex((f) => f.id === functionId);
  if (idx < 0) return m;
  const fn = m.functions[idx];
  if (!fn.steps) return m;
  const sIdx = fn.steps.findIndex((s) => s.id === step.id);
  if (sIdx < 0) return m;
  const nextSteps = fn.steps.slice();
  nextSteps[sIdx] = { ...step };
  const nextFns = m.functions.slice();
  nextFns[idx] = { ...fn, steps: nextSteps };
  return { ...m, functions: nextFns };
}

// ---------------------------------------------------------------------------
// reconcileSelectionAfterPatch
// ---------------------------------------------------------------------------

/**
 * If `sel` references a node that was removed by `patch`, return a new
 * selection that collapses to the closest surviving parent. Otherwise
 * return `sel` unchanged.
 *
 * Preserves invariants 15.9-C / 15.10-C: only `graph` and `selection` stores
 * are touched downstream; this helper never reads from Svelte stores.
 */
export function reconcileSelectionAfterPatch(
  sel: Selection,
  patch: GraphPatchJson,
  graph: GraphJson,
): Selection {
  if (sel.kind === 'none' || sel.id === null) return sel;

  if (sel.kind === 'step') {
    const removed = patch.stepsRemoved.find((r) => r.stepId === sel.id);
    if (!removed) return sel;
    // Prefer the parent function if it still exists in the graph.
    if (graph.modules.some((m) => m.functions.some((f) => f.id === removed.functionId))) {
      return { kind: 'function', id: removed.functionId };
    }
    // Function is gone. Check if it's in the SAME patch under
    // `functionsRemoved` — that entry carries the parent moduleId which is
    // not otherwise discoverable from the post-patch graph.
    const fnRemoval = patch.functionsRemoved.find(
      (r) => r.functionId === removed.functionId,
    );
    if (fnRemoval && graph.modules.some((m) => m.id === fnRemoval.moduleId)) {
      return { kind: 'module', id: fnRemoval.moduleId };
    }
    // Last resort: scan the post-patch graph (covers the case where
    // functionsRemoved is absent — function removed via cascade).
    const parentModule = findModuleOfFunction(graph, removed.functionId);
    if (parentModule) return { kind: 'module', id: parentModule };
    return { kind: 'project', id: graph.project.id };
  }

  if (sel.kind === 'function') {
    const removed = patch.functionsRemoved.find((r) => r.functionId === sel.id);
    if (!removed) return sel;
    if (graph.modules.some((m) => m.id === removed.moduleId)) {
      return { kind: 'module', id: removed.moduleId };
    }
    return { kind: 'project', id: graph.project.id };
  }

  if (sel.kind === 'module') {
    if (patch.modulesRemoved.includes(sel.id)) {
      return { kind: 'project', id: graph.project.id };
    }
    return sel;
  }

  return sel;
}

function findModuleOfFunction(graph: GraphJson, functionId: string): string | null {
  for (const m of graph.modules) {
    if (m.functions.some((f) => f.id === functionId)) return m.id;
  }
  return null;
}
