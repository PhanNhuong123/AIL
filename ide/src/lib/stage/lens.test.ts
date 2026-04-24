import { describe, it, expect } from 'vitest';
import {
  computeModuleMetrics,
  computeFunctionMetrics,
  computeSystemHeadSummary,
  computeModuleHeadSummary,
  computeSwimNodeHint,
  computeNodeDetailSummary,
} from './lens';
import { multiClusterFixture, detailedModuleFixture, nodeDetailFixture } from './fixtures';
import { walletFixture } from '$lib/chrome/fixtures';
import type { Lens, ModuleJson, FlowNodeJson, NodeDetail } from '$lib/types';
import type { PillTone } from './lens';

function moduleById(g: ReturnType<typeof multiClusterFixture>, id: string): ModuleJson {
  const m = g.modules.find((x) => x.id === id);
  if (!m) throw new Error(`module ${id} missing`);
  return m;
}

describe('lens.ts', () => {
  // --- structure lens ---

  it('test_compute_module_metrics_structure_lens', () => {
    // 2 functions: fn_transfer (2 steps) + fn_balance (1 step) = 3 total steps
    const g = multiClusterFixture();
    const m = moduleById(g, 'module:m_wallet');
    const metrics = computeModuleMetrics(m, g, 'structure');
    expect(metrics.pills.length).toBe(2);
    expect(metrics.pills[0].label).toBe('2 fn');
    expect(metrics.pills[1].label).toBe('3 steps');
    expect(metrics.bar.kind).toBe('none');
    expect(metrics.showDescription).toBe(true);
  });

  it('test_compute_function_metrics_structure_lens', () => {
    const g = multiClusterFixture();
    const m = moduleById(g, 'module:m_wallet');
    const fn = m.functions.find((f) => f.id === 'function:fn_transfer')!;
    // fn_transfer has 2 steps: s_debit, s_credit
    const metrics = computeFunctionMetrics(fn, m, g, 'structure');
    expect(metrics.pills[0].label).toBe('2 steps');
    expect(metrics.bar.kind).toBe('none');
  });

  // --- rules lens ---

  it('test_compute_module_metrics_rules_lens', () => {
    const g = multiClusterFixture();
    const m = moduleById(g, 'module:m_billing');
    const metrics = computeModuleMetrics(m, g, 'rules');
    const labels = metrics.pills.map((p) => p.label);
    expect(labels[0]).toBe('2 rules');
    expect(labels).not.toContain('0 unproven');
    expect(metrics.bar.kind).toBe('seg');
    if (metrics.bar.kind === 'seg') {
      expect(metrics.bar.proven).toBe(2);
      expect(metrics.bar.unproven).toBe(0);
      expect(metrics.bar.broken).toBe(0);
    }
  });

  // --- verify lens ---

  it('test_compute_module_metrics_verify_lens', () => {
    const g = multiClusterFixture();
    const m = moduleById(g, 'module:m_wallet');
    const metrics = computeModuleMetrics(m, g, 'verify');
    const head = metrics.pills[0];
    expect(head.tone).toBe('warn');
    expect(head.label).toBe('⚠ issues');
    expect(metrics.pills[1].label).toBe('1/2');
    expect(metrics.bar.kind).toBe('dots');
    if (metrics.bar.kind === 'dots') {
      expect(metrics.bar.statuses.length).toBe(2);
    }
  });

  // --- data lens ---

  it('test_compute_module_metrics_data_lens', () => {
    const g = detailedModuleFixture();
    const m = moduleById(g, 'module:m_wallet');
    const metrics = computeModuleMetrics(m, g, 'data');
    const labels = metrics.pills.map((p) => p.label);
    expect(labels[0]).toBe('3 types');
    expect(labels[1]).toBe('5 signals');
    expect(metrics.bar.kind).toBe('types');
    if (metrics.bar.kind === 'types') {
      expect(metrics.bar.names).toContain('Account');
    }
  });

  // --- tests lens ---

  it('test_compute_module_metrics_tests_lens', () => {
    const g = multiClusterFixture();
    const m = moduleById(g, 'module:m_wallet');
    const metrics = computeModuleMetrics(m, g, 'tests');
    expect(metrics.pills[0].label).toBe('0 tests');
    expect(metrics.pills[0].tone).toBe('muted');
    expect(metrics.bar.kind).toBe('none');
  });

  // --- all lenses for functions ---

  it('test_compute_function_metrics_all_lenses', () => {
    const lenses: Lens[] = ['structure', 'rules', 'verify', 'data', 'tests'];
    const g = multiClusterFixture();
    const m = moduleById(g, 'module:m_wallet');
    const fn = m.functions[0];
    for (const lens of lenses) {
      const metrics = computeFunctionMetrics(fn, m, g, lens);
      expect(metrics.pills.length).toBeGreaterThan(0);
      expect(metrics.bar.kind).toBeDefined();
    }
  });

  // --- empty detail fallback ---

  it('test_empty_detail_falls_back_to_zero_pills', () => {
    const g = walletFixture();
    const m = g.modules.find((x) => x.id === 'module:m_wallet')!;

    const rulesMetrics = computeModuleMetrics(m, g, 'rules');
    expect(rulesMetrics.pills[0].label).toBe('0 rules');
    expect(rulesMetrics.bar.kind).toBe('none');

    const dfMetrics = computeModuleMetrics(m, g, 'data');
    expect(dfMetrics.pills.map((p) => p.label)).toEqual(['0 types', '0 signals']);
    expect(dfMetrics.bar.kind).toBe('types');
    if (dfMetrics.bar.kind === 'types') {
      expect(dfMetrics.bar.names).toEqual([]);
    }

    const fn = m.functions[0];
    const fnMetrics = computeFunctionMetrics(fn, m, g, 'rules');
    expect(fnMetrics.pills[0].label).toBe('0 rules');
  });

  // --- computeSystemHeadSummary ---

  it('test_compute_system_head_summary_varies_by_lens', () => {
    const lenses: Lens[] = ['structure', 'rules', 'verify', 'data', 'tests'];
    const g = multiClusterFixture();
    for (const lens of lenses) {
      const s = computeSystemHeadSummary(g, lens);
      expect(s.testid).toBe(`system-head-action-${lens}`);
      expect(s.chips.length).toBeGreaterThan(0);
    }
    // structure specifically has externals and relations labels
    const structS = computeSystemHeadSummary(g, 'structure');
    expect(structS.chips.some((c) => /externals/.test(c.label))).toBe(true);
    expect(structS.chips.some((c) => /relations/.test(c.label))).toBe(true);
  });

  // --- computeModuleHeadSummary structure includes relations ---

  it('test_compute_module_head_summary_structure_includes_relations', () => {
    const g = multiClusterFixture();
    // m_wallet has cross-boundary edges: m_wallet→m_billing (cross), m_rewards→m_wallet (cross)
    // m_billing→ext_stripe is cross-boundary for m_billing, not m_wallet
    const m = moduleById(g, 'module:m_wallet');
    const s = computeModuleHeadSummary(m, g, 'structure');
    expect(s.chips.some((c) => /\d+ relations/.test(c.label))).toBe(true);
    // m_wallet is in 2 cross-boundary relations
    const relChip = s.chips.find((c) => /relations/.test(c.label))!;
    expect(relChip.label).toBe('2 relations');
  });

  // --- computeModuleHeadSummary verify scope ---

  it('test_compute_module_head_summary_verify_scope', () => {
    const g = multiClusterFixture();
    const m = moduleById(g, 'module:m_wallet'); // status 'warn'
    const s = computeModuleHeadSummary(m, g, 'verify');
    const labels = s.chips.map((c) => c.label);
    const hasIssuesOrFailing = labels.some((l) => /issues|failing/.test(l));
    expect(hasIssuesOrFailing).toBe(true);
  });
});

