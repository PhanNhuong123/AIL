import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  isVerifyRunning,
  currentVerifyRunId,
  verifyTick,
  resetVerifyState,
} from './verify-state';

beforeEach(() => {
  resetVerifyState();
});

describe('verify-state.ts', () => {
  it('VS1 — initial values', () => {
    expect(get(isVerifyRunning)).toBe(false);
    expect(get(currentVerifyRunId)).toBeNull();
    expect(get(verifyTick)).toBe(0);
  });

  it('VS2 — resetVerifyState clears all', () => {
    isVerifyRunning.set(true);
    currentVerifyRunId.set('vr-42');
    verifyTick.set(5);

    resetVerifyState();

    expect(get(isVerifyRunning)).toBe(false);
    expect(get(currentVerifyRunId)).toBeNull();
    expect(get(verifyTick)).toBe(0);
  });

  it('VS3 — currentVerifyRunId can hold string', () => {
    currentVerifyRunId.set('vr-run-123');
    expect(get(currentVerifyRunId)).toBe('vr-run-123');
  });

  it('VS4 — verifyTick is monotonic', () => {
    expect(get(verifyTick)).toBe(0);
    verifyTick.set(1);
    expect(get(verifyTick)).toBe(1);
    verifyTick.set(2);
    expect(get(verifyTick)).toBe(2);
    verifyTick.set(3);
    expect(get(verifyTick)).toBe(3);
  });

  it('VS5 — isVerifyRunning toggles', () => {
    expect(get(isVerifyRunning)).toBe(false);
    isVerifyRunning.set(true);
    expect(get(isVerifyRunning)).toBe(true);
    isVerifyRunning.set(false);
    expect(get(isVerifyRunning)).toBe(false);
  });
});
