import { describe, it, expect } from 'vitest';

import type {
  FunctionJson,
  GraphJson,
  GraphPatchJson,
  ModuleJson,
  StepJson,
} from './types';
import {
  applyGraphPatch,
  reconcileSelectionAfterPatch,
  type Selection,
} from './graph-patch';

// ---------------------------------------------------------------------------
// Builders
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

function seedGraph(modules: ModuleJson[] = []): GraphJson {
  const fnCount = modules.reduce((n, m) => n + m.functions.length, 0);
  return {
    project: {
      id: 'p1',
      name: 'Proj',
      description: '',
      nodeCount: modules.length + fnCount,
      moduleCount: modules.length,
      fnCount,
      status: 'ok',
    },
    clusters: [],
    modules,
    externals: [],
    relations: [],
    types: [],
    errors: [],
    issues: [],
    detail: {},
  };
}

function emptyPatch(): GraphPatchJson {
  return {
    modulesAdded: [],
    modulesModified: [],
    modulesRemoved: [],
    functionsAdded: [],
    functionsModified: [],
    functionsRemoved: [],
    stepsAdded: [],
    stepsModified: [],
    stepsRemoved: [],
    timestamp: 0,
  };
}

// ---------------------------------------------------------------------------
// applyGraphPatch — 9 array cases
// ---------------------------------------------------------------------------

