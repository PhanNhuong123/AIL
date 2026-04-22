/**
 * Stage-local fixtures.
 *
 * multiClusterFixture — 3 clusters × 2 modules each; drives cluster tests.
 * bigSystemFixture(n) — parameterised generator for 16.5-B render tests.
 * detailedModuleFixture — walletFixture + populated NodeDetail entries.
 * flowFixture — 7 nodes (all FlowNodeKind variants) + 7 edges (task 16.6).
 * nodeDetailFixture — full NodeDetail with code blobs, rules, counterexample (task 16.6).
 */

import type {
  GraphJson,
  ModuleJson,
  FunctionJson,
  StepJson,
  NodeDetail,
  Status,
  FlowchartJson,
  FlowNodeJson,
  FlowEdgeJson,
} from '$lib/types';
import { walletFixture } from '$lib/chrome/fixtures';

const CLUSTER_IDENTITY = { id: 'c_identity', name: 'Identity & Access', color: '#3ecf8e' };
const CLUSTER_MONEY    = { id: 'c_money',    name: 'Money Movement',   color: '#2997ff' };
const CLUSTER_GROWTH   = { id: 'c_growth',   name: 'Growth & Rewards', color: '#f0b429' };

function mkStep(id: string, name: string, status: Status, intent = ''): StepJson {
  return { id: `step:${id}`, name, status, intent: intent || name };
}

function mkFn(
  id: string,
  name: string,
  status: Status,
  steps: StepJson[] = [],
): FunctionJson {
  return { id: `function:${id}`, name, status, steps };
}

function mkModule(
  id: string,
  name: string,
  cluster: { id: string; name: string; color: string },
  status: Status,
  fns: FunctionJson[],
): ModuleJson {
  const nodeCount = 1 + fns.length + fns.reduce((acc, f) => acc + (f.steps?.length ?? 0), 0);
  return {
    id: `module:${id}`,
    name,
    description: `${name} module`,
    cluster: cluster.id,
    clusterName: cluster.name,
    clusterColor: cluster.color,
    status,
    nodeCount,
    functions: fns,
  };
}

export function multiClusterFixture(): GraphJson {
  const m_auth = mkModule('m_auth', 'Auth', CLUSTER_IDENTITY, 'ok', [
    mkFn('fn_login', 'login', 'ok', [mkStep('s_check_pw', 'check_password', 'ok')]),
    mkFn('fn_logout', 'logout', 'ok', [mkStep('s_clear', 'clear_session', 'ok')]),
  ]);
  const m_session = mkModule('m_session', 'Session', CLUSTER_IDENTITY, 'ok', [
    mkFn('fn_refresh', 'refresh', 'ok', [mkStep('s_renew', 'renew_token', 'ok')]),
    mkFn('fn_revoke', 'revoke', 'ok', []),
  ]);

  const m_billing = mkModule('m_billing', 'Billing', CLUSTER_MONEY, 'ok', [
    mkFn('fn_invoice', 'invoice', 'ok', [mkStep('s_gen_inv', 'generate_invoice', 'ok')]),
    mkFn('fn_refund', 'refund', 'ok', [mkStep('s_refund_tx', 'refund_tx', 'ok')]),
  ]);
  const m_wallet = mkModule('m_wallet', 'Wallet', CLUSTER_MONEY, 'warn', [
    mkFn('fn_transfer', 'transfer', 'warn', [
      mkStep('s_debit', 'debit_from', 'ok'),
      mkStep('s_credit', 'credit_to', 'warn'),
    ]),
    mkFn('fn_balance', 'balance', 'ok', [mkStep('s_fetch_acct', 'fetch_account', 'ok')]),
  ]);

  const m_rewards = mkModule('m_rewards', 'Rewards', CLUSTER_GROWTH, 'ok', [
    mkFn('fn_grant', 'grant', 'ok', [mkStep('s_add_pts', 'add_points', 'ok')]),
    mkFn('fn_query', 'query', 'ok', []),
  ]);
  const m_promos = mkModule('m_promos', 'Promotions', CLUSTER_GROWTH, 'fail', [
    mkFn('fn_redeem', 'redeem', 'fail', [mkStep('s_apply_promo', 'apply_promo', 'fail')]),
    mkFn('fn_list', 'list', 'ok', []),
  ]);

  const modules = [m_auth, m_session, m_billing, m_wallet, m_rewards, m_promos];
  const nodeCount = modules.reduce((acc, m) => acc + m.nodeCount, 0);

  const billingDetail: NodeDetail = {
    name: 'Billing',
    status: 'ok',
    description: 'Billing module',
    receives: [{ name: 'req', desc: 'InvoiceReq' }],
    returns: [{ name: 'res', desc: 'InvoiceRes' }],
    rules: [
      { text: 'amount > 0', source: 'own' },
      { text: 'currency is ISO', source: 'own' },
    ],
    inherited: [],
    proven: ['amount > 0', 'currency is ISO'],
    verification: { ok: true },
  };

  const transferDetail: NodeDetail = {
    name: 'transfer',
    status: 'warn',
    description: 'Transfer money',
    receives: [{ name: 'from', desc: 'Account' }, { name: 'amount', desc: 'Money' }],
    returns: [{ name: 'result', desc: 'TxReceipt' }],
    rules: [
      { text: 'balance >= amount', source: 'own' },
      { text: 'not self transfer', source: 'own' },
    ],
    inherited: [],
    proven: ['not self transfer'],
    verification: { ok: false },
  };

  return {
    project: {
      id: 'project:root',
      name: 'multi_cluster_demo',
      description: 'Multi-cluster demo project',
      nodeCount,
      moduleCount: modules.length,
      fnCount: modules.reduce((acc, m) => acc + m.functions.length, 0),
      status: 'fail',
    },
    clusters: [CLUSTER_IDENTITY, CLUSTER_MONEY, CLUSTER_GROWTH],
    modules,
    externals: [
      { id: 'external:ext_stripe', name: 'Stripe', description: 'Card processor' },
    ],
    relations: [
      { from: 'module:m_wallet', to: 'module:m_billing', label: 'uses' },
      { from: 'module:m_billing', to: 'external:ext_stripe', label: 'uses' },
      { from: 'module:m_rewards', to: 'module:m_wallet', label: 'uses' },
    ],
    types: [{ id: 'type:t_money', name: 'Money', status: 'ok' }],
    errors: [{ id: 'error:e_decline', name: 'CardDeclined', status: 'fail' }],
    issues: [],
    detail: {
      'module:m_billing': billingDetail,
      'function:fn_transfer': transferDetail,
    },
  };
}

