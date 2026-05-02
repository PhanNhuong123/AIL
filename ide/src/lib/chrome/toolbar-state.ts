/**
 * toolbar-state.ts — Toolbar-scoped stores and navigation helpers.
 *
 * History is Toolbar-scoped. Outline.selectNode writes path/selection/zoomLevel
 * directly and is intentionally excluded from the back/forward stack.
 */

import { writable, get } from 'svelte/store';
import type { Writable } from 'svelte/store';
import { graph, selection, path, history } from '$lib/stores';
import type { SelectionKind } from '$lib/stores';
import type { GraphJson, FunctionJson } from '$lib/types';

// ---------------------------------------------------------------------------
// Zoom level type and constants
// ---------------------------------------------------------------------------

export type ZoomLevel = 0 | 1 | 2 | 4;

/** Ordered array used for index-based navigation. Gap between 2 and 4 is intentional. */
export const LEVELS = [0, 1, 2, 4] as const;

export const LEVEL_NAMES: Record<ZoomLevel, string> = {
  0: 'System',
  1: 'Module',
  2: 'Workflow',
  4: 'Detail',
};

// ---------------------------------------------------------------------------
// Stores
// ---------------------------------------------------------------------------

export const zoomLevel: Writable<ZoomLevel> = writable(0);
export const pickerOpen: Writable<boolean>  = writable(false);

export interface PickerItem {
  id: string;
  name: string;
  kind: SelectionKind;
  newPath: string[];
}

export const pickerItems: Writable<PickerItem[]> = writable([]);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Maps a SelectionKind to the canonical zoom level for that depth. */
export function stageLevelForKind(kind: SelectionKind): ZoomLevel {
  switch (kind) {
    case 'project':  return 0;
    case 'module':   return 1;
    case 'function': return 2;
    case 'step':     return 4;
    default:         return 0;
  }
}

// ---------------------------------------------------------------------------
// History — single entry point
// ---------------------------------------------------------------------------

function pushHistory(): void {
  const cur = get(path);
  if (cur.length === 0) return;
  history.update(h => ({ back: [...h.back, JSON.stringify(cur)], forward: [] }));
}

/**
 * Resolve the kind of a path segment by looking it up against the graph.
 * Supports both fixture-style `kind:id` segments and the real ail-ui-bridge
 * parser's bare ids (`wallet_service`, `src`, `src.transfer_money`,
 * `src.transfer_money.new_balance`, types and errors).
 *
 * Returns `'none'` only when the segment cannot be located anywhere in the
 * graph — same behaviour as `breadcrumbs()` skipping unknown segments.
 */
function _kindForSegment(seg: string): SelectionKind {
  const colonIdx = seg.indexOf(':');
  if (colonIdx !== -1) {
    const prefix = seg.slice(0, colonIdx);
    if (
      prefix === 'project' ||
      prefix === 'module' ||
      prefix === 'function' ||
      prefix === 'step' ||
      prefix === 'type' ||
      prefix === 'error'
    ) {
      return prefix as SelectionKind;
    }
  }
  // Bare id — consult the graph to infer kind.
  const g = get(graph);
  if (!g) return 'none';
  if (g.project.id === seg) return 'project';
  if (g.modules.some((m) => m.id === seg)) return 'module';
  for (const m of g.modules) {
    for (const fn_ of m.functions) {
      if (fn_.id === seg) return 'function';
      for (const step of fn_.steps ?? []) {
        if (step.id === seg) return 'step';
      }
    }
  }
  if (g.types?.some((t) => t.id === seg)) return 'type';
  if (g.errors?.some((e) => e.id === seg)) return 'error';
  return 'none';
}

/** Restore selection from the last segment of a path array. */
function _restoreSelectionFromPath(p: string[]): void {
  if (p.length === 0) {
    selection.set({ kind: 'none', id: null });
    return;
  }
  const last = p[p.length - 1];
  const kind = _kindForSegment(last);
  if (kind === 'none') {
    selection.set({ kind: 'none', id: null });
    return;
  }
  selection.set({ kind, id: last });
}

function _restoreZoomFromPath(parsed: string[]): void {
  if (parsed.length === 0) return;
  const kind = _kindForSegment(parsed[parsed.length - 1]);
  if (kind === 'none') return;
  zoomLevel.set(stageLevelForKind(kind));
}

export function goBack(): void {
  const h = get(history);
  if (h.back.length === 0) return;
  const prev = h.back[h.back.length - 1];
  const cur = get(path);
  history.set({ back: h.back.slice(0, -1), forward: [JSON.stringify(cur), ...h.forward] });
  const parsed: string[] = JSON.parse(prev);
  path.set(parsed);
  _restoreSelectionFromPath(parsed);
  _restoreZoomFromPath(parsed);
}