describe('applyGraphPatch', () => {
  it('modules added: appends to graph.modules', () => {
    const prev = seedGraph([mod('m1')]);
    const patch = { ...emptyPatch(), modulesAdded: [mod('m2')] };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules.map((m) => m.id)).toEqual(['m1', 'm2']);
    expect(next.project.moduleCount).toBe(2);
  });

  it('modules modified: replaces metadata in place, preserves functions array', () => {
    const prev = seedGraph([mod('m1', [fn('m1.f1')])]);
    // The patch payload intentionally carries an EMPTY functions array;
    // applyGraphPatch must ignore it and keep the prev functions intact.
    const patch = {
      ...emptyPatch(),
      modulesModified: [{ ...mod('m1'), description: 'updated' }],
    };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules[0].description).toBe('updated');
    expect(next.modules[0].functions.map((f) => f.id)).toEqual(['m1.f1']);
  });

  it('modules removed: drops by id', () => {
    const prev = seedGraph([mod('m1'), mod('m2')]);
    const patch = { ...emptyPatch(), modulesRemoved: ['m2'] };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules.map((m) => m.id)).toEqual(['m1']);
    expect(next.project.moduleCount).toBe(1);
  });

  it('functions added: appends to parent module', () => {
    const prev = seedGraph([mod('m1')]);
    const patch = {
      ...emptyPatch(),
      functionsAdded: [{ moduleId: 'm1', function: fn('m1.f1') }],
    };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules[0].functions.map((f) => f.id)).toEqual(['m1.f1']);
    expect(next.project.fnCount).toBe(1);
  });

  it('functions modified: replaces in place, preserves steps', () => {
    const prev = seedGraph([mod('m1', [fn('m1.f1', [step('m1.f1.s1')])])]);
    // Patch payload carries NO steps field at all; prev.steps must survive.
    const patch = {
      ...emptyPatch(),
      functionsModified: [
        { moduleId: 'm1', function: { ...fn('m1.f1'), status: 'fail' as const } },
      ],
    };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules[0].functions[0].status).toBe('fail');
    expect(next.modules[0].functions[0].steps?.map((s) => s.id)).toEqual(['m1.f1.s1']);
  });

  it('functions removed: drops from parent module', () => {
    const prev = seedGraph([mod('m1', [fn('m1.f1'), fn('m1.f2')])]);
    const patch = {
      ...emptyPatch(),
      functionsRemoved: [{ moduleId: 'm1', functionId: 'm1.f2' }],
    };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules[0].functions.map((f) => f.id)).toEqual(['m1.f1']);
  });

  it('steps added: appends to parent function', () => {
    const prev = seedGraph([mod('m1', [fn('m1.f1', [])])]);
    const patch = {
      ...emptyPatch(),
      stepsAdded: [{ functionId: 'm1.f1', step: step('m1.f1.s1') }],
    };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules[0].functions[0].steps?.map((s) => s.id)).toEqual(['m1.f1.s1']);
  });

  it('steps modified: replaces in place', () => {
    const prev = seedGraph([mod('m1', [fn('m1.f1', [step('m1.f1.s1', 'orig')])])]);
    const patch = {
      ...emptyPatch(),
      stepsModified: [
        { functionId: 'm1.f1', step: { ...step('m1.f1.s1'), intent: 'changed' } },
      ],
    };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules[0].functions[0].steps?.[0].intent).toBe('changed');
  });

  it('steps removed: drops from parent function', () => {
    const prev = seedGraph([
      mod('m1', [fn('m1.f1', [step('m1.f1.s1'), step('m1.f1.s2')])]),
    ]);
    const patch = {
      ...emptyPatch(),
      stepsRemoved: [{ functionId: 'm1.f1', stepId: 'm1.f1.s2' }],
    };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules[0].functions[0].steps?.map((s) => s.id)).toEqual(['m1.f1.s1']);
  });

  // -------------------------------------------------------------------------
  // Ordering and immutability
  // -------------------------------------------------------------------------

  it('remove-then-add with the same id yields the new entry', () => {
    // Mirrors what diff_graph would emit: module_removed + module_added for
    // the same id, plus functions_added for the new function.
    const prev = seedGraph([mod('m1', [fn('m1.f1')])]);
    const patch = {
      ...emptyPatch(),
      modulesRemoved: ['m1'],
      modulesAdded: [mod('m1')],
      functionsAdded: [{ moduleId: 'm1', function: fn('m1.f9') }],
    };

    const next = applyGraphPatch(prev, patch);

    // Resulting m1 should be the added one (f9), not the removed one (f1).
    expect(next.modules[0].id).toBe('m1');
    expect(next.modules[0].functions.map((f) => f.id)).toEqual(['m1.f9']);
  });

  it('modulesModified ignores the .functions field in the payload (trusts functions_* arrays only)', () => {
    const prev = seedGraph([mod('m1', [fn('m1.f1'), fn('m1.f2')])]);
    // Payload falsely claims no functions; prev has two. They must survive.
    const patch = {
      ...emptyPatch(),
      modulesModified: [mod('m1', [])],
    };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules[0].functions.map((f) => f.id)).toEqual(['m1.f1', 'm1.f2']);
  });

  it('does not mutate inputs', () => {
    const prev = seedGraph([mod('m1', [fn('m1.f1', [step('m1.f1.s1')])])]);
    const prevSerialized = JSON.stringify(prev);
    const patch: GraphPatchJson = {
      ...emptyPatch(),
      modulesAdded: [mod('m2')],
      stepsRemoved: [{ functionId: 'm1.f1', stepId: 'm1.f1.s1' }],
    };
    const patchSerialized = JSON.stringify(patch);

    applyGraphPatch(prev, patch);

    expect(JSON.stringify(prev)).toBe(prevSerialized);
    expect(JSON.stringify(patch)).toBe(patchSerialized);
  });

  it('modulesAdded payload functions are ignored — trust functions_*/steps_* arrays (B1 regression)', () => {
    // diff_graph emits the full module (with functions) in modulesAdded AND
    // the same functions in functionsAdded AND the same steps in stepsAdded.
    // Applying a patch that looks exactly like that must NOT double-apply.
    const prev = seedGraph([]);
    const newModule = mod('m1', [fn('m1.f1', [step('m1.f1.s1')])]);
    const patch: GraphPatchJson = {
      ...emptyPatch(),
      modulesAdded: [newModule],
      functionsAdded: [
        { moduleId: 'm1', function: fn('m1.f1', [step('m1.f1.s1')]) },
      ],
      stepsAdded: [{ functionId: 'm1.f1', step: step('m1.f1.s1') }],
    };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules).toHaveLength(1);
    expect(next.modules[0].functions).toHaveLength(1);
    expect(next.modules[0].functions[0].steps).toHaveLength(1);
    expect(next.modules[0].functions[0].steps?.[0].id).toBe('m1.f1.s1');
  });

  it('functionsAdded payload steps are ignored — trust stepsAdded array', () => {
    const prev = seedGraph([mod('m1')]);
    const patch: GraphPatchJson = {
      ...emptyPatch(),
      functionsAdded: [
        { moduleId: 'm1', function: fn('m1.f1', [step('m1.f1.s1')]) },
      ],
      stepsAdded: [{ functionId: 'm1.f1', step: step('m1.f1.s1') }],
    };

    const next = applyGraphPatch(prev, patch);

    expect(next.modules[0].functions[0].steps).toHaveLength(1);
  });

  it('updates project counts based on final array sizes', () => {
    // A realistic multi-array patch as diff_graph would emit when a new
    // module with one function and two steps is added to prev.
    const prev = seedGraph([mod('m1')]);
    const patch = {
      ...emptyPatch(),
      modulesAdded: [mod('m2')],
      functionsAdded: [{ moduleId: 'm2', function: fn('m2.f1') }],
      stepsAdded: [
        { functionId: 'm2.f1', step: step('m2.f1.s1') },
        { functionId: 'm2.f1', step: step('m2.f1.s2') },
      ],
    };

    const next = applyGraphPatch(prev, patch);

    expect(next.project.moduleCount).toBe(2);
    expect(next.project.fnCount).toBe(1);
    // nodeCount = modules + fns + steps = 2 + 1 + 2
    expect(next.project.nodeCount).toBe(5);
  });
});

