/**
 * lens.ts — Per-lens metric computation for stage cards and rows.
 *
 * Dispatches on canonical `Lens` type (no ActiveLens / pickActiveLens).
 * Lens dispatch: structure | rules | verify | data | tests
 *
 * Stage-head chip helpers derive from frontend GraphJson only.
 * They MUST NOT call computeLensMetrics (invariant 15.6-B).
 */

import type { GraphJson, ModuleJson, FunctionJson, Status, NodeDetail, RelationJson, Lens, FlowNodeJson } from '$lib/types';

export type PillTone = 'ok' | 'warn' | 'fail' | 'muted';

export interface Pill { label: string; tone: PillTone; testid?: string; }

export type Bar =
  | { kind: 'seg'; proven: number; unproven: number; broken: number }
  | { kind: 'dots'; statuses: Status[] }
  | { kind: 'types'; names: string[] }
  | { kind: 'none' };

export interface LensMetrics { pills: Pill[]; bar: Bar; showDescription: boolean; }

export interface HeadChipDatum { label: string; tone: PillTone; testid?: string; }
export interface HeadChipSummary { lens: Lens; chips: HeadChipDatum[]; testid: string; }

// --- Leaf ID collectors ---

function moduleLeafIds(m: ModuleJson): string[] {
  const ids: string[] = [m.id];
  for (const fn of m.functions) { ids.push(fn.id); for (const st of fn.steps ?? []) ids.push(st.id); }
  return ids;
}

function fnLeafIds(fn: FunctionJson): string[] {
  const ids: string[] = [fn.id];
  for (const st of fn.steps ?? []) ids.push(st.id);
  return ids;
}

// --- Member-set helpers ---

function moduleMemberIds(module: ModuleJson): Set<string> {
  const ids = new Set<string>([module.id]);
  for (const fn of module.functions ?? []) { ids.add(fn.id); for (const step of fn.steps ?? []) ids.add(step.id); }
  return ids;
}

function isCrossBoundaryForModule(r: RelationJson, members: Set<string>): boolean {
  return members.has(r.from) !== members.has(r.to);
}

// --- Stats collectors ---

function collectRulesStats(ids: string[], detail: Record<string, NodeDetail>) {
  let rules = 0, unproven = 0, broken = 0;
  for (const id of ids) {
    const d = detail[id]; if (!d) continue;
    const proven = new Set(d.proven);
    for (const r of d.rules) { rules++; if (!proven.has(r.text)) unproven++; }
    if (d.verification && d.verification.ok === false) broken++;
  }
  return { rules, unproven, broken };
}

function collectDataflowStats(ids: string[], detail: Record<string, NodeDetail>) {
  const names: string[] = []; const seen = new Set<string>(); let signals = 0;
  for (const id of ids) {
    const d = detail[id]; if (!d) continue;
    for (const r of d.receives) { signals++; if (!seen.has(r.desc)) { seen.add(r.desc); names.push(r.desc); } }
    for (const r of d.returns)  { signals++; if (!seen.has(r.desc)) { seen.add(r.desc); names.push(r.desc); } }
  }
  return { typeNames: names, signals };
}

// --- Per-lens metric builders ---

function rulesLens(ids: string[], detail: Record<string, NodeDetail>): LensMetrics {
  const s = collectRulesStats(ids, detail);
  const pills: Pill[] = [{ label: `${s.rules} rules`, tone: 'muted', testid: 'rules-count' }];
  if (s.unproven > 0) pills.push({ label: `${s.unproven} unproven`, tone: 'warn', testid: 'rules-unproven' });
  if (s.broken > 0)   pills.push({ label: `${s.broken} broken`,   tone: 'fail', testid: 'rules-broken' });
  const total = s.rules;
  const proven = Math.max(0, total - s.unproven - s.broken);
  const bar: Bar = total === 0 && s.broken === 0 ? { kind: 'none' } : { kind: 'seg', proven, unproven: s.unproven, broken: s.broken };
  return { pills, bar, showDescription: true };
}

function verificationLensFromStatuses(rollup: Status, functionStatuses: Status[]): LensMetrics {
  const total = functionStatuses.length;
  const proven = functionStatuses.filter((s) => s === 'ok').length;
  const head: Pill = rollup === 'fail'
    ? { label: '✗ failing', tone: 'fail', testid: 'verify-failing' }
    : rollup === 'warn'
    ? { label: '⚠ issues',  tone: 'warn', testid: 'verify-issues' }
    : { label: '✓ verified', tone: 'ok',  testid: 'verify-verified' };
  const pills: Pill[] = [head, { label: `${proven}/${total}`, tone: 'muted', testid: 'verify-ratio' }];
  const bar: Bar = total === 0 ? { kind: 'none' } : { kind: 'dots', statuses: functionStatuses };
  return { pills, bar, showDescription: true };
}

function dataflowLens(ids: string[], detail: Record<string, NodeDetail>): LensMetrics {
  const s = collectDataflowStats(ids, detail);
  return {
    pills: [
      { label: `${s.typeNames.length} types`, tone: 'muted', testid: 'dataflow-types' },
      { label: `${s.signals} signals`,        tone: 'muted', testid: 'dataflow-signals' },
    ],
    bar: { kind: 'types', names: s.typeNames.slice(0, 6) },
    showDescription: false,
  };
}