export function goForward(): void {
  const h = get(history);
  if (h.forward.length === 0) return;
  const next = h.forward[0];
  const cur = get(path);
  history.set({ back: [...h.back, JSON.stringify(cur)], forward: h.forward.slice(1) });
  const parsed: string[] = JSON.parse(next);
  path.set(parsed);
  _restoreSelectionFromPath(parsed);
  _restoreZoomFromPath(parsed);
}

/**
 * Single entry point for all toolbar-initiated navigation.
 * Pushes current path to history BEFORE applying the new state.
 * Breadcrumb clicks and zoom handlers MUST delegate here.
 * Do NOT call pushHistory from outside this function / goBack / goForward.
 */
export function navigateTo(
  newPath: string[],
  kind: SelectionKind,
  id: string,
  level: ZoomLevel,
): void {
  pushHistory();
  path.set(newPath);
  selection.set({ kind, id });
  zoomLevel.set(level);
}

// ---------------------------------------------------------------------------
// Zoom helpers
// ---------------------------------------------------------------------------

/** Compute items for the zoom-in picker given the current selection and graph. */
function _computePickerItems(g: GraphJson, kind: SelectionKind, currentPath: string[]): PickerItem[] {
  if (kind === 'project') {
    return g.modules.map(mod => ({
      id: mod.id,
      name: mod.name,
      kind: 'module' as SelectionKind,
      newPath: [...currentPath, mod.id],
    }));
  }
  if (kind === 'module') {
    const mod = g.modules.find(m => m.id === get(selection).id);
    if (!mod) return [];
    return mod.functions.map(fn_ => ({
      id: fn_.id,
      name: fn_.name,
      kind: 'function' as SelectionKind,
      newPath: [...currentPath, fn_.id],
    }));
  }
  if (kind === 'function') {
    const sel = get(selection);
    for (const mod of g.modules) {
      const fn_ = mod.functions.find(f => f.id === sel.id);
      if (fn_) {
        return (fn_.steps ?? []).map(step => ({
          id: step.id,
          name: step.name,
          kind: 'step' as SelectionKind,
          newPath: [...currentPath, step.id],
        }));
      }
    }
  }
  return [];
}

export function zoomIn(): void {
  const g = get(graph);
  const sel = get(selection);
  const curPath = get(path);
  const level = get(zoomLevel);

  if (level === 4) return; // already at detail, no-op

  if (!g) return;

  if (level === 0) {
    // project → module
    if (g.modules.length === 0) return;
    if (g.modules.length === 1) {
      const mod = g.modules[0];
      navigateTo([...curPath, mod.id], 'module', mod.id, 1);
    } else {
      const items = _computePickerItems(g, sel.kind as SelectionKind, curPath);
      pickerItems.set(items);
      pickerOpen.set(true);
    }
    return;
  }

  if (level === 1) {
    // module → function
    const mod = g.modules.find(m => m.id === sel.id);
    if (!mod || mod.functions.length === 0) return;
    if (mod.functions.length === 1) {
      const fn_ = mod.functions[0];
      navigateTo([...curPath, fn_.id], 'function', fn_.id, 2);
    } else {
      const items = _computePickerItems(g, 'module', curPath);
      pickerItems.set(items);
      pickerOpen.set(true);
    }
    return;
  }

  if (level === 2) {
    // function → step
    let targetFn: FunctionJson | null = null;
    for (const mod of g.modules) {
      const fn_ = mod.functions.find(f => f.id === sel.id);
      if (fn_) { targetFn = fn_; break; }
    }
    if (!targetFn) return;
    const steps = targetFn.steps ?? [];
    if (steps.length === 0) return; // disabled
    if (steps.length === 1) {
      const step = steps[0];
      navigateTo([...curPath, step.id], 'step', step.id, 4);
    } else {
      const items = _computePickerItems(g, 'function', curPath);
      pickerItems.set(items);
      pickerOpen.set(true);
    }
    return;
  }
}

export function zoomOut(): void {
  const level = get(zoomLevel);
  const curPath = get(path);

  if (level === 0) return; // already at system, no-op

  // Pop the last path segment to go up one level
  const newPath = curPath.slice(0, -1);

  if (newPath.length === 0) {
    // Nothing left — navigate to project root. Use the real graph's
    // project id so this works for both fixture (`project:root`) and the
    // real parser's bare ids (`wallet_service`).
    const g = get(graph);
    if (!g) return;
    const pid = g.project.id;
    navigateTo([pid], 'project', pid, 0);
    return;
  }

  const parentSeg = newPath[newPath.length - 1];
  const kind = _kindForSegment(parentSeg);
  if (kind === 'none') return;
  navigateTo(newPath, kind, parentSeg, stageLevelForKind(kind));
}