export function detailedModuleFixture(): GraphJson {
  const g = walletFixture();
  const walletDetail: NodeDetail = {
    name: 'wallet',
    status: 'warn',
    description: 'Wallet module',
    receives: [{ name: 'actor', desc: 'Account' }],
    returns: [{ name: 'state', desc: 'WalletState' }],
    rules: [
      { text: 'balance >= 0', source: 'own' },
      { text: 'currency matches', source: 'own' },
      { text: 'actor authenticated', source: 'own' },
    ],
    inherited: [],
    proven: ['balance >= 0', 'actor authenticated'],
    verification: { ok: false },
  };
  const transferDetail: NodeDetail = {
    name: 'transfer_money',
    status: 'warn',
    description: 'Transfer funds',
    receives: [{ name: 'from', desc: 'Account' }, { name: 'to', desc: 'Account' }],
    returns: [{ name: 'receipt', desc: 'TxReceipt' }],
    rules: [
      { text: 'not self transfer', source: 'own' },
      { text: 'amount > 0', source: 'own' },
    ],
    inherited: [],
    proven: ['amount > 0'],
    verification: { ok: false },
  };
  g.detail['module:m_wallet'] = walletDetail;
  g.detail['function:fn_transfer'] = transferDetail;
  return g;
}

export function bigSystemFixture(n = 200): GraphJson {
  const targetNodes = Math.max(20, n);
  const clusterCount = 5;
  const modulesPerCluster = 4;
  const moduleCount = clusterCount * modulesPerCluster;
  const remaining = targetNodes - 1 - moduleCount;
  const fnsPerModule = Math.max(1, Math.floor(remaining / (moduleCount * 3)));
  const stepsPerFn = Math.max(1, Math.floor(remaining / (moduleCount * fnsPerModule + 1)));

  const clusters = Array.from({ length: clusterCount }, (_, i) => ({
    id: `c_big_${i}`,
    name: `Cluster ${i}`,
    color: '#2997ff',
  }));

  const modules: ModuleJson[] = [];
  let moduleIdx = 0;
  for (let ci = 0; ci < clusterCount; ci++) {
    for (let mi = 0; mi < modulesPerCluster; mi++) {
      const fns: FunctionJson[] = [];
      for (let fi = 0; fi < fnsPerModule; fi++) {
        const steps: StepJson[] = [];
        for (let si = 0; si < stepsPerFn; si++) {
          steps.push({
            id: `step:s_big_${moduleIdx}_${fi}_${si}`,
            name: `step_${si}`,
            status: 'ok',
            intent: '',
          });
        }
        fns.push({
          id: `function:fn_big_${moduleIdx}_${fi}`,
          name: `fn_${fi}`,
          status: 'ok',
          steps,
        });
      }
      modules.push(
        mkModule(
          `m_big_${moduleIdx}`,
          `Module ${moduleIdx}`,
          clusters[ci],
          'ok',
          fns,
        ),
      );
      moduleIdx++;
    }
  }

  const nodeCount = 1 + modules.reduce((acc, m) => acc + m.nodeCount, 0);
  const fnCount = modules.reduce((acc, m) => acc + m.functions.length, 0);

  return {
    project: {
      id: 'project:root',
      name: 'big_system',
      description: `Big system fixture (~${targetNodes} nodes)`,
      nodeCount,
      moduleCount: modules.length,
      fnCount,
      status: 'ok',
    },
    clusters,
    modules,
    externals: [],
    relations: [],
    types: [],
    errors: [],
    issues: [],
    detail: {},
  };
}

