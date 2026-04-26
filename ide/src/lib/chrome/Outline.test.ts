import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, selection, path, history } from '$lib/stores';
import { tweaksPanelOpen } from '$lib/stores';
import { expanded, filterTerm } from './outline-state';
import { zoomLevel } from './toolbar-state';
import { walletFixture, walletFixtureWithFail } from './fixtures';
import { sheafConflicts, resetSheafState } from '$lib/sheaf/sheaf-state';
import Outline from './Outline.svelte';

beforeEach(() => {
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  path.set([]);
  tweaksPanelOpen.set(false);
  expanded.set(new Set(['project:root']));
  filterTerm.set('');
  zoomLevel.set(0);
  history.set({ back: [], forward: [] });
  resetSheafState();
});

describe('Outline.svelte', () => {
  it('renders project, modules, functions when graph is loaded', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();

    const rows = container.querySelectorAll('.nav-row');
    // project + 2 modules = 3 visible rows (modules not expanded yet)
    expect(rows.length).toBeGreaterThanOrEqual(3);

    const text = container.textContent ?? '';
    expect(text).toContain('wallet_service');
    expect(text).toContain('wallet');
    expect(text).toContain('auth');
    // PROJECT section header should be visible
    expect(text).toContain('PROJECT');
  });

  it('renders PROJECT, TYPES, and ERRORS section labels', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();

    const text = container.textContent ?? '';
    expect(text).toContain('PROJECT');
    expect(text).toContain('TYPES');
    expect(text).toContain('ERRORS');
  });

  it('region root exposes data-testid="region-outline"', () => {
    const { container } = render(Outline);
    expect(container.querySelector('[data-testid="region-outline"]')).not.toBeNull();
  });

  it('filter keeps ancestors visible (15.4-A)', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();

    filterTerm.set('credit');
    await tick();

    const text = container.textContent ?? '';
    // step name should be visible
    expect(text).toContain('credit_to_payee');
    // direct function ancestor should be visible
    expect(text).toContain('transfer_money');
    // module ancestor should also be visible
    expect(text).toContain('wallet');
    // sibling module 'auth' should NOT appear
    expect(text).not.toContain('auth');
  });

  it('row click updates selection and path stores', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();

    // Click the wallet module row
    const rows = container.querySelectorAll('.nav-row');
    // row 0 = project, row 1 = wallet module, row 2 = auth module
    fireEvent.click(rows[1]);
    await tick();

    const sel = get(selection);
    expect(sel.id).toBe('module:m_wallet');

    const p = get(path);
    expect(p).toContain('module:m_wallet');
  });

  it('status dots have correct class for each status', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();

    // wallet module has status warn → dot-warn
    const warnDots = container.querySelectorAll('.dot-warn');
    expect(warnDots.length).toBeGreaterThan(0);

    // auth module has status ok → dot-ok
    const okDots = container.querySelectorAll('.dot-ok');
    expect(okDots.length).toBeGreaterThan(0);
  });

  it('parent status updates reactively when graph is patched (15.4-A)', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();

    // Initially wallet module is warn
    expect(container.querySelectorAll('.dot-warn').length).toBeGreaterThan(0);
    expect(container.querySelectorAll('.dot-fail').length).toBe(0);

    // Swap to fixture with fail status
    graph.set(walletFixtureWithFail());
    await tick();

    // Now fail dots should appear
    expect(container.querySelectorAll('.dot-fail').length).toBeGreaterThan(0);
  });

  it('breadcrumb navigation: path store is set correctly on row click', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();

    const rows = container.querySelectorAll('.nav-row');
    fireEvent.click(rows[0]); // project row
    await tick();

    const p = get(path);
    expect(p.length).toBe(1);
    expect(p[0]).toBe('project:root');
  });

  it('expand/collapse toggles children visibility', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();

    // Initially wallet module is visible but NOT expanded → functions hidden
    let text = container.textContent ?? '';
    expect(text).not.toContain('transfer_money');

    // Expand the wallet module
    expanded.update((s) => {
      const n = new Set(s);
      n.add('module:m_wallet');
      return n;
    });
    await tick();

    text = container.textContent ?? '';
    expect(text).toContain('transfer_money');
    expect(text).toContain('get_balance');

    // Collapse the wallet module
    expanded.update((s) => {
      const n = new Set(s);
      n.delete('module:m_wallet');
      return n;
    });
    await tick();

    text = container.textContent ?? '';
    expect(text).not.toContain('transfer_money');
  });

  it('OutlineRow exposes data-kind for each row type (icon stability)', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();

    // Expand wallet module + transfer_money function so all 6 kinds are present
    expanded.update((s) => {
      const n = new Set(s);
      n.add('module:m_wallet');
      n.add('function:fn_transfer');
      return n;
    });
    await tick();

    expect(container.querySelector('[data-kind="project"]')).not.toBeNull();
    expect(container.querySelector('[data-kind="module"]')).not.toBeNull();
    expect(container.querySelector('[data-kind="function"]')).not.toBeNull();
    expect(container.querySelector('[data-kind="step"]')).not.toBeNull();
    expect(container.querySelector('[data-kind="type"]')).not.toBeNull();
    expect(container.querySelector('[data-kind="error"]')).not.toBeNull();
  });

  it('clicking a project row sets zoomLevel to 0 (15.4-B)', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();
    zoomLevel.set(2); // start off 0 to prove the set happened

    const projectRow = container.querySelector('[data-kind="project"]') as HTMLElement;
    fireEvent.click(projectRow);
    await tick();

    expect(get(zoomLevel)).toBe(0);
  });

  it('clicking a module row sets zoomLevel to 1 via stageLevelForKind (15.4-B)', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();

    const moduleRow = container.querySelector('[data-kind="module"]') as HTMLElement;
    fireEvent.click(moduleRow);
    await tick();

    expect(get(zoomLevel)).toBe(1);
  });

  it('clicking a function row sets zoomLevel to 2 (15.4-B)', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();
    // expand wallet so function row is present
    expanded.update((s) => {
      const n = new Set(s);
      n.add('module:m_wallet');
      return n;
    });
    await tick();

    const functionRow = container.querySelector('[data-kind="function"]') as HTMLElement;
    fireEvent.click(functionRow);
    await tick();

    expect(get(zoomLevel)).toBe(2);
  });

  it('clicking a step row sets zoomLevel to 4 (15.4-B)', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();
    expanded.update((s) => {
      const n = new Set(s);
      n.add('module:m_wallet');
      n.add('function:fn_transfer');
      return n;
    });
    await tick();

    const stepRow = container.querySelector('[data-kind="step"]') as HTMLElement;
    fireEvent.click(stepRow);
    await tick();

    expect(get(zoomLevel)).toBe(4);
  });

  it('clicking a type row falls back to zoomLevel 0 via stageLevelForKind default (15.4-B)', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();
    zoomLevel.set(2); // start off 0 to confirm the set happened

    const typeRow = container.querySelector('[data-kind="type"]') as HTMLElement;
    fireEvent.click(typeRow);
    await tick();

    // stageLevelForKind('type') falls through the switch default and returns 0
    expect(get(zoomLevel)).toBe(0);
  });

  it('clicking an error row falls back to zoomLevel 0 via stageLevelForKind default (15.4-B)', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();
    zoomLevel.set(2);

    const errorRow = container.querySelector('[data-kind="error"]') as HTMLElement;
    fireEvent.click(errorRow);
    await tick();

    // stageLevelForKind('error') falls through the switch default and returns 0
    expect(get(zoomLevel)).toBe(0);
  });

  it('Outline row clicks do not push toolbar history (15.4-B)', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    await tick();

    const rows = container.querySelectorAll('.nav-row');
    fireEvent.click(rows[1]); // wallet module
    await tick();
    fireEvent.click(rows[0]); // project row
    await tick();

    const h = get(history);
    expect(h.back.length).toBe(0);
    expect(h.forward.length).toBe(0);
  });

  it('OL1: when sheafConflicts contains a step id, the step wrapper has data-conflict="true"', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    // Expand to show steps
    expanded.update((s) => {
      const n = new Set(s);
      n.add('module:m_wallet');
      n.add('function:fn_transfer');
      return n;
    });
    await tick();

    // Set a conflict for debit step
    sheafConflicts.set([{
      overlapIndex: 0,
      nodeA: 'step:s_debit',
      nodeB: 'step:s_credit',
      conflictingA: ['x > 0'],
      conflictingB: ['x <= 0'],
    }]);
    await tick();

    // Find all wrappers with data-conflict — should be on the step wrappers only
    const conflictDivs = container.querySelectorAll('[data-conflict="true"]');
    expect(conflictDivs.length).toBeGreaterThanOrEqual(1);

    // Both step:s_debit and step:s_credit wrappers should have data-conflict
    // (wrapper contains the OutlineRow which has data-kind="step")
    let foundConflictOnStep = false;
    for (const div of conflictDivs) {
      const row = div.querySelector('[data-kind="step"]');
      if (row) foundConflictOnStep = true;
    }
    expect(foundConflictOnStep).toBe(true);
  });

  it('OL2: module wrapper does NOT have data-conflict attribute even when its child step is conflicting', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    expanded.update((s) => {
      const n = new Set(s);
      n.add('module:m_wallet');
      n.add('function:fn_transfer');
      return n;
    });
    await tick();

    sheafConflicts.set([{
      overlapIndex: 0,
      nodeA: 'step:s_debit',
      nodeB: 'step:s_credit',
      conflictingA: ['x > 0'],
      conflictingB: ['x <= 0'],
    }]);
    await tick();

    // Module wrappers: these contain [data-kind="module"] rows but should not have data-conflict
    // Find all divs that directly contain a module row
    const moduleRows = container.querySelectorAll('[data-kind="module"]');
    for (const row of moduleRows) {
      const wrapper = row.parentElement;
      expect(wrapper?.getAttribute('data-conflict')).toBeNull();
    }
  });

  it('OL3: function wrapper does NOT have data-conflict attribute even when its child step is conflicting', async () => {
    const { container } = render(Outline);
    graph.set(walletFixture());
    expanded.update((s) => {
      const n = new Set(s);
      n.add('module:m_wallet');
      n.add('function:fn_transfer');
      return n;
    });
    await tick();

    sheafConflicts.set([{
      overlapIndex: 0,
      nodeA: 'step:s_debit',
      nodeB: 'step:s_credit',
      conflictingA: ['x > 0'],
      conflictingB: ['x <= 0'],
    }]);
    await tick();

    // Function wrappers: these contain [data-kind="function"] rows but should not have data-conflict
    const functionRows = container.querySelectorAll('[data-kind="function"]');
    for (const row of functionRows) {
      const wrapper = row.parentElement;
      expect(wrapper?.getAttribute('data-conflict')).toBeNull();
    }
  });
});
