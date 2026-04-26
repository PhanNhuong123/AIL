import { describe, it, expect, beforeEach, afterEach, vi, test } from 'vitest';
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

  // -------------------------------------------------------------------------
  // Invariant 15.11-B + 15.12-E: ESC listener uses document, not window
  // -------------------------------------------------------------------------

  it('does not register window keydown but registers document keydown', () => {
    const winSpy = vi.spyOn(window, 'addEventListener');
    const docSpy = vi.spyOn(document, 'addEventListener');
    tweaksPanelOpen.set(true);
    render(TweaksPanel);
    // Existing 15.11-B assertion: no window keydown
    const windowKeydownCalls = winSpy.mock.calls.filter((c) => c[0] === 'keydown');
    expect(windowKeydownCalls).toHaveLength(0);
    // New 15.12-E assertion: exactly one document keydown registered
    const docKeydownCalls = docSpy.mock.calls.filter((c) => c[0] === 'keydown');
    expect(docKeydownCalls).toHaveLength(1);
    winSpy.mockRestore();
    docSpy.mockRestore();
  });
});

// -------------------------------------------------------------------------
// 15.12-E: outside-click + floating card tests
// -------------------------------------------------------------------------

test('outside-click on document closes the panel', async () => {
  tweaksPanelOpen.set(true);
  render(TweaksPanel);
  await tick();
  // Dispatch a mousedown event outside the panel
  document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
  await tick();
  expect(get(tweaksPanelOpen)).toBe(false);
});

test('panel stays open immediately after opening (no same-tick close)', async () => {
  tweaksPanelOpen.set(true);
  render(TweaksPanel);
  // No mousedown dispatched; panel must still be open after a tick
  await tick();
  expect(get(tweaksPanelOpen)).toBe(true);
});

test('outside-click skips when target is the TitleBar gear button (clean toggle)', async () => {
  tweaksPanelOpen.set(true);
  render(TweaksPanel);
  await tick();
  // Synthesize a gear button outside the panel
  const fakeGear = document.createElement('button');
  fakeGear.setAttribute('data-testid', 'tweaks-toggle-btn');
  document.body.appendChild(fakeGear);
  fakeGear.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
  await tick();
  // Panel must still be open — TitleBar's click handler will toggle it
  expect(get(tweaksPanelOpen)).toBe(true);
  document.body.removeChild(fakeGear);
});
