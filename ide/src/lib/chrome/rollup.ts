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
 * Walk path segments and resolve display names from the graph.
 *
 * Segment format is the raw entity id. Two id shapes are supported:
 *
 * 1. **Bare ids** emitted by the real `ail-ui-bridge` parser pipeline
 *    (`wallet_service`, `src`, `src.transfer_money`,
 *    `src.transfer_money.new_balance`). Kind is inferred by looking the id
 *    up against `g.project`, `g.modules`, function, step, types, and errors.
 * 2. **Prefixed ids** used by the in-repo fixtures
 *    (`project:root`, `module:m_wallet`, `function:fn_transfer`,
 *    `step:s_debit`). The `kind:id` shape is honoured for backward compat
 *    with all existing tests.
 *
 * Segments whose lookup fails are skipped silently.
 */
export function breadcrumbs(
  g: GraphJson | null,
  p: string[],
): { kind: string; id: string; name: string }[] {
  if (!g) return [];

  const result: { kind: string; id: string; name: string }[] = [];

  for (const segment of p) {
    let kind = '';
    let id = segment;
    const colonIdx = segment.indexOf(':');
    if (colonIdx !== -1) {
      const prefix = segment.slice(0, colonIdx);
      // Only treat the prefix as a kind tag if it is one of the known kinds —
      // bare ids that happen to contain `:` (none today, but defensive) are
      // resolved by graph lookup below.
      if (
        prefix === 'project' ||
        prefix === 'module' ||
        prefix === 'function' ||
        prefix === 'step' ||
        prefix === 'type' ||
        prefix === 'error'
      ) {
        kind = prefix;
        id = segment.slice(colonIdx + 1);
      }
    }

    let name: string | undefined;

    if (kind === 'project' || (kind === '' && g.project.id === segment)) {
      if (g.project.id === segment) {
        name = g.project.name;
        kind = 'project';
      }
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
    } else if (kind === '') {
      // Bare id — try each entity kind in priority order: module, function, step, type, error.
      const mod = g.modules.find((m) => m.id === segment);
      if (mod) {
        kind = 'module';
        name = mod.name;
      } else {
        outer: for (const m of g.modules) {
          for (const fn_ of m.functions) {
            if (fn_.id === segment) {
              kind = 'function';
              name = fn_.name;
              break outer;
            }
            for (const step of fn_.steps ?? []) {
              if (step.id === segment) {
                kind = 'step';
                name = step.name;
                break outer;
              }
            }
          }
        }
        if (!name) {
          const t = g.types.find((t) => t.id === segment);
          if (t) {
            kind = 'type';
            name = t.name;
          }
        }
        if (!name) {
          const e = g.errors.find((e) => e.id === segment);
          if (e) {
            kind = 'error';
            name = e.name;
          }
        }
      }
    }

    if (name !== undefined) {
      result.push({ kind, id, name });
    }
  }

  return result;
}
