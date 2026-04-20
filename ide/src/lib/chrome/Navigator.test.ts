import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, selection, path, paletteVisible } from '$lib/stores';
import { tweaksPanelOpen } from '$lib/stores';
import { expanded, filterTerm } from './navigator-state';
import { walletFixture, walletFixtureWithFail } from './fixtures';
import Navigator from './Navigator.svelte';

beforeEach(() => {
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  path.set([]);
  paletteVisible.set(false);
  tweaksPanelOpen.set(false);
  expanded.set(new Set(['project:root']));
  filterTerm.set('');
});

describe('Navigator.svelte', () => {
  it('renders project, modules, functions when graph is loaded', async () => {
    const { container } = render(Navigator);
    graph.set(walletFixture());
    await tick();

    const rows = container.querySelectorAll('.nav-row');
    // project + 2 modules = 3 visible rows (modules not expanded yet)
    expect(rows.length).toBeGreaterThanOrEqual(3);

    const text = container.textContent ?? '';
    expect(text).toContain('wallet_service');
    expect(text).toContain('wallet');
    expect(text).toContain('auth');
  });

  it('filter keeps ancestors visible (16.3-A)', async () => {
    const { container } = render(Navigator);
    graph.set(walletFixture());
    await tick();

    filterTerm.set('credit');
    await tick();

    const text = container.textContent ?? '';
    // step name should be visible
    expect(text).toContain('credit_to_payee');
    // ancestor module should also be visible
    expect(text).toContain('wallet');
    // sibling module 'auth' should NOT appear
    expect(text).not.toContain('auth');
  });

  it('row click updates selection and path stores', async () => {
    const { container } = render(Navigator);
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
    const { container } = render(Navigator);
    graph.set(walletFixture());
    await tick();

    // wallet module has status warn → dot-warn
    const warnDots = container.querySelectorAll('.dot-warn');
    expect(warnDots.length).toBeGreaterThan(0);

    // auth module has status ok → dot-ok
    const okDots = container.querySelectorAll('.dot-ok');
    expect(okDots.length).toBeGreaterThan(0);
  });

  it('parent status updates reactively when graph is patched (16.3-B)', async () => {
    const { container } = render(Navigator);
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
    const { container } = render(Navigator);
    graph.set(walletFixture());
    await tick();

    const rows = container.querySelectorAll('.nav-row');
    fireEvent.click(rows[0]); // project row
    await tick();

    const p = get(path);
    expect(p.length).toBe(1);
    expect(p[0]).toBe('project:root');
  });

  it('palette section is hidden by default', async () => {
    const { container } = render(Navigator);
    graph.set(walletFixture());
    await tick();

    const text = container.textContent ?? '';
    expect(text).not.toContain('PALETTE');
    expect(container.querySelectorAll('.chip').length).toBe(0);
  });

  it('palette section visible when paletteVisible is true', async () => {
    const { container } = render(Navigator);
    graph.set(walletFixture());
    await tick();

    paletteVisible.set(true);
    await tick();

    const text = container.textContent ?? '';
    expect(text).toContain('PALETTE');
    const chips = container.querySelectorAll('.chip');
    expect(chips.length).toBe(3);
  });

  it('palette chip dragstart dispatches createdrag event', async () => {
    let firedDetail: { kind: string } | undefined;

    // `createEventDispatcher` in Svelte 5 dispatches non-bubbling events via
    // $$events, not the DOM — so a listener on a parent DOM node never fires.
    // The `events` mount option injects handlers into $$events so
    // createEventDispatcher finds them. The `as any` cast is required because
    // the @testing-library/svelte SvelteComponentOptions union collapses to
    // Props<Navigator> (Record<string, never>) for prop-less Svelte 5 components,
    // making the `events` key typed as `never` even though it is valid at runtime.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const { container } = render(Navigator, {
      events: { createdrag: (e: CustomEvent<{ kind: string }>) => { firedDetail = e.detail; } },
    } as any);
    paletteVisible.set(true);
    await tick();

    const chip = container.querySelector('.chip');
    expect(chip).not.toBeNull();

    fireEvent.dragStart(chip!);
    await tick();

    expect(firedDetail).toBeDefined();
    expect(firedDetail!.kind).toBe('sequence');
  });

  it('expand/collapse toggles children visibility', async () => {
    const { container } = render(Navigator);
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
});
