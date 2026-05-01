import { describe, it, expect, beforeEach, afterEach, vi, test } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import TweaksPanel from './TweaksPanel.svelte';
import { tweaksPanelOpen, theme, density } from '$lib/stores';
import {
  sidecarHealth, sidecarChecking, resetSidecarState,
} from '$lib/sidecar/sidecar-state';

beforeEach(() => {
  tweaksPanelOpen.set(false);
  theme.set('dark');
  density.set('comfortable');
  resetSidecarState();
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

// -------------------------------------------------------------------------
// N3 — Sidecar health buttons (closes review finding N3)
// -------------------------------------------------------------------------

describe('TweaksPanel sidecar section', () => {
  it('renders both Check buttons in the Sidecars section', async () => {
    tweaksPanelOpen.set(true);
    const { container } = render(TweaksPanel);
    await tick();

    expect(container.querySelector('[data-testid="tweaks-sidecar-section"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="sidecar-health-core"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="sidecar-health-agent"]')).not.toBeNull();
  });

  it('shows "not checked" status when no payload has been recorded', async () => {
    tweaksPanelOpen.set(true);
    const { container } = render(TweaksPanel);
    await tick();

    const coreStatus = container.querySelector('[data-testid="sidecar-status-core"]')!;
    expect(coreStatus.textContent).toContain('not checked');
    expect(coreStatus.getAttribute('data-state')).toBe('idle');
  });

  // Dispatch -> parent wiring is exercised in routes/layout.test.ts via
  // `+page.svelte`. Here we only assert that clicking the buttons triggers
  // their on:click handler at the DOM level (no exception, button toggles).

  it('clicking Check core does not throw and the panel stays open', async () => {
    tweaksPanelOpen.set(true);
    const { container } = render(TweaksPanel);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="sidecar-health-core"]')!);
    await tick();

    expect(get(tweaksPanelOpen)).toBe(true);
  });

  it('clicking Check agent does not throw and the panel stays open', async () => {
    tweaksPanelOpen.set(true);
    const { container } = render(TweaksPanel);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="sidecar-health-agent"]')!);
    await tick();

    expect(get(tweaksPanelOpen)).toBe(true);
  });

  it('button is disabled and shows "Checking…" while a check is in flight', async () => {
    tweaksPanelOpen.set(true);
    sidecarChecking.set({ core: true, agent: false });
    const { container } = render(TweaksPanel);
    await tick();

    const coreBtn = container.querySelector('[data-testid="sidecar-health-core"]') as HTMLButtonElement;
    expect(coreBtn.disabled).toBe(true);
    expect(coreBtn.textContent?.trim()).toBe('Checking…');

    const agentBtn = container.querySelector('[data-testid="sidecar-health-agent"]') as HTMLButtonElement;
    expect(agentBtn.disabled).toBe(false);
  });

  it('renders ok payload as mode + version with data-state="ok"', async () => {
    tweaksPanelOpen.set(true);
    sidecarHealth.set({
      core: { component: 'ail-core', ok: true, mode: 'dev', version: '0.1.0' },
      agent: null,
    });
    const { container } = render(TweaksPanel);
    await tick();

    const coreStatus = container.querySelector('[data-testid="sidecar-status-core"]')!;
    expect(coreStatus.textContent).toContain('dev');
    expect(coreStatus.textContent).toContain('0.1.0');
    expect(coreStatus.getAttribute('data-state')).toBe('ok');
  });

  it('renders failed payload as error with data-state="fail"', async () => {
    tweaksPanelOpen.set(true);
    sidecarHealth.set({
      core: null,
      agent: { component: 'ail-agent', ok: false, mode: 'dev', error: 'spawn failed' },
    });
    const { container } = render(TweaksPanel);
    await tick();

    const agentStatus = container.querySelector('[data-testid="sidecar-status-agent"]')!;
    expect(agentStatus.textContent).toContain('spawn failed');
    expect(agentStatus.getAttribute('data-state')).toBe('fail');
  });
});
