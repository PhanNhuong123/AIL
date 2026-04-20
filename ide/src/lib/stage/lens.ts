/**
 * lens.ts — Per-overlay metric computation.
 *
 * Addresses 16.5-A: each overlay produces its own Pill vocabulary and Bar shape.
 *
 * Overlay priority when multiple are active:
 *   rules > verification > dataflow > tests > dependencies > (default) verification
 *
 * Outputs fall back to zero-valued pills when GraphJson.detail is empty —
 * never throw.
 */

import type { GraphJson, ModuleJson, FunctionJson, Status, NodeDetail } from '$lib/types';
import type { Overlays } from '$lib/stores';

export type PillTone = 'ok' | 'warn' | 'fail' | 'muted';

export interface Pill {
  label: string;
  tone: PillTone;
  testid?: string;
}

export type Bar =
  | { kind: 'seg'; proven: number; unproven: number; broken: number }
  | { kind: 'dots'; statuses: Status[] }
  | { kind: 'types'; names: string[] }
  | { kind: 'none' };

export interface LensMetrics {
  pills: Pill[];
  bar: Bar;
  showDescription: boolean;
}

export type ActiveLens = 'rules' | 'verification' | 'dataflow' | 'tests' | 'dependencies';

export function pickActiveLens(o: Overlays): ActiveLens {
  if (o.rules) return 'rules';
  if (o.verification) return 'verification';
  if (o.dataflow) return 'dataflow';
  if (o.tests) return 'tests';
  if (o.dependencies) return 'dependencies';
  return 'verification';
}

function moduleLeafIds(m: ModuleJson): string[] {
  const ids: string[] = [m.id];
  for (const fn of m.functions) {
    ids.push(fn.id);
    for (const st of fn.steps ?? []) ids.push(st.id);
  }
  return ids;
}

function fnLeafIds(fn: FunctionJson): string[] {
  const ids: string[] = [fn.id];
  for (const st of fn.steps ?? []) ids.push(st.id);
  return ids;
}

function collectRulesStats(
  ids: string[],
  detail: Record<string, NodeDetail>,
): { rules: number; unproven: number; broken: number } {
  let rules = 0;
  let unproven = 0;
  let broken = 0;
  for (const id of ids) {
    const d = detail[id];
    if (!d) continue;
    const proven = new Set(d.proven);
    for (const r of d.rules) {
      rules++;
      if (!proven.has(r.text)) unproven++;
    }
    if (d.verification && d.verification.ok === false) broken++;
  }
  return { rules, unproven, broken };
}

function collectDataflowStats(
  ids: string[],
  detail: Record<string, NodeDetail>,
): { typeNames: string[]; signals: number } {
  const names: string[] = [];
  const seen = new Set<string>();
  let signals = 0;
  for (const id of ids) {
    const d = detail[id];
    if (!d) continue;
    for (const r of d.receives) {
      signals++;
      if (!seen.has(r.desc)) { seen.add(r.desc); names.push(r.desc); }
    }
    for (const r of d.returns) {
      signals++;
      if (!seen.has(r.desc)) { seen.add(r.desc); names.push(r.desc); }
    }
  }
  return { typeNames: names, signals };
}

function rulesLens(ids: string[], detail: Record<string, NodeDetail>): LensMetrics {
  const s = collectRulesStats(ids, detail);
  const pills: Pill[] = [
    { label: `${s.rules} rules`, tone: 'muted', testid: 'rules-count' },
  ];
  if (s.unproven > 0) {
    pills.push({ label: `${s.unproven} unproven`, tone: 'warn', testid: 'rules-unproven' });
  }
  if (s.broken > 0) {
    pills.push({ label: `${s.broken} broken`, tone: 'fail', testid: 'rules-broken' });
  }
  const total = s.rules;
  const proven = Math.max(0, total - s.unproven - s.broken);
  const bar: Bar =
    total === 0 && s.broken === 0
      ? { kind: 'none' }
      : { kind: 'seg', proven, unproven: s.unproven, broken: s.broken };
  return { pills, bar, showDescription: true };
}

function verificationLensFromStatuses(
  rollup: Status,
  functionStatuses: Status[],
): LensMetrics {
  const total = functionStatuses.length;
  const proven = functionStatuses.filter((s) => s === 'ok').length;
  let head: Pill;
  if (rollup === 'fail') {
    head = { label: '✗ failing', tone: 'fail', testid: 'verify-failing' };
  } else if (rollup === 'warn') {
    head = { label: '⚠ issues', tone: 'warn', testid: 'verify-issues' };
  } else {
    head = { label: '✓ verified', tone: 'ok', testid: 'verify-verified' };
  }
  const pills: Pill[] = [
    head,
    { label: `${proven}/${total}`, tone: 'muted', testid: 'verify-ratio' },
  ];
  const bar: Bar = total === 0 ? { kind: 'none' } : { kind: 'dots', statuses: functionStatuses };
  return { pills, bar, showDescription: true };
}

