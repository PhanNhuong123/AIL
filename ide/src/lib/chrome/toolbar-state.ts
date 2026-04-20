/**
 * toolbar-state.ts — Toolbar-scoped stores and navigation helpers.
 *
 * History is Toolbar-scoped. Navigator.selectNode writes path/selection directly
 * and is intentionally excluded from the back/forward stack.
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

/** Restore selection from the last segment of a path array. */
function _restoreSelectionFromPath(p: string[]): void {
  if (p.length === 0) {
    selection.set({ kind: 'none', id: null });
    return;
  }
  const last = p[p.length - 1];
  const colonIdx = last.indexOf(':');
  if (colonIdx === -1) {
    selection.set({ kind: 'none', id: null });
    return;
  }
  const kind = last.slice(0, colonIdx) as SelectionKind;
  selection.set({ kind, id: last });
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
  // Restore zoom level from path
  if (parsed.length > 0) {
    const last = parsed[parsed.length - 1];
    const colonIdx = last.indexOf(':');
    if (colonIdx !== -1) {
      const kind = last.slice(0, colonIdx) as SelectionKind;
      zoomLevel.set(stageLevelForKind(kind));
    }
  }
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
  // Restore zoom level from path
  if (parsed.length > 0) {
    const last = parsed[parsed.length - 1];
    const colonIdx = last.indexOf(':');
    if (colonIdx !== -1) {
      const kind = last.slice(0, colonIdx) as SelectionKind;
      zoomLevel.set(stageLevelForKind(kind));
    }
  }
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
    // Nothing left — navigate to project root
    const g = get(graph);
    if (!g) return;
    navigateTo(['project:root'], 'project', 'project:root', 0);
    return;
  }

  const parentSeg = newPath[newPath.length - 1];
  const colonIdx = parentSeg.indexOf(':');
  if (colonIdx === -1) return;
  const kind = parentSeg.slice(0, colonIdx) as SelectionKind;
  const newLevel = stageLevelForKind(kind);
  navigateTo(newPath, kind, parentSeg, newLevel);
}
