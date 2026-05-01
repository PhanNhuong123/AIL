import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, path, tweaksPanelOpen, activeLens, quickCreateModalOpen } from '$lib/stores';
import { walletFixture, walletFixtureWithFail } from './fixtures';
import TitleBar from './TitleBar.svelte';

beforeEach(() => {
  graph.set(null);
  path.set([]);
  tweaksPanelOpen.set(false);
  activeLens.set('verify');
  quickCreateModalOpen.set(false);
});

describe('TitleBar.svelte', () => {
  it('renders brand name "AIL" and v2 badge', () => {
    const { container } = render(TitleBar);
    const text = container.textContent ?? '';
    expect(text).toContain('AIL');
    expect(text).toContain('v2');
  });

  it('status pills reflect countPills($graph) and update when graph changes', async () => {
    const { container } = render(TitleBar);

    // Load wallet fixture — has verified=4, issues=2
    graph.set(walletFixture());
    await tick();

    const pillsEl = container.querySelector('.pills');
    expect(pillsEl).not.toBeNull();
    const pillText = pillsEl?.textContent ?? '';
    expect(pillText).toContain('4');  // verified
    expect(pillText).toContain('2');  // issues

    // Swap to fail fixture — verified=3, issues=3
    graph.set(walletFixtureWithFail());
    await tick();

    const updatedText = container.querySelector('.pills')?.textContent ?? '';
    expect(updatedText).toContain('3');
  });

  it('shows no pills when graph is null', () => {
    const { container } = render(TitleBar);
    // graph is null by default
    expect(container.querySelector('.pills')).toBeNull();
  });

  it('renders breadcrumbs from path store', async () => {
    const { container } = render(TitleBar);

    graph.set(walletFixture());
    path.set(['project:root', 'module:m_wallet']);
    await tick();

    const text = container.querySelector('.breadcrumbs')?.textContent ?? '';
    expect(text).toContain('wallet_service');
    expect(text).toContain('wallet');
  });

  it('crumb click truncates path to clicked index', async () => {
    const { container } = render(TitleBar);

    graph.set(walletFixture());
    path.set(['project:root', 'module:m_wallet', 'function:fn_transfer']);
    await tick();

    const crumbs = container.querySelectorAll('.crumb');
    expect(crumbs.length).toBe(3);

    // Click the first crumb (index 0 → wallet_service)
    fireEvent.click(crumbs[0]);
    await tick();

    const p = get(path);
    expect(p).toEqual(['project:root']);
  });

  it('Tweaks button toggles tweaksPanelOpen store', async () => {
    const { container } = render(TitleBar);

    const tweaksBtn = container.querySelector('[aria-label="Tweaks"]');
    expect(tweaksBtn).not.toBeNull();

    expect(get(tweaksPanelOpen)).toBe(false);

    fireEvent.click(tweaksBtn!);
    await tick();
    expect(get(tweaksPanelOpen)).toBe(true);

    fireEvent.click(tweaksBtn!);
    await tick();
    expect(get(tweaksPanelOpen)).toBe(false);
  });

  // Closes acceptance review MINOR-2 (2026-05-01): the gear button must
  // expose `data-testid="tweaks-toggle-btn"` so the outside-click skip logic
  // in TweaksPanel.svelte can identify gear clicks and avoid the
  // close-then-reopen race when the gear is clicked while the panel is open.
  it('Tweaks gear button exposes data-testid="tweaks-toggle-btn"', () => {
    const { container } = render(TitleBar);
    const gear = container.querySelector('[data-testid="tweaks-toggle-btn"]');
    expect(gear).not.toBeNull();
    expect(gear?.getAttribute('aria-label')).toBe('Tweaks');
  });

  it('root element has region-titlebar class and data-tauri-drag-region', () => {
    const { container } = render(TitleBar);
    const header = container.querySelector('header');
    expect(header?.classList.contains('region-titlebar')).toBe(true);
    expect(header?.hasAttribute('data-tauri-drag-region')).toBe(true);
  });

  // ── Lens pill group ──────────────────────────────────────────────────────
  it('renders the lens pill group with all 5 lenses', () => {
    const { container } = render(TitleBar);
    const group = container.querySelector('[data-testid="lens-group"]');
    expect(group).not.toBeNull();
    expect(group?.getAttribute('role')).toBe('group');
    const pills = group?.querySelectorAll('button[data-testid^="lens-"]') ?? [];
    expect(pills.length).toBe(5);
    const names = Array.from(pills).map((b) => b.getAttribute('data-testid'));
    expect(names).toEqual(['lens-structure', 'lens-rules', 'lens-verify', 'lens-data', 'lens-tests']);
  });

  it('defaults to verify lens with aria-pressed="true"', () => {
    const { container } = render(TitleBar);
    expect(container.querySelector('[data-testid="lens-verify"]')?.getAttribute('aria-pressed')).toBe('true');
    for (const name of ['structure', 'rules', 'data', 'tests']) {
      expect(container.querySelector(`[data-testid="lens-${name}"]`)?.getAttribute('aria-pressed')).toBe('false');
    }
  });

  it('clicking a lens sets activeLens and updates aria-pressed', async () => {
    const { container } = render(TitleBar);
    fireEvent.click(container.querySelector('[data-testid="lens-rules"]')!);
    await tick();
    expect(get(activeLens)).toBe('rules');
    expect(container.querySelector('[data-testid="lens-rules"]')?.getAttribute('aria-pressed')).toBe('true');
    expect(container.querySelector('[data-testid="lens-verify"]')?.getAttribute('aria-pressed')).toBe('false');
  });

  it('lens selection is mutually exclusive', async () => {
    const { container } = render(TitleBar);
    for (const name of ['structure', 'rules', 'verify', 'data', 'tests']) {
      fireEvent.click(container.querySelector(`[data-testid="lens-${name}"]`)!);
      await tick();
      const pressed = container.querySelectorAll('[aria-pressed="true"]');
      // Filter to only lens pills (exclude + New button which may have aria-pressed)
      const lensPressed = Array.from(pressed).filter((el) =>
        el.getAttribute('data-testid')?.startsWith('lens-')
      );
      expect(lensPressed.length).toBe(1);
      expect(lensPressed[0].getAttribute('data-testid')).toBe(`lens-${name}`);
    }
  });

  it('clicking the active lens is idempotent (no toggle off)', async () => {
    const { container } = render(TitleBar);
    const verifyBtn = container.querySelector('[data-testid="lens-verify"]')!;
    fireEvent.click(verifyBtn);
    await tick();
    expect(get(activeLens)).toBe('verify');
    fireEvent.click(verifyBtn);
    await tick();
    expect(get(activeLens)).toBe('verify');
    expect(verifyBtn.getAttribute('aria-pressed')).toBe('true');
  });

  // ── Open project ──────────────────────────────────────────────────────────
  it('renders Open button with correct aria-label', () => {
    const { container } = render(TitleBar);
    const btn = container.querySelector('[data-testid="open-btn"]');
    expect(btn).not.toBeNull();
    expect(btn?.getAttribute('aria-label')).toBe('Open project');
  });

  it('Open button is clickable and shows hover affordance via title', () => {
    // Svelte 5 createEventDispatcher only delivers events when the parent
    // template registers `on:event=`; isolated mounts cannot observe the
    // dispatch directly (matches WelcomeModal/QuickCreateModal pattern).
    // The dispatch -> +page.svelte -> handleWelcomeOpen wiring is verified
    // in routes/layout.test.ts.
    const { container } = render(TitleBar);
    const btn = container.querySelector('[data-testid="open-btn"]') as HTMLButtonElement;
    expect(btn).not.toBeNull();
    expect(btn.title).toBe('Open an existing .ail project');
    expect(btn.disabled).toBe(false);
  });

  // ── Quick Create ──────────────────────────────────────────────────────────
  it('renders + New button with correct aria-label', () => {
    const { container } = render(TitleBar);
    const btn = container.querySelector('[data-testid="new-btn"]');
    expect(btn).not.toBeNull();
    expect(btn?.getAttribute('aria-label')).toBe('New (Quick Create)');
  });

  it('clicking + New opens Quick Create modal', async () => {
    const { container } = render(TitleBar);
    fireEvent.click(container.querySelector('[data-testid="new-btn"]')!);
    await tick();
    expect(get(quickCreateModalOpen)).toBe(true);
  });

  it('Cmd+K opens Quick Create modal', async () => {
    render(TitleBar);
    fireEvent.keyDown(window, { key: 'k', metaKey: true });
    await tick();
    expect(get(quickCreateModalOpen)).toBe(true);

    quickCreateModalOpen.set(false);

    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    await tick();
    expect(get(quickCreateModalOpen)).toBe(true);

    quickCreateModalOpen.set(false);

    fireEvent.keyDown(window, { key: 'K', metaKey: true });
    await tick();
    expect(get(quickCreateModalOpen)).toBe(true);
  });

  it('plain K (no modifier) does not open Quick Create', async () => {
    render(TitleBar);
    fireEvent.keyDown(window, { key: 'k' });
    await tick();
    expect(get(quickCreateModalOpen)).toBe(false);
  });

  it('+ New and Cmd+K produce the same store state', async () => {
    const { container } = render(TitleBar);

    fireEvent.click(container.querySelector('[data-testid="new-btn"]')!);
    await tick();
    const afterClick = get(quickCreateModalOpen);

    quickCreateModalOpen.set(false);

    fireEvent.keyDown(window, { key: 'k', metaKey: true });
    await tick();
    const afterKey = get(quickCreateModalOpen);

    expect(afterClick).toBe(true);
    expect(afterKey).toBe(true);
  });

  it('Tweaks button still toggles tweaksPanelOpen (regression)', async () => {
    const { container } = render(TitleBar);
    const tweaksBtn = container.querySelector('[aria-label="Tweaks"]');
    expect(get(tweaksPanelOpen)).toBe(false);
    fireEvent.click(tweaksBtn!);
    await tick();
    expect(get(tweaksPanelOpen)).toBe(true);
  });

  it('renders generic traffic lights in non-mac environment (platform detection)', () => {
    const { container } = render(TitleBar);
    expect(container.querySelector('[data-testid="traffic-lights-generic"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="traffic-lights-mac"]')).toBeNull();
  });
});
