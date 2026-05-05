import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, selection, path, history, activeLens } from '$lib/stores';
import { zoomLevel } from '$lib/chrome/toolbar-state';
import {
  nodeViewActiveTab,
  nodeCodeLang,
  nodeTestResult,
  HISTORY_FIXTURE,
} from './node-view-state';
import { sheafConflicts, resetSheafState } from '$lib/sheaf/sheaf-state';
import { nodeDetailFixture } from './fixtures';
import NodeView from './NodeView.svelte';

beforeEach(() => {
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  path.set([]);
  history.set({ back: [], forward: [] });
  zoomLevel.set(4);
  nodeViewActiveTab.set('code');
  nodeCodeLang.set('python');
  nodeTestResult.set(null);
  activeLens.set('verify');
  resetSheafState();
});

describe('NodeView.svelte', () => {
  it('test_node_view_split_layout', async () => {
    const detail = nodeDetailFixture();
    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    // Both panels present
    expect(container.querySelector('[data-testid="node-view-detail"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="node-view-tabs"]')).not.toBeNull();
  });

  it('test_node_view_code_tab_python', async () => {
    const detail = nodeDetailFixture();
    nodeViewActiveTab.set('code');
    nodeCodeLang.set('python');

    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    const codeEl = container.querySelector('[data-testid="node-tab-code-text"]');
    expect(codeEl).not.toBeNull();
    expect(codeEl?.className).toContain('language-python');
    expect(codeEl?.textContent).toContain('def transfer_step');
  });

  it('test_node_view_code_lang_switch', async () => {
    const detail = nodeDetailFixture();
    nodeViewActiveTab.set('code');
    nodeCodeLang.set('python');

    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    // Switch to TypeScript
    const tsBtn = container.querySelector('[data-testid="node-code-lang-ts"]') as HTMLButtonElement;
    expect(tsBtn).not.toBeNull();
    fireEvent.click(tsBtn);
    await tick();

    expect(get(nodeCodeLang)).toBe('typescript');

    const codeEl = container.querySelector('[data-testid="node-tab-code-text"]');
    expect(codeEl?.className).toContain('language-typescript');
    expect(codeEl?.textContent).toContain('function transferStep');
  });

  it('test_node_view_proof_shows_counterexample', async () => {
    const detail = nodeDetailFixture(); // verification.ok = false, counterexample set
    nodeViewActiveTab.set('proof');

    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    const cex = container.querySelector('[data-testid="node-tab-proof-counterexample"]');
    expect(cex).not.toBeNull();
    expect(cex?.textContent).toContain('amount = 1000, balance = 500');
    expect(cex?.textContent).toContain('transfer proceeds despite insufficient funds');
    expect(cex?.textContent).toContain('balance >= amount');
  });

  // -------------------------------------------------------------------------
  // Phase 20 — verify-lens action buttons (Suggest fix / Relax rule / Add
  // handler). The buttons render only when verification.ok === false AND a
  // counterexample is present; events bubble to document for the route shell.
  // -------------------------------------------------------------------------

  it('test_proof_action_buttons_render_when_counterexample_present', async () => {
    const detail = nodeDetailFixture();
    nodeViewActiveTab.set('proof');
    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();
    expect(container.querySelector('[data-testid="proof-action-suggest-fix"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="proof-action-relax-rule"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="proof-action-add-handler"]')).not.toBeNull();
  });

  it('test_proof_action_buttons_absent_when_verification_ok', async () => {
    const detail = nodeDetailFixture();
    detail.verification = { ok: true };
    nodeViewActiveTab.set('proof');
    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();
    expect(container.querySelector('[data-testid="proof-action-suggest-fix"]')).toBeNull();
    expect(container.querySelector('[data-testid="proof-action-relax-rule"]')).toBeNull();
    expect(container.querySelector('[data-testid="proof-action-add-handler"]')).toBeNull();
  });

  // Svelte 5 `createEventDispatcher` only delivers events when a parent
  // template registered `on:event=` handlers (same caveat noted in
  // QuickCreateModal.test.ts). Isolated mounts cannot observe the dispatch
  // directly; the bubbling wire is exercised by the route shell. Here we
  // pin that the click does not throw and the buttons stay present —
  // sufficient regression coverage given the dispatcher quirk.

  it('test_proof_action_buttons_clickable_without_throwing', async () => {
    const detail = nodeDetailFixture();
    nodeViewActiveTab.set('proof');
    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();
    const ids = ['proof-action-suggest-fix', 'proof-action-relax-rule', 'proof-action-add-handler'];
    for (const id of ids) {
      const btn = container.querySelector(`[data-testid="${id}"]`) as HTMLButtonElement;
      expect(btn).not.toBeNull();
      // Clicks without a registered parent dispatcher must be no-throw.
      expect(() => fireEvent.click(btn)).not.toThrow();
    }
  });

  it('test_proof_action_buttons_hidden_when_stepId_empty', async () => {
    const detail = nodeDetailFixture();
    nodeViewActiveTab.set('proof');
    const { container } = render(NodeView, {
      props: { stepId: '', detail },
    });
    await tick();
    // Without a stepId we cannot scope the agent prompt; suppress the
    // affordance entirely.
    expect(container.querySelector('[data-testid="proof-action-suggest-fix"]')).toBeNull();
  });

  // v4.1: re-enable when Test tab wires real runner. v4.0 hides the Test tab
  // because runTestStub returns hard-coded passed:true.
  it.skip('test_node_view_test_run_populates_result', async () => {
    const detail = nodeDetailFixture();
    nodeViewActiveTab.set('test');

    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    const runBtn = container.querySelector('[data-testid="node-tab-test-run-btn"]') as HTMLButtonElement;
    expect(runBtn).not.toBeNull();
    fireEvent.click(runBtn);
    await tick();

    const result = get(nodeTestResult);
    expect(result).not.toBeNull();
    expect(result?.passed).toBe(true);
    expect(result?.message).toContain('All assertions passed');

    // UI should show result
    const resultEl = container.querySelector('[data-testid="node-tab-test-result"]');
    expect(resultEl).not.toBeNull();
  });

  // v4.1: re-enable with real commit history. v4.0 hides the History tab
  // because the only data source is Alice/Bob/2026-04-20 fixture.
  it.skip('test_node_view_history_tab', async () => {
    const detail = nodeDetailFixture();
    nodeViewActiveTab.set('history');

    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    const entries = container.querySelectorAll('[data-testid^="history-entry-"]');
    expect(entries.length).toBe(HISTORY_FIXTURE.length);
    expect(entries.length).toBe(3);

    // Verify fixture messages are shown
    expect(entries[0].textContent).toContain(HISTORY_FIXTURE[0].message);
    expect(entries[1].textContent).toContain(HISTORY_FIXTURE[1].message);
    expect(entries[2].textContent).toContain(HISTORY_FIXTURE[2].message);
  });

  it('test_node_view_receives_returns', async () => {
    const detail = nodeDetailFixture();
    // Detail has: receives: Account, Account, Money; returns: TxReceipt
    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    // Check in the detail card (left panel)
    const receivesTable = container.querySelector('[data-testid="node-detail-receives-table"]');
    expect(receivesTable).not.toBeNull();
    expect(receivesTable?.textContent).toContain('Account');

    const returnsTable = container.querySelector('[data-testid="node-detail-returns-table"]');
    expect(returnsTable).not.toBeNull();
    expect(returnsTable?.textContent).toContain('TxReceipt');
  });

  it('test_node_view_tab_switch_does_not_reset_lang', async () => {
    const detail = nodeDetailFixture();
    nodeViewActiveTab.set('code');
    nodeCodeLang.set('typescript');

    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    // Switch to proof tab
    const proofBtn = container.querySelector('[data-testid="node-tab-btn-proof"]') as HTMLButtonElement;
    fireEvent.click(proofBtn);
    await tick();
    expect(get(nodeViewActiveTab)).toBe('proof');

    // Switch back to code
    const codeBtn = container.querySelector('[data-testid="node-tab-btn-code"]') as HTMLButtonElement;
    fireEvent.click(codeBtn);
    await tick();
    expect(get(nodeViewActiveTab)).toBe('code');
    // Lang must still be typescript
    expect(get(nodeCodeLang)).toBe('typescript');
  });

  it('tests lens renders placeholder items without throwing', async () => {
    activeLens.set('tests');
    const detail = nodeDetailFixture();
    const { container } = render(NodeView, { props: { stepId: 'step:s_transfer', detail } });
    await tick();
    const sentinel = container.querySelector('[data-testid="node-detail-lens-tests"]');
    expect(sentinel).not.toBeNull();
    const items = container.querySelectorAll('[data-testid^="node-detail-lens-item-"]');
    expect(items.length).toBeGreaterThan(0);
    expect(items[0].textContent?.trim().length ?? 0).toBeGreaterThan(0);
  });

  it('detail=null across lens changes does not throw', async () => {
    const { container } = render(NodeView, { props: { stepId: 'step:s_transfer', detail: null } });
    for (const lens of ['verify', 'structure', 'tests'] as const) {
      activeLens.set(lens);
      await tick();
      const section = container.querySelector('[data-testid="node-detail-lens-section"]');
      expect(section).not.toBeNull();
    }
  });

  it('lens change does not reset nodeViewActiveTab or nodeCodeLang', async () => {
    const detail = nodeDetailFixture();
    render(NodeView, { props: { stepId: 'step:s_transfer', detail } });
    nodeViewActiveTab.set('proof');
    nodeCodeLang.set('typescript');
    activeLens.set('structure');
    await tick();
    activeLens.set('data');
    await tick();
    expect(get(nodeViewActiveTab)).toBe('proof');
    expect(get(nodeCodeLang)).toBe('typescript');
  });

  it('renders correct lens sentinel for each of the 5 lenses', async () => {
    const detail = nodeDetailFixture();
    const allLenses = ['structure', 'rules', 'verify', 'data', 'tests'] as const;
    const { container } = render(NodeView, { props: { stepId: 'step:s_transfer', detail } });
    for (const lens of allLenses) {
      activeLens.set(lens);
      await tick();
      expect(container.querySelector('[data-testid="node-detail-lens-' + lens + '"]')).not.toBeNull();
      for (const other of allLenses) {
        if (other === lens) continue;
        expect(container.querySelector('[data-testid="node-detail-lens-' + other + '"]')).toBeNull();
      }
      const heading = container.querySelector('[data-testid="node-detail-lens-heading"]');
      expect(heading).not.toBeNull();
      expect(heading?.textContent?.trim().length ?? 0).toBeGreaterThan(0);
    }
  });

  it('all 4 side tabs clickable and render corresponding tab body', async () => {
    const detail = nodeDetailFixture();
    const { container } = render(NodeView, { props: { stepId: 'step:s_transfer', detail } });
    // v4.1: re-add 'test' + 'history' when those tabs wire real data sources.
    const tabs = ['code', 'proof', 'types', 'rules'] as const;
    for (const id of tabs) {
      const btn = container.querySelector('[data-testid="node-tab-btn-' + id + '"]') as HTMLElement | null;
      expect(btn).not.toBeNull();
      btn?.click();
      await tick();
      expect(get(nodeViewActiveTab)).toBe(id);
      expect(container.querySelector('[data-testid="node-tab-' + id + '"]')).not.toBeNull();
      for (const other of tabs) {
        if (other === id) continue;
        expect(container.querySelector('[data-testid="node-tab-' + other + '"]')).toBeNull();
      }
    }
  });

  it('NV5: when $activeLens !== "verify", conflict section NOT in DOM', async () => {
    sheafConflicts.set([{
      overlapIndex: 0,
      nodeA: 'step:s_transfer',
      nodeB: 'step:s_other',
      conflictingA: ['x > 0'],
      conflictingB: ['x <= 0'],
    }]);
    activeLens.set('structure');
    const detail = nodeDetailFixture();
    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    expect(container.querySelector('[data-testid="node-view-conflict-section"]')).toBeNull();
  });

  it('NV6: when $activeLens === "verify" and sheafConflicts has matching entry, conflict section IS in DOM', async () => {
    sheafConflicts.set([{
      overlapIndex: 0,
      nodeA: 'step:s_transfer',
      nodeB: 'step:s_other',
      conflictingA: ['balance >= 0'],
      conflictingB: ['balance < 0'],
    }]);
    activeLens.set('verify');
    const detail = nodeDetailFixture();
    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    expect(container.querySelector('[data-testid="node-view-conflict-section"]')).not.toBeNull();
  });

  it('NV7: when $activeLens === "verify" but sheafConflicts has no matching entry, conflict section NOT in DOM', async () => {
    sheafConflicts.set([{
      overlapIndex: 0,
      nodeA: 'step:s_unrelated-a',
      nodeB: 'step:s_unrelated-b',
      conflictingA: ['x > 0'],
      conflictingB: ['x <= 0'],
    }]);
    activeLens.set('verify');
    const detail = nodeDetailFixture();
    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    expect(container.querySelector('[data-testid="node-view-conflict-section"]')).toBeNull();
  });

  // Acceptance pass 2026-05-02 — production-grade tab semantics. The side
  // panel had buttons but no role="tab" / role="tabpanel", so screen
  // readers heard them as plain buttons.
  it('exposes tablist + tab + tabpanel ARIA semantics on the side panel', async () => {
    const detail = nodeDetailFixture();
    const { container } = render(NodeView, {
      props: { stepId: 'step:s_transfer', detail },
    });
    await tick();

    const tablist = container.querySelector('[data-testid="node-view-tabs"]');
    expect(tablist?.getAttribute('role')).toBe('tablist');
    expect(tablist?.getAttribute('aria-label')).toBe('Node detail');

    const codeTab = container.querySelector('[data-testid="node-tab-btn-code"]');
    expect(codeTab?.getAttribute('role')).toBe('tab');
    expect(codeTab?.getAttribute('aria-controls')).toBe('node-tab-panel-code');
    // Exactly one tab has aria-selected=true at any time.
    const selected = container.querySelectorAll('[role="tab"][aria-selected="true"]');
    expect(selected.length).toBe(1);

    const panel = container.querySelector('[role="tabpanel"]');
    expect(panel).not.toBeNull();
    expect(panel?.getAttribute('aria-labelledby')?.startsWith('node-tab-btn-')).toBe(true);
  });
});
