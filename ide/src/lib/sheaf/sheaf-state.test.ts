import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { isSheafRunning, currentSheafRunId, sheafConflicts, resetSheafState } from './sheaf-state';

describe('sheaf-state.ts', () => {
  beforeEach(() => resetSheafState());

  it('SS1: initial values are empty/false/null', () => {
    expect(get(isSheafRunning)).toBe(false);
    expect(get(currentSheafRunId)).toBe(null);
    expect(get(sheafConflicts)).toEqual([]);
  });

  it('SS2: resetSheafState clears all three stores', () => {
    isSheafRunning.set(true);
    currentSheafRunId.set('sheaf-1-2');
    sheafConflicts.set([{
      overlapIndex: 0, nodeA: 'a', nodeB: 'b',
      conflictingA: ['x'], conflictingB: ['y'],
    }]);
    resetSheafState();
    expect(get(isSheafRunning)).toBe(false);
    expect(get(currentSheafRunId)).toBe(null);
    expect(get(sheafConflicts)).toEqual([]);
  });
});