describe('computeSwimNodeHint', () => {
  it('test_compute_swim_node_hint_structure', () => {
    const node: FlowNodeJson = { id: 'x', kind: 'process', label: 'Do', x: 0, y: 0 };
    const r = computeSwimNodeHint(node, 'structure');
    expect(r).toEqual({ subLabel: 'process', tone: 'muted' });
  });

  it('test_compute_swim_node_hint_verify_status_variants', () => {
    const okNode: FlowNodeJson = { id: 'a', kind: 'process', label: 'A', x: 0, y: 0, status: 'ok' };
    const warnNode: FlowNodeJson = { id: 'b', kind: 'decision', label: 'B', x: 0, y: 0, status: 'warn' };
    const failNode: FlowNodeJson = { id: 'c', kind: 'io', label: 'C', x: 0, y: 0, status: 'fail' };
    const noStatusNode: FlowNodeJson = { id: 'd', kind: 'start', label: 'D', x: 0, y: 0 };
    expect(computeSwimNodeHint(okNode, 'verify')).toEqual({ subLabel: '✓', tone: 'ok' });
    expect(computeSwimNodeHint(warnNode, 'verify')).toEqual({ subLabel: '⚠', tone: 'warn' });
    expect(computeSwimNodeHint(failNode, 'verify')).toEqual({ subLabel: '✗', tone: 'fail' });
    expect(computeSwimNodeHint(noStatusNode, 'verify')).toEqual({ subLabel: '', tone: 'muted' });
  });

  it('test_compute_swim_node_hint_tests', () => {
    const node: FlowNodeJson = { id: 'x', kind: 'process', label: 'Do', x: 0, y: 0, status: 'fail' };
    expect(computeSwimNodeHint(node, 'tests')).toEqual({ subLabel: '0 tests', tone: 'muted' });
  });

  it('test_compute_swim_node_hint_rules_data_are_muted', () => {
    const node: FlowNodeJson = { id: 'x', kind: 'process', label: 'Do', x: 0, y: 0, status: 'fail' };
    expect(computeSwimNodeHint(node, 'rules')).toEqual({ subLabel: '', tone: 'muted' });
    expect(computeSwimNodeHint(node, 'data')).toEqual({ subLabel: '', tone: 'muted' });
  });
});