function testsLens(_functionStatuses: Status[]): LensMetrics {
  return { pills: [{ label: '0 tests', tone: 'muted', testid: 'tests-count' }], bar: { kind: 'none' }, showDescription: true };
}

function structureLensForModule(module: ModuleJson, _graph: GraphJson): LensMetrics {
  const functions = module.functions ?? [];
  const stepCount = functions.reduce((acc, f) => acc + (f.steps?.length ?? 0), 0);
  return {
    pills: [
      { label: `${functions.length} fn`,  tone: 'muted', testid: 'structure-fn-count' },
      { label: `${stepCount} steps`,      tone: 'muted', testid: 'structure-step-count' },
    ],
    bar: { kind: 'none' },
    showDescription: true,
  };
}

function structureLensForFunction(fn: FunctionJson, _module: ModuleJson, _graph: GraphJson): LensMetrics {
  const steps = fn.steps ?? [];
  return {
    pills: [{ label: `${steps.length} steps`, tone: 'muted', testid: 'structure-step-count' }],
    bar: { kind: 'none' },
    showDescription: true,
  };
}

// --- Public: compute per-card/row metrics ---

export function computeModuleMetrics(module: ModuleJson, graph: GraphJson, lens: Lens): LensMetrics {
  const ids = moduleLeafIds(module);
  const fnStatuses = module.functions.map((f) => f.status);
  switch (lens) {
    case 'structure': return structureLensForModule(module, graph);
    case 'rules':     return rulesLens(ids, graph.detail);
    case 'verify':    return verificationLensFromStatuses(module.status, fnStatuses);
    case 'data':      return dataflowLens(ids, graph.detail);
    case 'tests':     return testsLens(fnStatuses);
  }
}

export function computeFunctionMetrics(fn: FunctionJson, _module: ModuleJson, graph: GraphJson, lens: Lens): LensMetrics {
  const ids = fnLeafIds(fn);
  const stepStatuses = (fn.steps ?? []).map((s) => s.status);
  switch (lens) {
    case 'structure': return structureLensForFunction(fn, _module, graph);
    case 'rules':     return rulesLens(ids, graph.detail);
    case 'verify':    return verificationLensFromStatuses(fn.status, stepStatuses);
    case 'data':      return dataflowLens(ids, graph.detail);
    case 'tests':     return testsLens(stepStatuses);
  }
}

// --- Public: stage-head chip summaries (frontend-only) ---

export function computeSystemHeadSummary(graph: GraphJson, lens: Lens): HeadChipSummary {
  const testid = `system-head-action-${lens}`;
  switch (lens) {
    case 'structure':
      return { lens, testid, chips: [
        { label: `${graph.externals.length} externals`, tone: 'muted' },
        { label: `${graph.relations.length} relations`, tone: 'muted' },
      ]};
    case 'rules': {
      let totalRules = 0, unproven = 0, broken = 0;
      for (const d of Object.values(graph.detail)) {
        const ps = new Set(d.proven);
        for (const r of d.rules) { totalRules++; if (!ps.has(r.text)) unproven++; }
        if (d.verification && d.verification.ok === false) broken++;
      }
      return { lens, testid, chips: [
        { label: `${totalRules} rules`, tone: 'muted' },
        { label: `${unproven} unproven`, tone: unproven > 0 ? 'warn' : 'muted' },
        { label: `${broken} broken`,    tone: broken > 0 ? 'fail' : 'muted' },
      ]};
    }
    case 'verify': {
      let failing = 0, proven = 0, total = 0;
      for (const m of graph.modules) for (const fn of m.functions) {
        total++; if (fn.status === 'ok') proven++; if (fn.status === 'fail') failing++;
      }
      return { lens, testid, chips: [
        { label: `${failing} failing`, tone: failing > 0 ? 'fail' : 'muted' },
        { label: `${proven}/${total}`, tone: 'muted' },
      ]};
    }
    case 'data': {
      const seen = new Set<string>(); let signals = 0;
      for (const d of Object.values(graph.detail)) {
        for (const r of d.receives) { signals++; seen.add(r.desc); }
        for (const r of d.returns)  { signals++; seen.add(r.desc); }
      }
      return { lens, testid, chips: [
        { label: `${graph.types.length} types`, tone: 'muted' },
        { label: `${signals} signals`,          tone: 'muted' },
      ]};
    }
    case 'tests':
      return { lens, testid, chips: [{ label: '0 tests', tone: 'muted' }] };
  }
}

