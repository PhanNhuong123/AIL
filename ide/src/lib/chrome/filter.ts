import type { GraphJson } from '$lib/types';

// Sentinel meaning "all nodes are visible" (returned when term is empty).
export const ALL: unique symbol = Symbol('ALL');

export type VisibleSet = Set<string> | typeof ALL;

export function isVisible(set: VisibleSet, id: string): boolean {
  if (set === ALL) return true;
  return (set as Set<string>).has(id);
}

/**
 * Two-pass filter that keeps any ancestor of a matching node visible.
 * Returns ALL when term is empty/whitespace-only.
 *
 * Pass 1: mark own-matches (case-insensitive name.includes(term)).
 * Pass 2: walk the tree bottom-up; admit ancestors of matched nodes.
 */
export function filterTree(g: GraphJson | null, term: string): VisibleSet {
  if (!g) return ALL;
  const t = term.trim().toLowerCase();
  if (t === '') return ALL;

  // Pass 1 — collect ids whose own name matches
  const own = new Set<string>();

  const check = (id: string, name: string) => {
    if (name.toLowerCase().includes(t)) own.add(id);
  };

  check(g.project.id, g.project.name);

  for (const mod of g.modules) {
    check(mod.id, mod.name);
    for (const fn_ of mod.functions) {
      check(fn_.id, fn_.name);
      for (const step of fn_.steps ?? []) {
        check(step.id, step.name);
      }
    }
  }

  for (const tp of g.types) {
    check(tp.id, tp.name);
  }

  for (const err of g.errors) {
    check(err.id, err.name);
  }

  if (own.size === 0) return new Set<string>();

  // Pass 2 — admit ancestors of any own-match
  const visible = new Set<string>(own);

  let anyMatch = false;

  for (const mod of g.modules) {
    let modVisible = own.has(mod.id);

    for (const fn_ of mod.functions) {
      let fnVisible = own.has(fn_.id);

      for (const step of fn_.steps ?? []) {
        if (own.has(step.id)) {
          visible.add(step.id);
          fnVisible = true;
        }
      }

      if (fnVisible) {
        visible.add(fn_.id);
        modVisible = true;
      }
    }

    // types and errors belong to project level, not module, but if they matched
    // we still mark the project visible (done below)

    if (modVisible) {
      visible.add(mod.id);
      anyMatch = true;
    }
  }

  // Check types/errors — they are project-level children
  for (const tp of g.types) {
    if (own.has(tp.id)) {
      visible.add(tp.id);
      anyMatch = true;
    }
  }

  for (const err of g.errors) {
    if (own.has(err.id)) {
      visible.add(err.id);
      anyMatch = true;
    }
  }

  if (anyMatch || own.has(g.project.id)) {
    visible.add(g.project.id);
  }

  return visible;
}
