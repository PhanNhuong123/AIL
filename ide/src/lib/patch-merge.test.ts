import { describe, it, expect } from 'vitest';
import type {
  ModuleJson,
  FunctionJson,
  StepJson,
  GraphPatchJson,
} from './types';
import { emptyPatch, isEmptyPatch, mergePatches } from './patch-merge';

// ---------------------------------------------------------------------------
// Inline builders (mirrors graph-patch.test.ts style)
// ---------------------------------------------------------------------------

function step(id: string, intent = 'do'): StepJson {
  return { id, name: id, status: 'ok', intent };
}

function fn(id: string, steps?: StepJson[]): FunctionJson {
  return { id, name: id, status: 'ok', steps };
}

function mod(id: string, fns: FunctionJson[] = []): ModuleJson {
  return {
    id,
    name: id,
    description: '',
    cluster: 'default',
    clusterName: 'default',
    clusterColor: '#2997ff',
    status: 'ok',
    nodeCount: fns.length + 1,
    functions: fns,
  };
}

// ---------------------------------------------------------------------------
// Tests M1 – M16
// ---------------------------------------------------------------------------

describe('mergePatches', () => {
  it('M1 — right identity: mergePatches(a, emptyPatch()) deep-equals a', () => {
    const a: GraphPatchJson = {
      ...emptyPatch(),
      modulesAdded: [mod('m1')],
      timestamp: 10,
    };
    const result = mergePatches(a, emptyPatch());
    // timestamp comes from b (emptyPatch = 0)
    expect(result.modulesAdded.map((m) => m.id)).toEqual(['m1']);
    expect(result.modulesRemoved).toEqual([]);
    expect(result.modulesModified).toEqual([]);
    expect(result.timestamp).toBe(0);
  });

  it('M2 — left identity: mergePatches(emptyPatch(), b) deep-equals b', () => {
    const b: GraphPatchJson = {
      ...emptyPatch(),
      modulesAdded: [mod('m2')],
      timestamp: 20,
    };
    const result = mergePatches(emptyPatch(), b);
    expect(result.modulesAdded.map((m) => m.id)).toEqual(['m2']);
    expect(result.timestamp).toBe(20);
  });

  it('M3 — add then remove same module → both arrays drop the entry (full cancel)', () => {
    // The entity briefly existed and was gone before any consumer saw it; the
    // burst nets to a no-op so neither modulesAdded nor modulesRemoved should
    // carry the id. This semantic lets isEmptyPatch correctly skip the flush.
    const a: GraphPatchJson = { ...emptyPatch(), modulesAdded: [mod('m-x')] };
    const b: GraphPatchJson = { ...emptyPatch(), modulesRemoved: ['m-x'] };
    const result = mergePatches(a, b);
    expect(result.modulesAdded.map((m) => m.id)).not.toContain('m-x');
    expect(result.modulesRemoved).not.toContain('m-x');
  });

  it('M4 — remove then add same module → only modulesAdded (b payload), modulesRemoved empty', () => {
    const bMod = { ...mod('m-r'), name: 're-added' };
    const a: GraphPatchJson = { ...emptyPatch(), modulesRemoved: ['m-r'] };
    const b: GraphPatchJson = { ...emptyPatch(), modulesAdded: [bMod] };
    const result = mergePatches(a, b);
    expect(result.modulesRemoved).not.toContain('m-r');
    const found = result.modulesAdded.find((m) => m.id === 'm-r');
    expect(found).toBeDefined();
    expect(found!.name).toBe('re-added');
  });

  it('M5 — modify then modify same module → result has b entry (LWW)', () => {
    const a: GraphPatchJson = {
      ...emptyPatch(),
      modulesModified: [{ ...mod('m-lw'), description: 'v1' }],
    };
    const b: GraphPatchJson = {
      ...emptyPatch(),
      modulesModified: [{ ...mod('m-lw'), description: 'v2' }],
    };
    const result = mergePatches(a, b);
    const found = result.modulesModified.find((m) => m.id === 'm-lw');
    expect(found?.description).toBe('v2');
    expect(result.modulesModified.filter((m) => m.id === 'm-lw')).toHaveLength(1);
  });

  it('M6 — modify then remove same module → only modulesRemoved', () => {
    const a: GraphPatchJson = {
      ...emptyPatch(),
      modulesModified: [mod('m-del')],
    };
    const b: GraphPatchJson = { ...emptyPatch(), modulesRemoved: ['m-del'] };
    const result = mergePatches(a, b);
    expect(result.modulesModified.find((m) => m.id === 'm-del')).toBeUndefined();
    expect(result.modulesRemoved).toContain('m-del');
  });

  it('M7 — two disjoint modulesAdded → both present', () => {
    const a: GraphPatchJson = { ...emptyPatch(), modulesAdded: [mod('m-a')] };
    const b: GraphPatchJson = { ...emptyPatch(), modulesAdded: [mod('m-b')] };
    const result = mergePatches(a, b);
    const ids = result.modulesAdded.map((m) => m.id);
    expect(ids).toContain('m-a');
    expect(ids).toContain('m-b');
  });

  it('M8 — add then remove same function → full cancel (both arrays drop entry)', () => {
    const a: GraphPatchJson = {
      ...emptyPatch(),
      functionsAdded: [{ moduleId: 'm1', function: fn('m1.f1') }],
    };
    const b: GraphPatchJson = {
      ...emptyPatch(),
      functionsRemoved: [{ moduleId: 'm1', functionId: 'm1.f1' }],
    };
    const result = mergePatches(a, b);
    expect(result.functionsAdded.find((fa) => fa.function.id === 'm1.f1')).toBeUndefined();
    expect(result.functionsRemoved.some((r) => r.functionId === 'm1.f1')).toBe(false);
  });

  it('M9 — add then modify same function → functionsAdded contains updated payload', () => {
    const origFn = fn('m1.f2');
    const modifiedFn = { ...fn('m1.f2'), status: 'fail' as const };
    const a: GraphPatchJson = {
      ...emptyPatch(),
      functionsAdded: [{ moduleId: 'm1', function: origFn }],
    };
    const b: GraphPatchJson = {
      ...emptyPatch(),
      functionsModified: [{ moduleId: 'm1', function: modifiedFn }],
    };
    const result = mergePatches(a, b);
    const found = result.functionsAdded.find((fa) => fa.function.id === 'm1.f2');
    expect(found).toBeDefined();
    expect(found!.function.status).toBe('fail');
    expect(found!.moduleId).toBe('m1');
    // Must NOT appear in functionsModified
    expect(result.functionsModified.find((fm) => fm.function.id === 'm1.f2')).toBeUndefined();
  });

  it('M10 — add then remove same step → full cancel (both arrays drop entry)', () => {
    const a: GraphPatchJson = {
      ...emptyPatch(),
      stepsAdded: [{ functionId: 'f1', step: step('f1.s1') }],
    };
    const b: GraphPatchJson = {
      ...emptyPatch(),
      stepsRemoved: [{ functionId: 'f1', stepId: 'f1.s1' }],
    };
    const result = mergePatches(a, b);
    expect(result.stepsAdded.find((sa) => sa.step.id === 'f1.s1')).toBeUndefined();
    expect(result.stepsRemoved.some((r) => r.stepId === 'f1.s1')).toBe(false);
  });

  it('M11 — modify then modify same step → b wins', () => {
    const a: GraphPatchJson = {
      ...emptyPatch(),
      stepsModified: [{ functionId: 'f1', step: { ...step('f1.s2'), intent: 'v1' } }],
    };
    const b: GraphPatchJson = {
      ...emptyPatch(),
      stepsModified: [{ functionId: 'f1', step: { ...step('f1.s2'), intent: 'v2' } }],
    };
    const result = mergePatches(a, b);
    const found = result.stepsModified.find((sm) => sm.step.id === 'f1.s2');
    expect(found?.step.intent).toBe('v2');
    expect(result.stepsModified.filter((sm) => sm.step.id === 'f1.s2')).toHaveLength(1);
  });

  it('M12 — timestamp = b.timestamp', () => {
    const a: GraphPatchJson = { ...emptyPatch(), timestamp: 100 };
    const b: GraphPatchJson = { ...emptyPatch(), timestamp: 200 };
    expect(mergePatches(a, b).timestamp).toBe(200);
  });

  it('M13 — three-patch chain associativity: key fields match expected union', () => {
    const pa: GraphPatchJson = { ...emptyPatch(), modulesAdded: [mod('x')], timestamp: 1 };
    const pb: GraphPatchJson = { ...emptyPatch(), modulesAdded: [mod('y')], timestamp: 2 };
    const pc: GraphPatchJson = { ...emptyPatch(), modulesAdded: [mod('z')], timestamp: 3 };
    const result = mergePatches(mergePatches(pa, pb), pc);
    const ids = result.modulesAdded.map((m) => m.id);
    expect(ids).toContain('x');
    expect(ids).toContain('y');
    expect(ids).toContain('z');
    expect(result.timestamp).toBe(3);
  });

  it('M14 — isEmptyPatch returns true for emptyPatch()', () => {
    expect(isEmptyPatch(emptyPatch())).toBe(true);
  });

  it('M15 — isEmptyPatch returns false when any array is non-empty', () => {
    expect(isEmptyPatch({ ...emptyPatch(), modulesAdded: [mod('m1')] })).toBe(false);
    expect(isEmptyPatch({ ...emptyPatch(), modulesRemoved: ['m1'] })).toBe(false);
    expect(isEmptyPatch({ ...emptyPatch(), functionsAdded: [{ moduleId: 'm1', function: fn('f1') }] })).toBe(false);
    expect(isEmptyPatch({ ...emptyPatch(), stepsAdded: [{ functionId: 'f1', step: step('s1') }] })).toBe(false);
  });

  it('M16 — mergePatches does not mutate inputs', () => {
    const a: GraphPatchJson = {
      ...emptyPatch(),
      modulesAdded: [mod('m-input-a')],
      modulesRemoved: ['m-gone'],
      timestamp: 5,
    };
    const b: GraphPatchJson = {
      ...emptyPatch(),
      modulesAdded: [mod('m-input-b')],
      functionsAdded: [{ moduleId: 'm-input-b', function: fn('f-b') }],
      timestamp: 6,
    };
    const aSnapshot = JSON.stringify(a);
    const bSnapshot = JSON.stringify(b);

    mergePatches(a, b);

    expect(JSON.stringify(a)).toBe(aSnapshot);
    expect(JSON.stringify(b)).toBe(bSnapshot);
  });
});
