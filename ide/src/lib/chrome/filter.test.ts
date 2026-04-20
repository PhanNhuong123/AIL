import { describe, it, expect } from 'vitest';
import { filterTree, isVisible, ALL } from './filter';
import { walletFixture } from './fixtures';

describe('filterTree', () => {
  it('returns ALL sentinel for empty term', () => {
    const g = walletFixture();
    const result = filterTree(g, '');
    expect(result).toBe(ALL);
  });

  it('returns ALL sentinel for whitespace-only term', () => {
    const g = walletFixture();
    const result = filterTree(g, '   ');
    expect(result).toBe(ALL);
  });

  it('preserves ancestors when a step matches', () => {
    const g = walletFixture();
    // 'credit_to_payee' step matches
    const result = filterTree(g, 'credit');
    expect(result).not.toBe(ALL);
    const set = result as Set<string>;
    // the step itself
    expect(set.has('step:s_credit')).toBe(true);
    // parent function
    expect(set.has('function:fn_transfer')).toBe(true);
    // parent module
    expect(set.has('module:m_wallet')).toBe(true);
    // project
    expect(set.has('project:root')).toBe(true);
  });

  it('drops siblings that do not match', () => {
    const g = walletFixture();
    // 'credit_to_payee' matches — auth module should NOT be visible
    const result = filterTree(g, 'credit');
    const set = result as Set<string>;
    expect(set.has('module:m_auth')).toBe(false);
  });

  it('is case-insensitive', () => {
    const g = walletFixture();
    const lower = filterTree(g, 'wallet');
    const upper = filterTree(g, 'WALLET');
    expect(lower).not.toBe(ALL);
    expect(upper).not.toBe(ALL);
    const lSet = lower as Set<string>;
    const uSet = upper as Set<string>;
    expect(lSet.has('module:m_wallet')).toBe(true);
    expect(uSet.has('module:m_wallet')).toBe(true);
  });

  it('matches types and errors at project level', () => {
    const g = walletFixture();
    const result = filterTree(g, 'InsufficientFunds');
    const set = result as Set<string>;
    expect(set.has('error:e_insufficient')).toBe(true);
    expect(set.has('project:root')).toBe(true);
  });

  it('returns empty Set when nothing matches', () => {
    const g = walletFixture();
    const result = filterTree(g, 'xyzzy_no_match');
    expect(result).not.toBe(ALL);
    expect((result as Set<string>).size).toBe(0);
  });
});

describe('isVisible', () => {
  it('returns true for ALL sentinel', () => {
    expect(isVisible(ALL, 'anything')).toBe(true);
  });

  it('returns true when id is in the set', () => {
    const s = new Set(['a', 'b']);
    expect(isVisible(s, 'a')).toBe(true);
  });

  it('returns false when id is not in the set', () => {
    const s = new Set(['a']);
    expect(isVisible(s, 'z')).toBe(false);
  });
});
