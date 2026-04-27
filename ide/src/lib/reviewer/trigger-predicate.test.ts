import { describe, it, expect } from 'vitest';
import { hasReviewerTrigger, hasMaterialStatusChange } from './trigger-predicate';
import type { VerifyCompletePayload } from '$lib/types';

describe('trigger-predicate.ts (Phase 16.4)', () => {
  const base: VerifyCompletePayload = {
    runId: 'v-1',
    ok: true,
    scope: 'project',
    nodeIds: [],
    failures: [],
    cancelled: false,
  };

  // --- hasReviewerTrigger ---

  it('hasReviewerTrigger_true_when_ok_and_not_cancelled', () => {
    expect(hasReviewerTrigger({ ...base, ok: true, cancelled: false })).toBe(true);
  });

  it('hasReviewerTrigger_false_when_cancelled', () => {
    expect(hasReviewerTrigger({ ...base, cancelled: true })).toBe(false);
  });

  it('hasReviewerTrigger_false_when_not_ok', () => {
    expect(hasReviewerTrigger({ ...base, ok: false })).toBe(false);
  });

  // --- hasMaterialStatusChange ---

  it('hasMaterialStatusChange_null_weak_returns_true', () => {
    expect(hasMaterialStatusChange(null, 'Weak')).toBe(true);
  });

  it('hasMaterialStatusChange_null_unavailable_returns_true', () => {
    expect(hasMaterialStatusChange(null, 'Unavailable')).toBe(true);
  });

  it('hasMaterialStatusChange_null_full_returns_false', () => {
    expect(hasMaterialStatusChange(null, 'Full')).toBe(false);
  });

  it('hasMaterialStatusChange_null_partial_returns_false', () => {
    expect(hasMaterialStatusChange(null, 'Partial')).toBe(false);
  });

  it('hasMaterialStatusChange_full_to_partial_returns_true', () => {
    // Full(3) -> Partial(2): degradation
    expect(hasMaterialStatusChange('Full', 'Partial')).toBe(true);
  });

  it('hasMaterialStatusChange_partial_to_weak_returns_true', () => {
    // Partial(2) -> Weak(1): degradation
    expect(hasMaterialStatusChange('Partial', 'Weak')).toBe(true);
  });

  it('hasMaterialStatusChange_partial_to_full_returns_false', () => {
    // Partial(2) -> Full(3): improvement — suppress
    expect(hasMaterialStatusChange('Partial', 'Full')).toBe(false);
  });

  it('hasMaterialStatusChange_same_status_returns_false', () => {
    expect(hasMaterialStatusChange('Full', 'Full')).toBe(false);
    expect(hasMaterialStatusChange('Partial', 'Partial')).toBe(false);
    expect(hasMaterialStatusChange('Weak', 'Weak')).toBe(false);
  });
});
