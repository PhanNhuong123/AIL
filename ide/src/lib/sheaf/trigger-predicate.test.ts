import { describe, it, expect } from 'vitest';
import { hasSheafTriggerFailure } from './trigger-predicate';
import type { VerifyCompletePayload } from '$lib/types';

describe('hasSheafTriggerFailure (Phase 17.4)', () => {
  const base: VerifyCompletePayload = {
    runId: 'v-1',
    ok: true,
    scope: 'project',
    nodeIds: [],
    failures: [],
    cancelled: false,
  };

  it('returns false when payload is cancelled', () => {
    expect(hasSheafTriggerFailure({ ...base, cancelled: true })).toBe(false);
  });

  it('returns false when ok=true and failures empty', () => {
    expect(hasSheafTriggerFailure(base)).toBe(false);
  });

  it('returns true when !ok', () => {
    expect(hasSheafTriggerFailure({ ...base, ok: false })).toBe(true);
  });

  it('returns true when failures non-empty', () => {
    expect(
      hasSheafTriggerFailure({
        ...base,
        failures: [{ nodeId: 'n', message: 'fail', severity: 'fail', source: 'verify' }],
      })
    ).toBe(true);
  });

  it('returns false when cancelled=true even if ok=false and failures non-empty', () => {
    expect(
      hasSheafTriggerFailure({
        ...base,
        cancelled: true,
        ok: false,
        failures: [{ nodeId: 'n', message: 'fail', severity: 'fail', source: 'verify' }],
      })
    ).toBe(false);
  });
});
