import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  scheduleSheaf,
  cancelSheafPending,
  resetSheaf,
  SHEAF_DEBOUNCE_MS,
} from './sheaf-scheduler';

beforeEach(() => {
  vi.useFakeTimers();
  resetSheaf();
});

afterEach(() => {
  resetSheaf();
  vi.useRealTimers();
});

describe('sheaf-scheduler.ts', () => {
  it('SC1: scheduleSheaf fires runFn after SHEAF_DEBOUNCE_MS with the given nodeId', () => {
    const runFn = vi.fn();
    scheduleSheaf('node-1', runFn);

    expect(runFn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(SHEAF_DEBOUNCE_MS);

    expect(runFn).toHaveBeenCalledOnce();
    expect(runFn).toHaveBeenCalledWith('node-1');
  });

  it('SC2: multiple scheduleSheaf calls within window coalesce to single run with most recent nodeId', () => {
    const runFn = vi.fn();
    scheduleSheaf('node-1', runFn);
    vi.advanceTimersByTime(SHEAF_DEBOUNCE_MS / 2);
    scheduleSheaf('node-2', runFn);
    vi.advanceTimersByTime(SHEAF_DEBOUNCE_MS / 2);

    // Not yet fired — second call reset the timer
    expect(runFn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(SHEAF_DEBOUNCE_MS);

    expect(runFn).toHaveBeenCalledOnce();
    expect(runFn).toHaveBeenCalledWith('node-2');
  });

  it('SC3: cancelSheafPending clears timer; runFn never fires', () => {
    const runFn = vi.fn();
    scheduleSheaf('node-1', runFn);

    cancelSheafPending();

    vi.advanceTimersByTime(SHEAF_DEBOUNCE_MS * 2);

    expect(runFn).not.toHaveBeenCalled();
  });

  it('SC4: resetSheaf is alias of cancelSheafPending; runFn never fires after reset', () => {
    const runFn = vi.fn();
    scheduleSheaf('node-1', runFn);

    resetSheaf();

    vi.advanceTimersByTime(SHEAF_DEBOUNCE_MS * 2);

    expect(runFn).not.toHaveBeenCalled();
  });
});
