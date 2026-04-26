import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  scheduleVerify,
  cancelPending,
  reset,
  VERIFY_DEBOUNCE_MS,
} from './verifier-scheduler';

beforeEach(() => {
  vi.useFakeTimers();
  reset();
});

afterEach(() => {
  reset();
  vi.useRealTimers();
});

describe('verifier-scheduler.ts', () => {
  it('S1 — scheduleVerify with non-empty ids fires callback after VERIFY_DEBOUNCE_MS with same ids', () => {
    const cb = vi.fn();
    scheduleVerify(['mod-1', 'fn-1'], cb);

    expect(cb).not.toHaveBeenCalled();

    vi.advanceTimersByTime(VERIFY_DEBOUNCE_MS);

    expect(cb).toHaveBeenCalledOnce();
    expect(cb).toHaveBeenCalledWith(['mod-1', 'fn-1']);
  });

  it('S2 — multiple scheduleVerify calls within window coalesce ids (union) into single callback', () => {
    const cb = vi.fn();
    scheduleVerify(['mod-1'], cb);
    vi.advanceTimersByTime(VERIFY_DEBOUNCE_MS / 2);
    scheduleVerify(['fn-1', 'fn-2'], cb);
    vi.advanceTimersByTime(VERIFY_DEBOUNCE_MS / 2);

    // Not yet fired — second call reset the timer
    expect(cb).not.toHaveBeenCalled();

    vi.advanceTimersByTime(VERIFY_DEBOUNCE_MS);

    expect(cb).toHaveBeenCalledOnce();
    const ids = cb.mock.calls[0][0] as string[];
    expect(ids).toContain('mod-1');
    expect(ids).toContain('fn-1');
    expect(ids).toContain('fn-2');
    expect(ids.length).toBe(3);
  });

  it('S3 — scheduleVerify with empty ids does not arm timer or fire callback', () => {
    const cb = vi.fn();
    scheduleVerify([], cb);

    vi.advanceTimersByTime(VERIFY_DEBOUNCE_MS * 2);

    expect(cb).not.toHaveBeenCalled();
  });

  it('S4 — cancelPending clears timer and accumulator; callback never fires', () => {
    const cb = vi.fn();
    scheduleVerify(['mod-1', 'fn-1'], cb);

    cancelPending();

    vi.advanceTimersByTime(VERIFY_DEBOUNCE_MS * 2);

    expect(cb).not.toHaveBeenCalled();
  });

  it('S5 — reset is alias of cancelPending', () => {
    const cb = vi.fn();
    scheduleVerify(['step-1'], cb);

    reset();

    vi.advanceTimersByTime(VERIFY_DEBOUNCE_MS * 2);

    expect(cb).not.toHaveBeenCalled();
  });
});
