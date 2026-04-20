import type { GraphJson, Status } from '$lib/types';

/** worst-wins: ok < warn < fail */
export function rollupStatus(ss: Status[]): Status {
  if (ss.includes('fail')) return 'fail';
  if (ss.includes('warn')) return 'warn';
  return 'ok';
}

/**
 * Leaf-step pill counting (GAP-2):
 * - For each module → each function:
 *   - If function has steps (steps.length > 0): each step is a leaf.
 *     status==='ok' → verified++, else issues++
 *   - If function has no steps: the function itself is the leaf.
 *     status==='ok' → verified++, else issues++
 * - Types: status !== 'ok' → issues++
 * - Errors: status !== 'ok' → issues++
 * - Modules are NOT counted (they are rollups, not leaves).
 */
export function countPills(g: GraphJson | null): { verified: number; issues: number } {
  if (!g) return { verified: 0, issues: 0 };

  let verified = 0;
  let issues = 0;

  for (const mod of g.modules) {
    for (const fn_ of mod.functions) {
      const steps = fn_.steps ?? [];
      if (steps.length > 0) {
        for (const step of steps) {
          if (step.status === 'ok') {
            verified++;
          } else {
            issues++;
          }
        }
      } else {
        // function itself is the leaf
        if (fn_.status === 'ok') {
          verified++;
        } else {
          issues++;
        }
      }
    }
  }

  for (const tp of g.types) {
    if (tp.status !== 'ok') issues++;
  }

  for (const err of g.errors) {
    if (err.status !== 'ok') issues++;
  }

  return { verified, issues };
}

/**
 * Walk path segments (each "kind:id") and resolve display names from the graph.
 * Skips segments whose lookup fails.
 */
export function breadcrumbs(
  g: GraphJson | null,
  p: string[],
): { kind: string; id: string; name: string }[] {
  if (!g) return [];

  const result: { kind: string; id: string; name: string }[] = [];

  for (const segment of p) {
    const colonIdx = segment.indexOf(':');
    if (colonIdx === -1) continue;
    const kind = segment.slice(0, colonIdx);
    const id = segment.slice(colonIdx + 1);

    let name: string | undefined;

    if (kind === 'project') {
      if (g.project.id === segment) name = g.project.name;
    } else if (kind === 'module') {
      name = g.modules.find((m) => m.id === segment)?.name;
    } else if (kind === 'function') {
      outer: for (const mod of g.modules) {
        for (const fn_ of mod.functions) {
          if (fn_.id === segment) {
            name = fn_.name;
            break outer;
          }
        }
      }
    } else if (kind === 'step') {
      outer: for (const mod of g.modules) {
        for (const fn_ of mod.functions) {
          for (const step of fn_.steps ?? []) {
            if (step.id === segment) {
              name = step.name;
              break outer;
            }
          }
        }
      }
    } else if (kind === 'type') {
      name = g.types.find((t) => t.id === segment)?.name;
    } else if (kind === 'error') {
      name = g.errors.find((e) => e.id === segment)?.name;
    }

    if (name !== undefined) {
      result.push({ kind, id, name });
    }
  }

  return result;
}
