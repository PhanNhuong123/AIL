/**
 * Phase 16.3 — Pure helper that mirrors the Rust `collect_scope_ids` logic.
 *
 * Given a loaded GraphJson and a (scope, scopeId) pair, returns the array of
 * node ids that the verifier should operate on:
 *
 *   'project'  → all module ids ∪ all function ids ∪ all step ids
 *   'module'   → that module id + all functions + all steps under it
 *   'function' → that function id + parent module id + all steps under it
 *   'step'     → that step id + parent function id + grandparent module id
 *
 * Unknown scopeId or unknown scope value → empty array (defensive).
 * Cross-function ancestor traversal is deferred to Phase 17.
 *
 * No imports from $lib/bridge.ts or $lib/stores (mirrors invariant 15.10-B).
 */

import type { GraphJson, ModuleJson, FunctionJson } from '$lib/types';

export function collectScopeIds(
  graph: GraphJson,
  scope: string,
  scopeId: string | null | undefined,
): string[] {
  switch (scope) {
    case 'project':
      return collectProjectIds(graph);
    case 'module':
      return collectModuleIds(graph, scopeId ?? null);
    case 'function':
      return collectFunctionIds(graph, scopeId ?? null);
    case 'step':
      return collectStepIds(graph, scopeId ?? null);
    default:
      return [];
  }
}

function collectProjectIds(graph: GraphJson): string[] {
  const ids: string[] = [];
  for (const mod of graph.modules) {
    ids.push(mod.id);
    for (const fn_ of mod.functions) {
      ids.push(fn_.id);
      for (const step of fn_.steps ?? []) {
        ids.push(step.id);
      }
    }
  }
  return ids;
}

function collectModuleIds(graph: GraphJson, scopeId: string | null): string[] {
  if (scopeId === null) return [];
  const mod = findModule(graph, scopeId);
  if (!mod) return [];

  const ids: string[] = [mod.id];
  for (const fn_ of mod.functions) {
    ids.push(fn_.id);
    for (const step of fn_.steps ?? []) {
      ids.push(step.id);
    }
  }
  return ids;
}

function collectFunctionIds(graph: GraphJson, scopeId: string | null): string[] {
  if (scopeId === null) return [];
  const found = findFunctionWithModule(graph, scopeId);
  if (!found) return [];
  const { mod, fn_ } = found;

  const ids: string[] = [fn_.id, mod.id];
  for (const step of fn_.steps ?? []) {
    ids.push(step.id);
  }
  return ids;
}

function collectStepIds(graph: GraphJson, scopeId: string | null): string[] {
  if (scopeId === null) return [];
  for (const mod of graph.modules) {
    for (const fn_ of mod.functions) {
      for (const step of fn_.steps ?? []) {
        if (step.id === scopeId) {
          return [step.id, fn_.id, mod.id];
        }
      }
    }
  }
  return [];
}

// --- Internal finders ---

function findModule(graph: GraphJson, moduleId: string): ModuleJson | null {
  return graph.modules.find((m) => m.id === moduleId) ?? null;
}

function findFunctionWithModule(
  graph: GraphJson,
  functionId: string,
): { mod: ModuleJson; fn_: FunctionJson } | null {
  for (const mod of graph.modules) {
    for (const fn_ of mod.functions) {
      if (fn_.id === functionId) return { mod, fn_ };
    }
  }
  return null;
}
