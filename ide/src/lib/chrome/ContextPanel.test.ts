import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';

import { graph, selection, path } from '$lib/stores';
import { contextActiveTab, contextTestResult, contextTestScenario } from './context-panel-state';
import { nodeDetailFixture } from '$lib/stage/fixtures';
import { walletFixture } from './fixtures';
import type { GraphJson, NodeDetail } from '$lib/types';
import ContextPanel from './ContextPanel.svelte';

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/**
 * Build a GraphJson with a single detail entry injected.
 * walletFixture() has detail:{} so we must inject here — confirmed by reading
 * ide/src/lib/chrome/fixtures.ts.
 */
function graphWithDetail(id: string, detail: NodeDetail): GraphJson {
  const base = walletFixture();
  return { ...base, detail: { ...base.detail, [id]: detail } };
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

beforeEach(() => {
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  path.set([]);
  contextActiveTab.set('overview');
  contextTestResult.set(null);
  contextTestScenario.set('');
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('ContextPanel.svelte', () => {
  it('test_context_panel_hidden_no_selection', async () => {
    const { container } = render(ContextPanel);
    await tick();

    expect(container.querySelector('[data-testid="ctx-header"]')).toBeNull();
    expect(container.querySelector('[data-testid="ctx-tabs"]')).toBeNull();
    expect(container.querySelector('[data-testid="ctx-footer"]')).toBeNull();
  });

  it('test_context_panel_tabs_switch', async () => {
    const detail = nodeDetailFixture();
    const nodeId = 'step:s_transfer_step';

    graph.set(graphWithDetail(nodeId, detail));
    selection.set({ kind: 'step', id: nodeId });

    const { container } = render(ContextPanel);
    await tick();

    // Default tab should be overview
    expect(container.querySelector('[data-testid="ctx-overview"]')).not.toBeNull();

    // Click rules tab
    const rulesTabBtn = container.querySelector('[data-testid="ctx-tab-btn-rules"]');
    expect(rulesTabBtn).not.toBeNull();
    fireEvent.click(rulesTabBtn!);
    await tick();

    expect(get(contextActiveTab)).toBe('rules');
    expect(container.querySelector('[data-testid="ctx-tab-rules"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="ctx-overview"]')).toBeNull();
  });

  it('test_overview_shows_description', async () => {
    const detail = nodeDetailFixture();
    const nodeId = 'step:s_transfer_step';

    graph.set(graphWithDetail(nodeId, detail));
    selection.set({ kind: 'step', id: nodeId });

    const { container } = render(ContextPanel);
    await tick();

    const descEl = container.querySelector('[data-testid="ctx-description"]');
    expect(descEl).not.toBeNull();
    expect(descEl!.textContent).toContain(detail.description);
  });

  it('test_rules_tab_inherited_separated', async () => {
    const detail = nodeDetailFixture();
    const nodeId = 'step:s_transfer_step';

    graph.set(graphWithDetail(nodeId, detail));
    selection.set({ kind: 'step', id: nodeId });
    contextActiveTab.set('rules');

    const { container } = render(ContextPanel);
    await tick();

    // Own rules section
    expect(container.querySelector('[data-testid="ctx-rules-own"]')).not.toBeNull();

    // Inherited section — gated by inheritedList.length > 0
    const inheritedSection = container.querySelector('[data-testid="ctx-rules-inherited"]');
    expect(inheritedSection).not.toBeNull();

    // The fixture has inherited from 'wallet_module'
    const inheritedGroup = container.querySelector(
      '[data-testid="ctx-inherited-from-wallet_module"]',
    );
    expect(inheritedGroup).not.toBeNull();

    // Both rules visible in that group
    expect(inheritedGroup!.textContent).toContain('actor authenticated');
    expect(inheritedGroup!.textContent).toContain('currency valid');

    // "from wallet_module" label visible
    expect(inheritedGroup!.textContent).toContain('from wallet_module');
  });

  it('test_status_failed_red_text', async () => {
    const detail = nodeDetailFixture(); // verification.ok === false
    const nodeId = 'step:s_transfer_step';

    graph.set(graphWithDetail(nodeId, detail));
    selection.set({ kind: 'step', id: nodeId });

    const { container } = render(ContextPanel);
    await tick();

    const statusEl = container.querySelector('[data-testid="ctx-status-text"]');
    expect(statusEl).not.toBeNull();
    expect((statusEl as HTMLElement).classList.contains('status-fail')).toBe(true);
    expect(statusEl!.textContent).toContain('✗ Verification failed');
  });

  it('test_context_panel_close_clears_path', async () => {
    const detail = nodeDetailFixture();
    const nodeId = 'function:fn_transfer';

    graph.set(graphWithDetail(nodeId, detail));
    selection.set({ kind: 'function', id: nodeId });
    path.set(['project:root', 'module:m_wallet', 'function:fn_transfer']);

    const { container } = render(ContextPanel);
    await tick();

    const closeBtn = container.querySelector('[data-testid="ctx-close-btn"]');
    expect(closeBtn).not.toBeNull();
    fireEvent.click(closeBtn!);
    await tick();

    expect(get(selection)).toEqual({ kind: 'none', id: null });
    expect(get(path)).toEqual([]);
  });

  it('test_context_panel_footer_always_visible', async () => {
    const detail = nodeDetailFixture();
    const nodeId = 'step:s_transfer_step';

    graph.set(graphWithDetail(nodeId, detail));
    selection.set({ kind: 'step', id: nodeId });

    const { container } = render(ContextPanel);
    await tick();

    const footer = container.querySelector('[data-testid="ctx-footer"]');
    expect(footer).not.toBeNull();

    // Footer must NOT be a child of the tab body
    const tabBody = container.querySelector('[data-testid="ctx-tab-body"]');
    expect(tabBody).not.toBeNull();
    expect(tabBody!.contains(footer)).toBe(false);
  });
});
