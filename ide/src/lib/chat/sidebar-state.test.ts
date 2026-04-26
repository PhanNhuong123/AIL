import { describe, test, expect, beforeEach, afterEach } from 'vitest';
import { get } from 'svelte/store';
import {
  sidebarCollapsed,
  sidebarActiveTab,
  sidebarSlots,
  initSidebarState,
  registerSidebarSlot,
  resetSidebarState,
} from './sidebar-state';

const STORAGE_KEY = 'ail3_sidebar_v1';

beforeEach(() => {
  resetSidebarState();
  if (typeof localStorage !== 'undefined') localStorage.clear();
  if (typeof document !== 'undefined') document.documentElement.classList.remove('sb-collapsed');
});

afterEach(() => {
  resetSidebarState();
  if (typeof localStorage !== 'undefined') localStorage.clear();
  if (typeof document !== 'undefined') document.documentElement.classList.remove('sb-collapsed');
});

// ---------------------------------------------------------------------------
// init
// ---------------------------------------------------------------------------

describe('sidebar-state — init', () => {
  test('init reads collapsed from localStorage', () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ collapsed: true, activeTab: 'chat' }));
    initSidebarState();
    expect(get(sidebarCollapsed)).toBe(true);
  });

  test('init reads activeTab from localStorage', () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ collapsed: false, activeTab: 'inspector' }));
    initSidebarState();
    expect(get(sidebarActiveTab)).toBe('inspector');
  });

  test('init falls back to defaults on corrupt localStorage', () => {
    localStorage.setItem(STORAGE_KEY, '{not valid json');
    initSidebarState();
    expect(get(sidebarCollapsed)).toBe(false);
    expect(get(sidebarActiveTab)).toBe('chat');
  });

  test('init does not throw when localStorage is unavailable', () => {
    const original = (globalThis as Record<string, unknown>).localStorage;
    // Simulate an environment without localStorage by temporarily redefining
    // the property to undefined (the SSR guard in initSidebarState checks
    // `typeof localStorage !== 'undefined'`).
    Object.defineProperty(globalThis, 'localStorage', {
      value: undefined,
      configurable: true,
      writable: true,
    });
    expect(() => initSidebarState()).not.toThrow();
    // Restore
    Object.defineProperty(globalThis, 'localStorage', {
      value: original,
      configurable: true,
      writable: true,
    });
  });
});

// ---------------------------------------------------------------------------
// persistence
// ---------------------------------------------------------------------------

describe('sidebar-state — persistence', () => {
  test('toggling sidebarCollapsed writes to localStorage', () => {
    initSidebarState();
    sidebarCollapsed.set(true);
    const raw = localStorage.getItem(STORAGE_KEY);
    expect(raw).not.toBeNull();
    expect(JSON.parse(raw!).collapsed).toBe(true);
  });

  test('changing sidebarActiveTab writes to localStorage', () => {
    initSidebarState();
    sidebarActiveTab.set('inspector');
    const raw = localStorage.getItem(STORAGE_KEY);
    expect(JSON.parse(raw!).activeTab).toBe('inspector');
  });
});

// ---------------------------------------------------------------------------
// slots
// ---------------------------------------------------------------------------

describe('sidebar-state — slots', () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const stubComponent = (() => null) as any;

  test('register adds slot to sidebarSlots', () => {
    registerSidebarSlot('inspector', { label: 'Inspector', icon: 'wrench', order: 1, component: stubComponent });
    expect(get(sidebarSlots)['inspector']).toBeDefined();
  });

  test('register same id overwrites existing entry (idempotent)', () => {
    registerSidebarSlot('inspector', { label: 'A', icon: 'wrench', order: 1, component: stubComponent });
    registerSidebarSlot('inspector', { label: 'B', icon: 'wrench', order: 1, component: stubComponent });
    expect(get(sidebarSlots)['inspector'].label).toBe('B');
  });

  test('unregister removes slot', () => {
    const off = registerSidebarSlot('inspector', { label: 'I', icon: 'wrench', order: 1, component: stubComponent });
    off();
    expect(get(sidebarSlots)['inspector']).toBeUndefined();
  });

  test('unregister resets activeTab to chat when active slot is removed', () => {
    const off = registerSidebarSlot('inspector', { label: 'I', icon: 'wrench', order: 1, component: stubComponent });
    sidebarActiveTab.set('inspector');
    off();
    expect(get(sidebarActiveTab)).toBe('chat');
  });

  test('first-unregister does not resurrect a re-registered entry', () => {
    const offA = registerSidebarSlot('inspector', { label: 'A', icon: 'wrench', order: 1, component: stubComponent });
    registerSidebarSlot('inspector', { label: 'B', icon: 'wrench', order: 1, component: stubComponent });
    // first off() deletes the slot by id regardless of which register wrote it
    offA();
    expect(get(sidebarSlots)['inspector']).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// html class
// ---------------------------------------------------------------------------

describe('sidebar-state — html class', () => {
  test('sidebarCollapsed toggles html.sb-collapsed class', () => {
    initSidebarState();
    sidebarCollapsed.set(true);
    expect(document.documentElement.classList.contains('sb-collapsed')).toBe(true);
    sidebarCollapsed.set(false);
    expect(document.documentElement.classList.contains('sb-collapsed')).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// reset
// ---------------------------------------------------------------------------

describe('sidebar-state — reset', () => {
  test('resetSidebarState clears all stores', () => {
    initSidebarState();
    sidebarCollapsed.set(true);
    sidebarActiveTab.set('inspector');
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    registerSidebarSlot('inspector', { label: 'I', icon: 'wrench', order: 1, component: (() => null) as any });
    resetSidebarState();
    expect(get(sidebarCollapsed)).toBe(false);
    expect(get(sidebarActiveTab)).toBe('chat');
    expect(get(sidebarSlots)).toEqual({});
  });
});
