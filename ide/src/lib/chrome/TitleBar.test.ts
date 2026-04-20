import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, path, tweaksPanelOpen } from '$lib/stores';
import { walletFixture, walletFixtureWithFail } from './fixtures';
import TitleBar from './TitleBar.svelte';

beforeEach(() => {
  graph.set(null);
  path.set([]);
  tweaksPanelOpen.set(false);
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

  it('root element has region-titlebar class and data-tauri-drag-region', () => {
    const { container } = render(TitleBar);
    const header = container.querySelector('header');
    expect(header?.classList.contains('region-titlebar')).toBe(true);
    expect(header?.hasAttribute('data-tauri-drag-region')).toBe(true);
  });
});
