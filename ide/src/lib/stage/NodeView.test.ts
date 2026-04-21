import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, selection, path, history } from '$lib/stores';
import { zoomLevel } from '$lib/chrome/toolbar-state';
import {
  nodeViewActiveTab,
  nodeCodeLang,
  nodeTestResult,
  HISTORY_FIXTURE,
} from './node-view-state';
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

  it('test_node_view_test_run_populates_result', async () => {
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

  it('test_node_view_history_tab', async () => {
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
});