export function computeSwimNodeHint(
  node: FlowNodeJson,
  lens: Lens,
): { subLabel: string; tone: PillTone } {
  switch (lens) {
    case 'structure':
      return { subLabel: node.kind, tone: 'muted' };
    case 'rules':
      return { subLabel: '', tone: 'muted' };
    case 'verify':
      return node.status === 'ok'
        ? { subLabel: '✓', tone: 'ok' }
        : node.status === 'fail'
        ? { subLabel: '✗', tone: 'fail' }
        : node.status === 'warn'
        ? { subLabel: '⚠', tone: 'warn' }
        : { subLabel: '', tone: 'muted' };
    case 'data':
      return { subLabel: '', tone: 'muted' };
    case 'tests':
      return { subLabel: '0 tests', tone: 'muted' };
  }
}

// --- NodeDetailLensSummary and computeNodeDetailSummary (task 15.9) ---

export interface NodeDetailLensSummary {
  lens: Lens;
  heading: string;
  items: string[];
  tone: PillTone;
}

export function computeNodeDetailSummary(
  detail: NodeDetail | null,
  lens: Lens,
): NodeDetailLensSummary {
  if (detail === null) {
    return { lens, heading: '', items: [], tone: 'muted' };
  }
  switch (lens) {
    case 'structure': {
      const items: string[] = [];
      if (detail.description && detail.description.length > 0) items.push(detail.description);
      for (const r of detail.receives) items.push('← ' + r.name + ': ' + r.desc);
      for (const r of detail.returns) items.push('→ ' + r.name + ': ' + r.desc);
      if (items.length === 0) items.push('No signature data');
      return { lens, heading: 'Signature', items, tone: 'muted' };
    }
    case 'rules': {
      const items: string[] = [];
      for (const r of detail.rules) items.push(r.text);
      for (const r of detail.inherited) items.push(r.text + ' (from ' + r.from + ')');
      if (items.length === 0) items.push('No rules defined');
      const total = detail.rules.length + detail.inherited.length;
      return { lens, heading: 'Rules (' + total + ')', items, tone: 'warn' };
    }
    case 'verify': {
      const items: string[] = [];
      if (detail.verification.ok) {
        for (const p of detail.proven) items.push('✓ ' + p);
        if (items.length === 0) items.push('Verified');
        return { lens, heading: 'Verification', items, tone: 'ok' };
      }
      const ce = detail.verification.counterexample;
      if (ce) {
        items.push('Scenario: ' + ce.scenario);
        items.push('Effect: ' + ce.effect);
        items.push('Violates: ' + ce.violates);
      } else {
        items.push('Not verified');
      }
      return { lens, heading: 'Verification', items, tone: 'fail' };
    }
    case 'data': {
      const recNames = detail.receives.map((r) => r.name).join(', ');
      const retNames = detail.returns.map((r) => r.name).join(', ');
      const items = [
        'Receives: ' + (recNames.length > 0 ? recNames : 'none'),
        'Returns: ' + (retNames.length > 0 ? retNames : 'none'),
      ];
      return { lens, heading: 'Data Flow', items, tone: 'ok' };
    }
    case 'tests': {
      return {
        lens,
        heading: 'Tests',
        items: ['No test field on NodeDetail — attach test results via agent (Phase 16/17)'],
        tone: 'muted',
      };
    }
  }
}

export function computeModuleHeadSummary(module: ModuleJson, graph: GraphJson, lens: Lens): HeadChipSummary {
  const testid = `module-head-action-${lens}`;
  const members = moduleMemberIds(module);
  const crossCount = graph.relations.filter((r) => isCrossBoundaryForModule(r, members)).length;
  switch (lens) {
    case 'structure': {
      const fnCount = module.functions.length;
      const stepCount = module.functions.reduce((acc, f) => acc + (f.steps?.length ?? 0), 0);
      return { lens, testid, chips: [
        { label: `${fnCount} fn`,         tone: 'muted' },
        { label: `${stepCount} steps`,    tone: 'muted' },
        { label: `${crossCount} relations`, tone: 'muted' },
      ]};
    }
    case 'rules': {
      const s = collectRulesStats(moduleLeafIds(module), graph.detail);
      return { lens, testid, chips: [
        { label: `${s.rules} rules`,     tone: 'muted' },
        { label: `${s.unproven} unproven`, tone: s.unproven > 0 ? 'warn' : 'muted' },
        { label: `${s.broken} broken`,   tone: s.broken > 0 ? 'fail' : 'muted' },
      ]};
    }
    case 'verify': {
      const fnStatuses = module.functions.map((f) => f.status);
      const total = fnStatuses.length;
      const proven = fnStatuses.filter((s) => s === 'ok').length;
      const head: HeadChipDatum = module.status === 'fail'
        ? { label: '✗ failing', tone: 'fail' }
        : module.status === 'warn'
        ? { label: '⚠ issues',  tone: 'warn' }
        : { label: '✓ verified', tone: 'ok' };
      return { lens, testid, chips: [head, { label: `${proven}/${total}`, tone: 'muted' }] };
    }
    case 'data': {
      const s = collectDataflowStats(moduleLeafIds(module), graph.detail);
      return { lens, testid, chips: [
        { label: `${s.typeNames.length} types`, tone: 'muted' },
        { label: `${s.signals} signals`,        tone: 'muted' },
      ]};
    }
    case 'tests':
      return { lens, testid, chips: [{ label: '0 tests', tone: 'muted' }] };
  }
}
