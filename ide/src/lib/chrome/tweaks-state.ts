/**
 * tweaks-state.ts — single owner of localStorage persistence for the
 * Tweaks-panel-controlled appearance settings (theme, density, accent).
 *
 * Mirrors the pattern in `lib/chat/sidebar-state.ts` (init on mount, write-
 * through subscribers, idempotent). Keeps invariant 15.12-B intact: the
 * frontend's allowed localStorage writers are the sidebar key, the welcome-
 * dismissed key, and now `ail3_tweaks_v1`.
 *
 * Acceptance review 2026-05-01 found:
 *   - theme toggle left both `dark` and `light` classes on `<html>` because
 *     it only toggled `light` and never adjusted `dark`. Cosmetic since
 *     `:root.light` wins by specificity, but bad hygiene and a debug
 *     hazard.
 *   - theme / density / accent did not persist across reloads.
 *
 * This module fixes all three.
 */

import { get } from 'svelte/store';
import { theme, density, accent } from '$lib/stores';
import type { Theme, Density } from '$lib/stores';

const STORAGE_KEY = 'ail3_tweaks_v1';

interface PersistedTweaks {
  theme?: Theme;
  density?: Density;
  accent?: string;
}

let initialized = false;
let pendingUnsubs = [] as Array<() => void>;

export function initTweaksState(): void {
  if (initialized) return;
  initialized = true;

  // 1) Read persisted state, with SSR + corruption guards.
  if (typeof localStorage !== 'undefined') {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as PersistedTweaks;
        if (parsed.theme === 'dark' || parsed.theme === 'light') theme.set(parsed.theme);
        if (parsed.density === 'compact' || parsed.density === 'cozy' || parsed.density === 'comfortable') density.set(parsed.density);
        if (typeof parsed.accent === 'string' && /^#[0-9a-fA-F]{3,8}$/.test(parsed.accent)) accent.set(parsed.accent);
      }
    } catch {
      // Corrupt JSON or quota error — silently fall back to defaults.
    }
  }

  // 2) Apply the stored values to the DOM so the page reflects them on first
  //    paint after init. Both `dark` and `light` classes are managed
  //    explicitly so they never coexist (E2 fix).
  applyThemeClass(get(theme));
  applyAccent(get(accent));

  // 3) Write-through persistence + DOM application on any change.
  const persist = () => {
    if (typeof localStorage === 'undefined') return;
    try {
      const payload: PersistedTweaks = {
        theme: get(theme),
        density: get(density),
        accent: get(accent),
      };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
    } catch {
      // Quota exceeded — drop silently.
    }
  };

  pendingUnsubs.push(theme.subscribe((t) => { applyThemeClass(t); persist(); }));
  pendingUnsubs.push(density.subscribe(() => persist()));
  pendingUnsubs.push(accent.subscribe((a) => { applyAccent(a); persist(); }));
}

function applyThemeClass(t: Theme): void {
  if (typeof document === 'undefined') return;
  document.documentElement.classList.toggle('light', t === 'light');
  document.documentElement.classList.toggle('dark', t === 'dark');
}

function applyAccent(value: string): void {
  if (typeof document === 'undefined') return;
  if (!value) return;
  document.documentElement.style.setProperty('--accent', value);
}

/**
 * Test seam — restore initial state and tear down subscribers between
 * vitest runs so cross-test bleed doesn't leak DOM classes or store values.
 */
export function resetTweaksState(): void {
  for (const fn of pendingUnsubs) fn();
  pendingUnsubs = [];
  initialized = false;
  if (typeof localStorage !== 'undefined') {
    try { localStorage.removeItem(STORAGE_KEY); } catch { /* ignore */ }
  }
  if (typeof document !== 'undefined') {
    document.documentElement.classList.remove('light');
    document.documentElement.classList.add('dark');
    document.documentElement.style.removeProperty('--accent');
  }
  theme.set('dark');
  density.set('comfortable');
  accent.set('#2997ff');
}
