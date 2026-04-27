import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import {
  scheduleReview,
  cancelReviewerPending,
  reset,
  REVIEWER_DEBOUNCE_MS,
} from './reviewer-scheduler';

beforeEach(() => {
  vi.useFakeTimers();
  reset();
});

afterEach(() => {
  reset();
  vi.useRealTimers();
});

describe('reviewer-scheduler.ts', () => {
  it('scheduleReview_fires_after_debounce_window', () => {
    const cb = vi.fn();
    scheduleReview('wallet_service.src', cb);

    expect(cb).not.toHaveBeenCalled();

    vi.advanceTimersByTime(REVIEWER_DEBOUNCE_MS);

    expect(cb).toHaveBeenCalledOnce();
    expect(cb).toHaveBeenCalledWith('wallet_service.src');
  });

  it('multiple_scheduleReview_calls_coalesce_to_last_nodeId', () => {
    const cb = vi.fn();
    scheduleReview('node-1', cb);
    vi.advanceTimersByTime(REVIEWER_DEBOUNCE_MS / 2);
    scheduleReview('node-2', cb);
    vi.advanceTimersByTime(REVIEWER_DEBOUNCE_MS / 2);

    // Not yet fired — second call reset the timer
    expect(cb).not.toHaveBeenCalled();

    vi.advanceTimersByTime(REVIEWER_DEBOUNCE_MS);

    expect(cb).toHaveBeenCalledOnce();
    expect(cb).toHaveBeenCalledWith('node-2');
  });

  it('cancelReviewerPending_prevents_callback', () => {
    const cb = vi.fn();
    scheduleReview('node-1', cb);

    cancelReviewerPending();

    vi.advanceTimersByTime(REVIEWER_DEBOUNCE_MS * 2);

    expect(cb).not.toHaveBeenCalled();
  });

  it('reset_is_alias_for_cancelReviewerPending', () => {
    const cb = vi.fn();
    scheduleReview('node-1', cb);

    reset();

    vi.advanceTimersByTime(REVIEWER_DEBOUNCE_MS * 2);

    expect(cb).not.toHaveBeenCalled();
  });

  it('reviewer_scheduler_no_imports_from_stores_or_bridge', () => {
    const __filename = fileURLToPath(import.meta.url);
    const __dirname = dirname(__filename);
    const source = readFileSync(
      join(__dirname, 'reviewer-scheduler.ts'),
      'utf-8',
    );
    expect(source).not.toMatch(/\$lib\/stores/);
    expect(source).not.toMatch(/\$lib\/bridge/);
  });
});
