import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import TweaksPanel from './TweaksPanel.svelte';
import { tweaksPanelOpen, theme, density } from '$lib/stores';

beforeEach(() => {
  tweaksPanelOpen.set(false);
  theme.set('dark');
  density.set('comfortable');
});

afterEach(() => {
  document.documentElement.classList.remove('light');
  document.documentElement.style.removeProperty('--accent');
});

describe('TweaksPanel.svelte', () => {
  it('test_tweaks_accent_updates_css_var', async () => {
    tweaksPanelOpen.set(true);
    const { container } = render(TweaksPanel);
    await tick();

    const input = container.querySelector('[data-testid="tweaks-accent-input"]') as HTMLInputElement;
    input.value = '#ff00aa';
    await fireEvent.input(input);
    await tick();

    expect(document.documentElement.style.getPropertyValue('--accent').trim()).toBe('#ff00aa');
  });

  it('theme toggle writes theme store and .light class', async () => {
    tweaksPanelOpen.set(true);
    const { container } = render(TweaksPanel);
    await tick();

    expect(get(theme)).toBe('dark');

    await fireEvent.click(container.querySelector('[data-testid="tweaks-theme-toggle"]')!);
    await tick();

    expect(get(theme)).toBe('light');
    expect(document.documentElement.classList.contains('light')).toBe(true);

    await fireEvent.click(container.querySelector('[data-testid="tweaks-theme-toggle"]')!);
    await tick();

    expect(get(theme)).toBe('dark');
    expect(document.documentElement.classList.contains('light')).toBe(false);
  });

  it('density segmented buttons update density store', async () => {
    tweaksPanelOpen.set(true);
    const { container } = render(TweaksPanel);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="tweaks-density-compact"]')!);
    expect(get(density)).toBe('compact');

    await fireEvent.click(container.querySelector('[data-testid="tweaks-density-cozy"]')!);
    expect(get(density)).toBe('cozy');

    await fireEvent.click(container.querySelector('[data-testid="tweaks-density-comfortable"]')!);
    expect(get(density)).toBe('comfortable');
  });

  it('close button closes panel', async () => {
    tweaksPanelOpen.set(true);
    const { container } = render(TweaksPanel);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="tweaks-close"]')!);
    await tick();

    expect(get(tweaksPanelOpen)).toBe(false);
  });

  it('backdrop click closes; click inside panel does not', async () => {
    tweaksPanelOpen.set(true);
    const { container } = render(TweaksPanel);
    await tick();

    const backdrop = container.querySelector('[data-testid="tweaks-backdrop"]')!;
    await fireEvent.click(backdrop);
    await tick();

    expect(get(tweaksPanelOpen)).toBe(false);

    tweaksPanelOpen.set(true);
    await tick();

    const panel = container.querySelector('[data-testid="tweaks-panel"]');
    expect(panel).not.toBeNull();
    await fireEvent.click(panel!);
    await tick();
    expect(get(tweaksPanelOpen)).toBe(true);
  });

  it('invariant 15.11-B: does not register a global keydown listener', async () => {
    const spy = vi.spyOn(window, 'addEventListener');
    tweaksPanelOpen.set(true);
    render(TweaksPanel);
    await tick();

    const keydownCalls = spy.mock.calls.filter((c) => c[0] === 'keydown');
    expect(keydownCalls).toHaveLength(0);
    spy.mockRestore();
  });
});
