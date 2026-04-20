import type { GraphJson } from '$lib/types';

/**
 * wallet_service fixture — 2 modules, 3 functions, 5 steps.
 * Module wallet:
 *   fn transfer_money: steps [debit_from_sender(ok), credit_to_payee(warn), validate_balance(ok)]
 *   fn get_balance:    steps [fetch_account(ok)]
 * Module auth:
 *   fn login: no steps
 * Types:  Money(ok), Account(warn)
 * Errors: InsufficientFunds(ok)
 */
export function walletFixture(): GraphJson {
  return {
    project: {
      id: 'project:root',
      name: 'wallet_service',
      description: 'Wallet service project',
      nodeCount: 9,
      moduleCount: 2,
      fnCount: 3,
      status: 'warn',
    },
    clusters: [{ id: 'default', name: 'default', color: '#2997ff' }],
    modules: [
      {
        id: 'module:m_wallet',
        name: 'wallet',
        description: 'Wallet module',
        cluster: 'default',
        clusterName: 'default',
        clusterColor: '#2997ff',
        status: 'warn',
        nodeCount: 5,
        functions: [
          {
            id: 'function:fn_transfer',
            name: 'transfer_money',
            status: 'warn',
            steps: [
              { id: 'step:s_debit',    name: 'debit_from_sender', status: 'ok',   intent: 'Debit sender' },
              { id: 'step:s_credit',   name: 'credit_to_payee',   status: 'warn', intent: 'Credit payee' },
              { id: 'step:s_validate', name: 'validate_balance',   status: 'ok',   intent: 'Validate balance' },
            ],
          },
          {
            id: 'function:fn_balance',
            name: 'get_balance',
            status: 'ok',
            steps: [
              { id: 'step:s_fetch', name: 'fetch_account', status: 'ok', intent: 'Fetch account' },
            ],
          },
        ],
      },
      {
        id: 'module:m_auth',
        name: 'auth',
        description: 'Auth module',
        cluster: 'default',
        clusterName: 'default',
        clusterColor: '#2997ff',
        status: 'ok',
        nodeCount: 1,
        functions: [
          {
            id: 'function:fn_login',
            name: 'login',
            status: 'ok',
            steps: [],
          },
        ],
      },
    ],
    externals: [],
    relations: [],
    types: [
      { id: 'type:t_money',   name: 'Money',   status: 'ok' },
      { id: 'type:t_account', name: 'Account', status: 'warn' },
    ],
    errors: [
      { id: 'error:e_insufficient', name: 'InsufficientFunds', status: 'ok' },
    ],
    detail: {},
  };
}

/**
 * walletFixtureWithFail — same as walletFixture but validate_balance step is 'fail'.
 * Used to test reactive status propagation.
 */
export function walletFixtureWithFail(): GraphJson {
  const g = walletFixture();
  const transferFn = g.modules[0].functions[0];
  const step = transferFn.steps!.find((s) => s.id === 'step:s_validate')!;
  step.status = 'fail';
  transferFn.status = 'fail';
  g.modules[0].status = 'fail';
  g.project.status = 'fail';
  return g;
}
