import { describe, test, expect } from 'vitest';
import {
  computeModulePositions,
  filterRelations,
  badgeFor,
  SG_VIEWBOX,
} from './system-graph-layout';

describe('computeModulePositions', () => {
  test('is deterministic for the same input', () => {
    const modules = [
      { id: 'm_a', cluster: 'c1', clusterColor: '#fff' },
      { id: 'm_b', cluster: 'c1', clusterColor: '#fff' },
      { id: 'm_c', cluster: 'c2', clusterColor: '#000' },
    ];
    const clusters = [
      { id: 'c1', color: '#fff', name: 'Cluster1' },
      { id: 'c2', color: '#000', name: 'Cluster2' },
    ];
    const a = computeModulePositions(modules, clusters);
    const b = computeModulePositions(modules, clusters);
    expect(a).toEqual(b);
  });

  test('handles zero clusters by placing modules on a single fallback ring', () => {
    const modules = [
      { id: 'm_a' },
      { id: 'm_b' },
      { id: 'm_c' },
    ];
    const result = computeModulePositions(modules, []);
    expect(result.clusterPositions).toEqual([]);
    expect(result.modulePositions).toHaveLength(3);
    // All modules should orbit (cx, cy) — at distance ~SG_RING_R
    for (const p of result.modulePositions) {
      const dx = p.x - SG_VIEWBOX.cx;
      const dy = p.y - SG_VIEWBOX.cy;
      const dist = Math.hypot(dx, dy);
      expect(Math.round(dist)).toBeGreaterThan(100);
    }
  });

  test('handles zero modules by returning empty arrays', () => {
    const result = computeModulePositions([], [{ id: 'c1', color: '#fff' }]);
    expect(result.modulePositions).toEqual([]);
    expect(result.clusterPositions).toHaveLength(1);
  });
});

describe('filterRelations', () => {
  const relations = [
    { from: 'a', to: 'b', style: 'sync' },
    { from: 'a', to: 'c', style: 'async' },
    { from: 'b', to: 'c', style: 'data' },
  ];

  test('data lens keeps only data-style edges', () => {
    const edges = filterRelations(relations, 'data');
    expect(edges).toHaveLength(1);
    expect(edges[0].style).toBe('data');
  });

  test('rules lens hides async edges', () => {
    const edges = filterRelations(relations, 'rules');
    expect(edges.every((e) => e.style !== 'async')).toBe(true);
    expect(edges).toHaveLength(2);
  });
});

describe('badgeFor', () => {
  test('returns lens-appropriate badge', () => {
    const m = { id: 'm', functions: [{ status: 'ok' }, { status: 'fail' }, { status: 'ok' }] };
    expect(badgeFor(m, 'verify').tone).toBe('fail');
    expect(badgeFor(m, 'verify').label).toBe('1');
    expect(badgeFor(m, 'structure').label).toBe('3');
    expect(badgeFor(m, 'tests').label).toBe('3');
    expect(badgeFor(m, 'data').label).toBe('3');
    expect(badgeFor({ id: 'm2', functions: [] }, 'verify').tone).toBe('ok');
  });

  test('rules lens returns muted tone and function count label', () => {
    const m = { id: 'm', functions: [{ status: 'ok' }] };
    expect(badgeFor(m, 'rules').tone).toBe('muted');
    expect(badgeFor(m, 'rules').label).toBe('1');
  });

  test('verify lens returns warn tone when at least one function has warn status', () => {
    const m = { id: 'm', functions: [{ status: 'warn' }, { status: 'ok' }] };
    expect(badgeFor(m, 'verify').tone).toBe('warn');
    expect(badgeFor(m, 'verify').label).toBe('1');
  });
});
