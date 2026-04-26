import { describe, it, expect } from 'vitest';
import { collectScopeIds } from './collect-scope-ids';
import type { GraphJson } from '$lib/types';

/**
 * Small fixture: 1 module, 2 functions, each with 2 steps.
 *
 * module:m_wallet
 *   function:fn_transfer
 *     step:s_debit
 *     step:s_credit
 *   function:fn_balance
 *     step:s_fetch
 *     step:s_return
 */
function smallFixture(): GraphJson {
  return {
    project: {
      id: 'project:root',
      name: 'test',
      description: '',
      nodeCount: 9,
      moduleCount: 1,
      fnCount: 2,
      status: 'ok',
    },
    clusters: [],
    modules: [
      {
        id: 'module:m_wallet',
        name: 'Wallet',
        description: '',
        cluster: '',
        clusterName: '',
        clusterColor: '#2997ff',
        status: 'ok',
        nodeCount: 7,
        functions: [
          {
            id: 'function:fn_transfer',
            name: 'transfer',
            status: 'ok',
            steps: [
              { id: 'step:s_debit',  name: 'debit',  status: 'ok', intent: '' },
              { id: 'step:s_credit', name: 'credit', status: 'ok', intent: '' },
            ],
          },
          {
            id: 'function:fn_balance',
            name: 'balance',
            status: 'ok',
            steps: [
              { id: 'step:s_fetch',  name: 'fetch',  status: 'ok', intent: '' },
              { id: 'step:s_return', name: 'return', status: 'ok', intent: '' },
            ],
          },
        ],
      },
    ],
    externals: [],
    relations: [],
    types: [],
    errors: [],
    issues: [],
    detail: {},
  };
}

describe('collect-scope-ids.ts', () => {
  it('project scope returns all node ids', () => {
    const graph = smallFixture();
    const ids = collectScopeIds(graph, 'project', undefined);
    expect(ids).toContain('module:m_wallet');
    expect(ids).toContain('function:fn_transfer');
    expect(ids).toContain('function:fn_balance');
    expect(ids).toContain('step:s_debit');
    expect(ids).toContain('step:s_credit');
    expect(ids).toContain('step:s_fetch');
    expect(ids).toContain('step:s_return');
    expect(ids.length).toBe(7);
  });

  it('module scope returns module + functions + steps under it', () => {
    const graph = smallFixture();
    const ids = collectScopeIds(graph, 'module', 'module:m_wallet');
    expect(ids).toContain('module:m_wallet');
    expect(ids).toContain('function:fn_transfer');
    expect(ids).toContain('function:fn_balance');
    expect(ids).toContain('step:s_debit');
    expect(ids).toContain('step:s_credit');
    expect(ids).toContain('step:s_fetch');
    expect(ids).toContain('step:s_return');
    expect(ids.length).toBe(7);
  });

  it('function scope returns function + parent module + steps under it', () => {
    const graph = smallFixture();
    const ids = collectScopeIds(graph, 'function', 'function:fn_transfer');
    expect(ids).toContain('function:fn_transfer');
    expect(ids).toContain('module:m_wallet');
    expect(ids).toContain('step:s_debit');
    expect(ids).toContain('step:s_credit');
    // fn_balance and its steps must NOT be present
    expect(ids).not.toContain('function:fn_balance');
    expect(ids).not.toContain('step:s_fetch');
    expect(ids.length).toBe(4);
  });

  it('step scope returns step + parent function + grandparent module', () => {
    const graph = smallFixture();
    const ids = collectScopeIds(graph, 'step', 'step:s_credit');
    expect(ids).toContain('step:s_credit');
    expect(ids).toContain('function:fn_transfer');
    expect(ids).toContain('module:m_wallet');
    expect(ids.length).toBe(3);
  });

  it('unknown id returns empty array', () => {
    const graph = smallFixture();
    expect(collectScopeIds(graph, 'module', 'module:does_not_exist')).toEqual([]);
    expect(collectScopeIds(graph, 'function', 'function:no_such_fn')).toEqual([]);
    expect(collectScopeIds(graph, 'step', 'step:no_such_step')).toEqual([]);
  });
});
