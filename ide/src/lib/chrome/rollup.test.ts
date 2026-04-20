import { describe, it, expect } from 'vitest';
import { rollupStatus, countPills, breadcrumbs } from './rollup';
import { walletFixture, walletFixtureWithFail } from './fixtures';

describe('rollupStatus', () => {
  it('returns ok for empty array', () => {
    expect(rollupStatus([])).toBe('ok');
  });

  it('returns ok when all ok', () => {
    expect(rollupStatus(['ok', 'ok'])).toBe('ok');
  });

  it('returns warn when mix of ok and warn', () => {
    expect(rollupStatus(['ok', 'warn', 'ok'])).toBe('warn');
  });

  it('returns fail when any fail present', () => {
    expect(rollupStatus(['ok', 'warn', 'fail'])).toBe('fail');
  });

  it('returns fail even when only one fail and rest ok', () => {
    expect(rollupStatus(['ok', 'ok', 'fail'])).toBe('fail');
  });
});

describe('countPills', () => {
  it('returns zeros for null graph', () => {
    expect(countPills(null)).toEqual({ verified: 0, issues: 0 });
  });

  it('counts leaf steps correctly for walletFixture', () => {
    const g = walletFixture();
    // Steps: debit(ok), credit(warn), validate(ok), fetch(ok) = 4 step leaves
    // login has no steps → function itself is leaf (ok) = 1
    // Types: Money(ok) → not counted; Account(warn) → issues++
    // Errors: InsufficientFunds(ok) → not counted
    // Total: verified = debit+validate+fetch+login = 4, issues = credit+Account = 2
    expect(countPills(g)).toEqual({ verified: 4, issues: 2 });
  });

  it('increases issues when a step fails (walletFixtureWithFail)', () => {
    const g = walletFixtureWithFail();
    // validate_balance is now fail, was ok
    // verified = debit+fetch+login = 3, issues = credit+validate+Account = 3
    expect(countPills(g)).toEqual({ verified: 3, issues: 3 });
  });
});

describe('breadcrumbs', () => {
  it('returns empty array for null graph', () => {
    expect(breadcrumbs(null, ['project:root'])).toEqual([]);
  });

  it('returns empty array for empty path', () => {
    expect(breadcrumbs(walletFixture(), [])).toEqual([]);
  });

  it('resolves project crumb', () => {
    const g = walletFixture();
    const result = breadcrumbs(g, ['project:root']);
    expect(result).toEqual([{ kind: 'project', id: 'root', name: 'wallet_service' }]);
  });

  it('resolves full path: project → module → function', () => {
    const g = walletFixture();
    const result = breadcrumbs(g, [
      'project:root',
      'module:m_wallet',
      'function:fn_transfer',
    ]);
    expect(result).toEqual([
      { kind: 'project',  id: 'root',        name: 'wallet_service' },
      { kind: 'module',   id: 'm_wallet',     name: 'wallet' },
      { kind: 'function', id: 'fn_transfer',  name: 'transfer_money' },
    ]);
  });

  it('skips segments that cannot be resolved', () => {
    const g = walletFixture();
    const result = breadcrumbs(g, ['project:root', 'module:nonexistent']);
    expect(result).toHaveLength(1);
    expect(result[0].kind).toBe('project');
  });
});