// ---------------------------------------------------------------------------
// Task 16.6: Flow + Node fixtures
// ---------------------------------------------------------------------------

/**
 * flowFixture — returns a FlowchartJson with 7 nodes (all FlowNodeKind
 * variants) and 7 edges (yes/ok/no/err/neutral labels + styles).
 * Also exposes the fixture as a GraphJson wrapper for Stage.svelte testing.
 */
export function flowFixture(): { flowchart: FlowchartJson; graph: GraphJson } {
  const nodes: FlowNodeJson[] = [
    { id: 'n_start',  kind: 'start',    label: 'Start',         x: 200, y: 20  },
    { id: 'n_do',     kind: 'process',  label: 'Validate',      x: 200, y: 100 },
    { id: 'n_decide', kind: 'decision', label: 'Balance OK?',   x: 200, y: 200 },
    { id: 'n_io',     kind: 'io',       label: 'Read Account',  x: 400, y: 200 },
    { id: 'n_sub',    kind: 'sub',      label: 'Audit Log',     x: 200, y: 320 },
    { id: 'n_store',  kind: 'storage',  label: 'DB Write',      x: 200, y: 440 },
    { id: 'n_end',    kind: 'end',      label: 'End',           x: 200, y: 540 },
  ];

  const edges: FlowEdgeJson[] = [
    { from: 'n_start',  to: 'n_do',     label: undefined,  style: undefined   },
    { from: 'n_do',     to: 'n_decide', label: undefined,  style: undefined   },
    { from: 'n_decide', to: 'n_sub',    label: 'yes',      style: 'ok'        },
    { from: 'n_decide', to: 'n_io',     label: 'no',       style: 'err'       },
    { from: 'n_io',     to: 'n_end',    label: undefined,  style: undefined   },
    { from: 'n_sub',    to: 'n_store',  label: 'ok',       style: 'ok'        },
    { from: 'n_store',  to: 'n_end',    label: undefined,  style: undefined   },
  ];

  const flowchart: FlowchartJson = { nodes, edges };

  // Minimal GraphJson wrapper so Stage.svelte can hydrate `activeFlowchart`
  const graph: GraphJson = {
    project: {
      id: 'project:root',
      name: 'flow_demo',
      description: 'Flow fixture project',
      nodeCount: 1,
      moduleCount: 1,
      fnCount: 1,
      status: 'ok',
    },
    clusters: [],
    modules: [
      {
        id: 'module:m_demo',
        name: 'demo',
        description: 'Demo module',
        cluster: '',
        clusterName: '',
        clusterColor: '#2997ff',
        status: 'ok',
        nodeCount: 1,
        functions: [
          {
            id: 'function:fn_demo',
            name: 'demo_fn',
            status: 'ok',
            steps: [
              { id: 'step:s_demo', name: 'demo_step', status: 'ok', intent: 'demo' },
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

  return { flowchart, graph };
}

/**
 * nodeDetailFixture — full NodeDetail with:
 *  - CodeBlob in both Python and TypeScript
 *  - 3 receives + 1 return
 *  - 3 own rules + 2 inherited rules
 *  - 2 proven facts
 *  - populated counterexample (verification.ok = false)
 */
export function nodeDetailFixture(): NodeDetail {
  return {
    name: 'transfer_step',
    status: 'fail',
    description: 'Validates and executes a fund transfer between two accounts.',
    receives: [
      { name: 'from',   desc: 'Account'  },
      { name: 'to',     desc: 'Account'  },
      { name: 'amount', desc: 'Money'    },
    ],
    returns: [
      { name: 'receipt', desc: 'TxReceipt' },
    ],
    rules: [
      { text: 'amount > 0',          source: 'own'       },
      { text: 'from != to',          source: 'own'       },
      { text: 'balance >= amount',   source: 'own'       },
    ],
    inherited: [
      { text: 'actor authenticated', from: 'wallet_module' },
      { text: 'currency valid',      from: 'wallet_module' },
    ],
    proven: ['amount > 0', 'from != to'],
    verification: {
      ok: false,
      counterexample: {
        scenario: 'amount = 1000, balance = 500',
        effect:   'transfer proceeds despite insufficient funds',
        violates: 'balance >= amount',
      },
    },
    code: {
      python: `def transfer_step(from_account, to_account, amount):\n    assert amount > 0\n    assert from_account != to_account\n    if from_account.balance < amount:\n        raise InsufficientFunds()\n    from_account.balance -= amount\n    to_account.balance += amount\n    return TxReceipt(ok=True)`,
      typescript: `function transferStep(from: Account, to: Account, amount: Money): TxReceipt {\n  if (amount <= 0) throw new Error('amount must be positive');\n  if (from.id === to.id) throw new Error('self-transfer not allowed');\n  if (from.balance < amount) throw new InsufficientFundsError();\n  from.balance -= amount;\n  to.balance += amount;\n  return { ok: true };\n}`,
    },
  };
}
