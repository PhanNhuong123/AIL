import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  isReviewerRunning,
  currentReviewerRunId,
  coverageVersion,
  updateLastReviewed,
  getLastReviewedStatus,
  resetReviewerState,
} from './reviewer-state';

beforeEach(() => {
  resetReviewerState();
});

describe('reviewer-state.ts', () => {
  it('updateLastReviewed_increments_coverageVersion', () => {
    const before = get(coverageVersion);
    updateLastReviewed('wallet_service.src.transfer', 'Full');
    expect(get(coverageVersion)).toBe(before + 1);
  });

  it('getLastReviewedStatus_returns_null_before_first_update', () => {
    expect(getLastReviewedStatus('wallet_service.src.transfer')).toBeNull();
  });

  it('resetReviewerState_clears_map_stores_and_bumps_version', () => {
    updateLastReviewed('mod-a', 'Partial');
    isReviewerRunning.set(true);
    currentReviewerRunId.set('rev-42');

    const versionBefore = get(coverageVersion);
    resetReviewerState();

    expect(get(isReviewerRunning)).toBe(false);
    expect(get(currentReviewerRunId)).toBeNull();
    expect(getLastReviewedStatus('mod-a')).toBeNull();
    // resetReviewerState also bumps version (via update)
    expect(get(coverageVersion)).toBeGreaterThan(versionBefore);
  });

  it('lastReviewedStatus_is_session_lifetime', () => {
    // Writes survive multiple read cycles within session
    updateLastReviewed('mod-persistent', 'Partial');
    expect(getLastReviewedStatus('mod-persistent')).toBe('Partial');
    // Second read cycle — value still there
    expect(getLastReviewedStatus('mod-persistent')).toBe('Partial');
    // Update overrides
    updateLastReviewed('mod-persistent', 'Weak');
    expect(getLastReviewedStatus('mod-persistent')).toBe('Weak');
  });
});