// ---------------------------------------------------------------------------
// reconcileSelectionAfterPatch — 4 cases
// ---------------------------------------------------------------------------

describe('reconcileSelectionAfterPatch', () => {
  it('step removed → selection collapses to parent function', () => {
    const graph = seedGraph([mod('m1', [fn('m1.f1', [])])]); // step gone
    const sel: Selection = { kind: 'step', id: 'm1.f1.s1' };
    const patch = {
      ...emptyPatch(),
      stepsRemoved: [{ functionId: 'm1.f1', stepId: 'm1.f1.s1' }],
    };

    const next = reconcileSelectionAfterPatch(sel, patch, graph);

    expect(next).toEqual({ kind: 'function', id: 'm1.f1' });
  });

  it('function removed → selection collapses to parent module', () => {
    const graph = seedGraph([mod('m1', [])]);
    const sel: Selection = { kind: 'function', id: 'm1.f1' };
    const patch = {
      ...emptyPatch(),
      functionsRemoved: [{ moduleId: 'm1', functionId: 'm1.f1' }],
    };

    const next = reconcileSelectionAfterPatch(sel, patch, graph);

    expect(next).toEqual({ kind: 'module', id: 'm1' });
  });

  it('module removed → selection collapses to project', () => {
    const graph = seedGraph([]);
    const sel: Selection = { kind: 'module', id: 'm1' };
    const patch = { ...emptyPatch(), modulesRemoved: ['m1'] };

    const next = reconcileSelectionAfterPatch(sel, patch, graph);

    expect(next).toEqual({ kind: 'project', id: 'p1' });
  });

  it('unrelated patch → selection unchanged', () => {
    const graph = seedGraph([mod('m1', [fn('m1.f1', [step('m1.f1.s1')])])]);
    const sel: Selection = { kind: 'step', id: 'm1.f1.s1' };
    const patch = {
      ...emptyPatch(),
      modulesAdded: [mod('m2')],
    };

    const next = reconcileSelectionAfterPatch(sel, patch, graph);

    expect(next).toBe(sel); // same reference
  });

  it('step removed AND parent function removed in same patch → collapses to module (N2 regression)', () => {
    // Parent module 'm1' survives, but function 'm1.f1' and step 'm1.f1.s1'
    // both disappear. Current selection on the step must fall back to the
    // module via the functionsRemoved.moduleId, NOT skip to project.
    const graph = seedGraph([mod('m1', [])]);
    const sel: Selection = { kind: 'step', id: 'm1.f1.s1' };
    const patch = {
      ...emptyPatch(),
      functionsRemoved: [{ moduleId: 'm1', functionId: 'm1.f1' }],
      stepsRemoved: [{ functionId: 'm1.f1', stepId: 'm1.f1.s1' }],
    };

    const next = reconcileSelectionAfterPatch(sel, patch, graph);

    expect(next).toEqual({ kind: 'module', id: 'm1' });
  });

  it('none selection → returned unchanged', () => {
    const graph = seedGraph([]);
    const sel: Selection = { kind: 'none', id: null };
    const patch = { ...emptyPatch(), modulesRemoved: ['m1'] };

    const next = reconcileSelectionAfterPatch(sel, patch, graph);

    expect(next).toBe(sel);
  });
});