describe('computeNodeDetailSummary', () => {
  it('returns non-empty items + correct tone for all 5 lenses', () => {
    const detail = nodeDetailFixture();
    const cases: Array<{ lens: Lens; headingIncludes: string; minItems: number; tone: PillTone }> = [
      { lens: 'structure', headingIncludes: 'Signature',    minItems: 1, tone: 'muted' },
      { lens: 'rules',     headingIncludes: 'Rules',        minItems: 1, tone: 'warn'  },
      { lens: 'verify',    headingIncludes: 'Verification', minItems: 1, tone: detail.verification.ok ? 'ok' : 'fail' },
      { lens: 'data',      headingIncludes: 'Data Flow',    minItems: 2, tone: 'ok'    },
      { lens: 'tests',     headingIncludes: 'Tests',        minItems: 1, tone: 'muted' },
    ];
    for (const c of cases) {
      const s = computeNodeDetailSummary(detail, c.lens);
      expect(s.lens).toBe(c.lens);
      expect(s.heading).toContain(c.headingIncludes);
      expect(s.items.length).toBeGreaterThanOrEqual(c.minItems);
      expect(s.tone).toBe(c.tone);
    }
  });

  it('null detail returns empty items for every lens', () => {
    for (const lens of ['structure', 'rules', 'verify', 'data', 'tests'] as const) {
      const s = computeNodeDetailSummary(null, lens);
      expect(s.lens).toBe(lens);
      expect(s.heading).toBe('');
      expect(s.items).toEqual([]);
      expect(s.tone).toBe('muted');
    }
  });

  it('verify lens with counterexample includes scenario/effect/violates', () => {
    const base = nodeDetailFixture();
    const detail: NodeDetail = {
      ...base,
      verification: {
        ok: false,
        counterexample: { scenario: 'S1', effect: 'E1', violates: 'V1' },
      },
    };
    const s = computeNodeDetailSummary(detail, 'verify');
    expect(s.tone).toBe('fail');
    const joined = s.items.join(' | ');
    expect(joined).toContain('Scenario: S1');
    expect(joined).toContain('Effect: E1');
    expect(joined).toContain('Violates: V1');
  });
});
