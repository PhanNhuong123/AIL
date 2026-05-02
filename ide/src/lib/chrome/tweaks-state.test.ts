import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { get } from 'svelte/store';
import { theme, density, accent, activeLens } from '$lib/stores';
import { initTweaksState, resetTweaksState } from './tweaks-state';

beforeEach(() => {
  resetTweaksState();
});

afterEach(() => {
  resetTweaksState();
});

describe('tweaks-state', () => {
  // Bug E2 (acceptance 2026-05-01): toggling theme used to leave both `dark`
  // and `light` classes on `<html>` because TweaksPanel only toggled
  // `light` and never adjusted `dark`. tweaks-state now applies BOTH on
  // each theme change.
  it('theme change replaces the html class — never leaves both dark and light', () => {
    initTweaksState();
    document.documentElement.classList.add('dark');

    theme.set('light');
    expect(document.documentElement.classList.contains('light')).toBe(true);
    expect(document.documentElement.classList.contains('dark')).toBe(false);

    theme.set('dark');
    expect(document.documentElement.classList.contains('light')).toBe(false);
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  // Bug E3: theme / density / accent did not survive reload.
  it('theme persists to localStorage and re-hydrates on next init', () => {
    initTweaksState();
    theme.set('light');
    const raw = localStorage.getItem('ail3_tweaks_v1');
    expect(raw).not.toBeNull();
    const parsed = JSON.parse(raw!);
    expect(parsed.theme).toBe('light');

    // Simulate a fresh page load: tear down subscribers, reset stores to
    // their default, then re-init so the persisted value rehydrates.
    resetTweaksState();
    expect(get(theme)).toBe('dark');
    // Manually re-seed localStorage as if it survived the reload.
    localStorage.setItem('ail3_tweaks_v1', JSON.stringify({ theme: 'light' }));
    initTweaksState();

    expect(get(theme)).toBe('light');
    expect(document.documentElement.classList.contains('light')).toBe(true);
  });

  it('density persists to localStorage', () => {
    initTweaksState();
    density.set('compact');
    const parsed = JSON.parse(localStorage.getItem('ail3_tweaks_v1')!);
    expect(parsed.density).toBe('compact');
  });

  it('accent persists to localStorage and applies the --accent CSS var', () => {
    initTweaksState();
    accent.set('#ff00aa');
    const parsed = JSON.parse(localStorage.getItem('ail3_tweaks_v1')!);
    expect(parsed.accent).toBe('#ff00aa');
    expect(document.documentElement.style.getPropertyValue('--accent').trim()).toBe('#ff00aa');
  });

  it('init is idempotent — second init does not double-subscribe', () => {
    initTweaksState();
    initTweaksState();
    theme.set('light');
    // Without the idempotency guard, two subscribers would persist twice.
    // Persisted payload should still be a single, well-formed JSON object.
    const raw = localStorage.getItem('ail3_tweaks_v1');
    expect(() => JSON.parse(raw!)).not.toThrow();
    expect(JSON.parse(raw!).theme).toBe('light');
  });

  it('rejects malformed accent entries on hydrate', () => {
    localStorage.setItem('ail3_tweaks_v1', JSON.stringify({ accent: 'not-a-color' }));
    initTweaksState();
    // Default accent stays — invalid persisted value is ignored.
    expect(get(accent)).toBe('#2997ff');
  });

  // Acceptance pass 2026-05-02 — story B4 caught that activeLens was the
  // odd-one-out: theme / density / accent persisted, lens did not. Now
  // tweaks-state owns lens persistence too, so a user who picks "rules"
  // doesn't get yanked back to "verify" on every reload.
  it('lens persists to localStorage and re-hydrates on next init', () => {
    initTweaksState();
    activeLens.set('rules');
    const parsed = JSON.parse(localStorage.getItem('ail3_tweaks_v1')!);
    expect(parsed.lens).toBe('rules');

    resetTweaksState();
    expect(get(activeLens)).toBe('verify'); // reset returns to default
    localStorage.setItem('ail3_tweaks_v1', JSON.stringify({ lens: 'rules' }));
    initTweaksState();
    expect(get(activeLens)).toBe('rules');
  });

  it('rejects unknown lens values on hydrate', () => {
    localStorage.setItem('ail3_tweaks_v1', JSON.stringify({ lens: 'galaxy-brain' }));
    initTweaksState();
    expect(get(activeLens)).toBe('verify');
  });
});
