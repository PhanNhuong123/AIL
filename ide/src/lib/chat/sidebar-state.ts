/**
 * sidebar-state.ts — RightSidebar slot store, collapse state, and localStorage
 * persistence for the canonical right-column host.
 *
 * Ownership:
 *   sidebarCollapsed   — collapse toggle; persisted under ail3_sidebar_v1
 *   sidebarActiveTab   — which tab is active ('chat' or a registered slot id)
 *   sidebarSlots       — map of dynamically registered tab slots
 *
 * Write rules:
 *   sidebarCollapsed   — written ONLY by RightSidebar collapse button and
 *                        initSidebarState (hydrate from localStorage)
 *   sidebarActiveTab   — written ONLY by RightSidebar rail buttons,
 *                        initSidebarState (hydrate), registerSidebarSlot
 *                        unregister cleanup, and resetSidebarState
 *   sidebarSlots       — written ONLY by registerSidebarSlot / unregister
 *                        and resetSidebarState
 *
 * INVARIANT 15.12-B: This file is the only frontend file allowed to read or
 * write localStorage. Only { collapsed: boolean, activeTab: string } is stored
 * under key 'ail3_sidebar_v1'. Theme / accent / density / chat / flow / node
 * tab / outline state must NOT be persisted here.
 */

import { get, writable } from 'svelte/store';
import type { Writable } from 'svelte/store';
import type { IconName } from '$lib/icons/icon-types';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SidebarSlotEntry {
  id: string;
  label: string;
  icon: IconName;
  order: number;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  component: any;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STORAGE_KEY = 'ail3_sidebar_v1';

// ---------------------------------------------------------------------------
// Stores
// ---------------------------------------------------------------------------

export const sidebarCollapsed: Writable<boolean> = writable(false);
export const sidebarActiveTab: Writable<string> = writable('chat');
export const sidebarSlots: Writable<Record<string, SidebarSlotEntry>> = writable({});
export const sidebarHydrated: Writable<boolean> = writable(false);

// ---------------------------------------------------------------------------
// Initialization (must be called from onMount — NOT module top-level so that
// Vitest Node-env tests don't blow up at import time when localStorage is
// unavailable)
// ---------------------------------------------------------------------------

let initialized = false;
let pendingUnsubs = [] as Array<() => void>;

/**
 * Hydrate `sidebarCollapsed` and `sidebarActiveTab` from localStorage and
 * subscribe both to write-through persistence + the `html.sb-collapsed` class
 * toggle. Idempotent — safe to call from multiple onMount hooks.
 *
 * Must be called from `onMount` so Vitest's Node-env tests don't fail at
 * import time when `localStorage` is unavailable.
 */
export function initSidebarState(): void {
  if (initialized) return;
  initialized = true;

  // 1) Read persisted state with SSR + corruption guards.
  if (typeof localStorage !== 'undefined') {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw);
        if (typeof parsed?.collapsed === 'boolean') {
          sidebarCollapsed.set(parsed.collapsed);
        }
        if (typeof parsed?.activeTab === 'string') {
          sidebarActiveTab.set(parsed.activeTab);
        }
      }
    } catch {
      // Corrupt JSON or quota error — silently fall back to defaults.
    }
  }

  // 2) Write-through persistence on any change.
  const persist = () => {
    if (typeof localStorage === 'undefined') return;
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          collapsed: get(sidebarCollapsed),
          activeTab: get(sidebarActiveTab),
        }),
      );
    } catch {
      // Quota exceeded — drop silently.
    }
  };
  pendingUnsubs.push(sidebarCollapsed.subscribe(persist));
  pendingUnsubs.push(sidebarActiveTab.subscribe(persist));

  // 3) html.sb-collapsed class toggle.
  pendingUnsubs.push(
    sidebarCollapsed.subscribe((collapsed) => {
      if (typeof document === 'undefined') return;
      document.documentElement.classList.toggle('sb-collapsed', collapsed);
    }),
  );

  sidebarHydrated.set(true);
}

// ---------------------------------------------------------------------------
// Slot registration
// ---------------------------------------------------------------------------

/**
 * Register a sidebar tab slot. Returns an `unregister` cleanup that removes the
 * slot and resets `sidebarActiveTab` to `'chat'` if the unregistered slot was
 * currently active.
 *
 * Idempotent on `id`: re-registering the same id replaces the entry in place.
 */
export function registerSidebarSlot(
  id: string,
  entry: Omit<SidebarSlotEntry, 'id'>,
): () => void {
  sidebarSlots.update((slots) => ({ ...slots, [id]: { id, ...entry } }));
  return () => {
    sidebarSlots.update((slots) => {
      const next = { ...slots };
      delete next[id];
      return next;
    });
    if (get(sidebarActiveTab) === id) {
      sidebarActiveTab.set('chat');
    }
  };
}

// ---------------------------------------------------------------------------
// Reset (test-only helper)
// ---------------------------------------------------------------------------

/**
 * Reset all sidebar stores to defaults. Test-only helper — must be called in
 * beforeEach/afterEach alongside `document.documentElement.classList.remove('sb-collapsed')`.
 */
export function resetSidebarState(): void {
  for (const unsub of pendingUnsubs) unsub();
  pendingUnsubs = [];
  initialized = false;
  sidebarHydrated.set(false);
  sidebarCollapsed.set(false);
  sidebarActiveTab.set('chat');
  sidebarSlots.set({});
}
