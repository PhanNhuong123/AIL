import { describe, it, expect } from 'vitest';
import {
  computeModuleMetrics,
  computeFunctionMetrics,
  pickActiveLens,
} from './lens';
import { multiClusterFixture, detailedModuleFixture } from './fixtures';
import { walletFixture } from '$lib/chrome/fixtures';
import type { Overlays } from '$lib/stores';
import type { ModuleJson } from '$lib/types';

function ov(partial: Partial<Overlays>): Overlays {
  return {
    rules: false,
    verification: false,
    dataflow: false,
    dependencies: false,
    tests: false,
    ...partial,
  };
}

function moduleById(g: ReturnType<typeof multiClusterFixture>, id: string): ModuleJson {
  const m = g.modules.find((x) => x.id === id);
  if (!m) throw new Error(`module ${id} missing`);
  return m;
}

describe('lens.ts', () => {
  it('test_rules_overlay_returns_rules_pills_and_seg_bar', () => {
    const g = multiClusterFixture();
    const m = moduleById(g, 'module:m_billing'); // detail has 2 rules, both proven, verification ok
    const metrics = computeModuleMetrics(m, g, ov({ rules: true }));
    const labels = metrics.pills.map((p) => p.label);
    expect(labels[0]).toBe('2 rules');
    // All 2 proven, none unproven, none broken → no warn/fail pills emitted.
    expect(labels).not.toContain('0 unproven');
    expect(metrics.bar.kind).toBe('seg');
    if (metrics.bar.kind === 'seg') {
      expect(metrics.bar.proven).toBe(2);
      expect(metrics.bar.unproven).toBe(0);
      expect(metrics.bar.broken).toBe(0);
    }
  });

  it('test_verification_overlay_returns_verify_pills_and_dots_bar', () => {
    const g = multiClusterFixture();
    const m = moduleById(g, 'module:m_wallet'); // warn status, 2 functions
    const metrics = computeModuleMetrics(m, g, ov({ verification: true }));
    const head = metrics.pills[0];
    expect(head.tone).toBe('warn');
    expect(head.label).toBe('⚠ issues');
    expect(metrics.pills[1].label).toBe('1/2');
    expect(metrics.bar.kind).toBe('dots');
    if (metrics.bar.kind === 'dots') {
      expect(metrics.bar.statuses.length).toBe(2);
    }
  });

  it('test_dataflow_overlay_returns_types_and_signals_pills_and_types_bar', () => {
    const g = detailedModuleFixture();
    const m = moduleById(g, 'module:m_wallet');
    const metrics = computeModuleMetrics(m, g, ov({ dataflow: true }));
    const labels = metrics.pills.map((p) => p.label);
    // Wallet detail: receives 1, returns 1 → 2 signals, 2 distinct type names.
    // Transfer detail nested under the module: 2 receives + 1 return = 3 more signals,
    // two of which reuse `Account` (already seen). So totals: 5 signals, names:
    // ["Account","WalletState","TxReceipt"] (3 distinct).
    expect(labels[0]).toBe('3 types');
    expect(labels[1]).toBe('5 signals');
    expect(metrics.bar.kind).toBe('types');
    if (metrics.bar.kind === 'types') {
      expect(metrics.bar.names).toContain('Account');
    }
  });

  it('test_dependencies_overlay_returns_uses_pill_and_no_bar', () => {
    const g = multiClusterFixture();
    const m = moduleById(g, 'module:m_wallet');
    const metrics = computeModuleMetrics(m, g, ov({ dependencies: true }));
    const labels = metrics.pills.map((p) => p.label);
    // m_wallet participates in: m_wallet→m_billing (uses), m_rewards→m_wallet (uses) = 2 uses
    expect(labels).toContain('2 uses');
    expect(metrics.bar.kind).toBe('none');
  });

  it('test_tests_overlay_returns_placeholder_pills', () => {
    const g = multiClusterFixture();
    const m = moduleById(g, 'module:m_wallet');
    const metrics = computeModuleMetrics(m, g, ov({ tests: true }));
    expect(metrics.pills[0].label).toBe('0 tests');
    expect(metrics.pills[0].tone).toBe('muted');
    // Bar stays 'none' so the tests lens does not mirror verification-lens dots.
    expect(metrics.bar.kind).toBe('none');
  });

  it('test_empty_detail_falls_back_to_zero_pills', () => {
    // walletFixture.detail is empty — exercises rules/dataflow fallback to zeros.
    const g = walletFixture();
    const m = g.modules.find((x) => x.id === 'module:m_wallet')!;

    const rulesMetrics = computeModuleMetrics(m, g, ov({ rules: true }));
    expect(rulesMetrics.pills[0].label).toBe('0 rules');
    expect(rulesMetrics.bar.kind).toBe('none');

    const dfMetrics = computeModuleMetrics(m, g, ov({ dataflow: true }));
    expect(dfMetrics.pills.map((p) => p.label)).toEqual(['0 types', '0 signals']);
    expect(dfMetrics.bar.kind).toBe('types');
    if (dfMetrics.bar.kind === 'types') {
      expect(dfMetrics.bar.names).toEqual([]);
    }

    // Function-level fallback
    const fn = m.functions[0];
    const fnMetrics = computeFunctionMetrics(fn, m, g, ov({ rules: true }));
    expect(fnMetrics.pills[0].label).toBe('0 rules');
  });

  it('test_overlay_priority_rules_beats_verification', () => {
    expect(pickActiveLens(ov({ rules: true, verification: true }))).toBe('rules');
    expect(pickActiveLens(ov({ verification: true }))).toBe('verification');
    expect(pickActiveLens(ov({ dataflow: true, verification: true }))).toBe('verification');
    expect(pickActiveLens(ov({ dataflow: true }))).toBe('dataflow');
    expect(pickActiveLens(ov({ tests: true }))).toBe('tests');
    expect(pickActiveLens(ov({ dependencies: true }))).toBe('dependencies');
    // No overlays active → default verification
    expect(pickActiveLens(ov({}))).toBe('verification');
  });
});