function dataflowLens(ids: string[], detail: Record<string, NodeDetail>): LensMetrics {
  const s = collectDataflowStats(ids, detail);
  const typeCount = s.typeNames.length;
  const pills: Pill[] = [
    { label: `${typeCount} types`, tone: 'muted', testid: 'dataflow-types' },
    { label: `${s.signals} signals`, tone: 'muted', testid: 'dataflow-signals' },
  ];
  const bar: Bar = { kind: 'types', names: s.typeNames.slice(0, 6) };
  return { pills, bar, showDescription: false };
}

function testsLens(_functionStatuses: Status[]): LensMetrics {
  // Tests telemetry isn't wired yet. Emit distinct vocabulary from the
  // verification lens — a 'none' bar avoids mirroring verification statuses
  // (invariant 16.5-A: each overlay must use distinct vocabulary).
  const pills: Pill[] = [
    { label: '0 tests', tone: 'muted', testid: 'tests-count' },
  ];
  return { pills, bar: { kind: 'none' }, showDescription: true };
}

function dependenciesLensForModule(module: ModuleJson, graph: GraphJson): LensMetrics {
  const moduleMemberIds = new Set<string>([module.id]);
  for (const fn of module.functions) {
    moduleMemberIds.add(fn.id);
    for (const st of fn.steps ?? []) moduleMemberIds.add(st.id);
  }
  const externalIds = new Set(graph.externals.map((e) => e.id));

  let uses = 0;
  let externalsHit = 0;
  for (const rel of graph.relations) {
    const fromMember = moduleMemberIds.has(rel.from);
    const toMember = moduleMemberIds.has(rel.to);
    // Only cross-boundary edges count: intra-module edges are internal wiring,
    // not dependencies. `fromMember !== toMember` keeps the XOR cases.
    if (fromMember === toMember) continue;
    uses++;
    // Count externals only when the module reaches OUT to an external,
    // not when an external (hypothetically) references the module.
    if (fromMember && externalIds.has(rel.to)) externalsHit++;
  }
  const pills: Pill[] = [{ label: `${uses} uses`, tone: 'muted', testid: 'deps-uses' }];
  if (externalsHit > 0) {
    pills.push({ label: `${externalsHit} externals`, tone: 'muted', testid: 'deps-externals' });
  }
  return { pills, bar: { kind: 'none' }, showDescription: true };
}

function dependenciesLensForFunction(fn: FunctionJson, graph: GraphJson): LensMetrics {
  const memberIds = new Set<string>([fn.id]);
  for (const st of fn.steps ?? []) memberIds.add(st.id);
  let uses = 0;
  for (const rel of graph.relations) {
    const fromMember = memberIds.has(rel.from);
    const toMember = memberIds.has(rel.to);
    if (fromMember === toMember) continue;
    uses++;
  }
  const pills: Pill[] = [{ label: `${uses} uses`, tone: 'muted', testid: 'deps-uses' }];
  return { pills, bar: { kind: 'none' }, showDescription: true };
}

export function computeModuleMetrics(
  module: ModuleJson,
  graph: GraphJson,
  overlays: Overlays,
): LensMetrics {
  const lens = pickActiveLens(overlays);
  const ids = moduleLeafIds(module);
  const fnStatuses = module.functions.map((f) => f.status);
  switch (lens) {
    case 'rules':         return rulesLens(ids, graph.detail);
    case 'verification':  return verificationLensFromStatuses(module.status, fnStatuses);
    case 'dataflow':      return dataflowLens(ids, graph.detail);
    case 'tests':         return testsLens(fnStatuses);
    case 'dependencies':  return dependenciesLensForModule(module, graph);
  }
}

export function computeFunctionMetrics(
  fn: FunctionJson,
  _module: ModuleJson,
  graph: GraphJson,
  overlays: Overlays,
): LensMetrics {
  const lens = pickActiveLens(overlays);
  const ids = fnLeafIds(fn);
  const stepStatuses = (fn.steps ?? []).map((s) => s.status);
  switch (lens) {
    case 'rules':         return rulesLens(ids, graph.detail);
    case 'verification':  return verificationLensFromStatuses(fn.status, stepStatuses);
    case 'dataflow':      return dataflowLens(ids, graph.detail);
    case 'tests':         return testsLens(stepStatuses);
    case 'dependencies':  return dependenciesLensForFunction(fn, graph);
  }
}
