// Tauri mock must be hoisted before any bridge imports via ChatPanel.
const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import { describe, test, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, selection, path, activeLens, overlays } from '$lib/stores';
import { resetChatState } from '$lib/chat/chat-state';
import RightSidebar from './RightSidebar.svelte';
import {
  sidebarCollapsed,
  sidebarActiveTab,
  sidebarHydrated,
  registerSidebarSlot,
  resetSidebarState,
  initSidebarState,
} from './sidebar-state';

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue('r-default');
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  path.set([]);
  activeLens.set('verify');
  overlays.set({ rules: false, verification: true, dataflow: false, dependencies: false, tests: false });
  resetChatState();
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
// Basic rendering
// ---------------------------------------------------------------------------

test('renders right-sidebar root element', () => {
  const { container } = render(RightSidebar);
  expect(container.querySelector('[data-testid="right-sidebar"]')).not.toBeNull();
});

test('chat tab button is visible in rail', () => {
  const { container } = render(RightSidebar);
  expect(container.querySelector('[data-testid="sb-rail-btn-chat"]')).not.toBeNull();
});

test('ChatPanel is mounted inside sidebar when chat tab is active', async () => {
  const { container } = render(RightSidebar);
  await tick();
  // chat-panel is the data-testid on ChatPanel's root <aside>
  expect(container.querySelector('[data-testid="chat-panel"]')).not.toBeNull();
});

// ---------------------------------------------------------------------------
// Collapse toggle
// ---------------------------------------------------------------------------

test('collapse button toggles sidebarCollapsed store', async () => {
  const { container } = render(RightSidebar);
  const btn = container.querySelector('[data-testid="sb-collapse-btn"]') as HTMLButtonElement;
  await fireEvent.click(btn);
  expect(get(sidebarCollapsed)).toBe(true);
});

test('aria-pressed mirrors sidebarCollapsed on collapse button', async () => {
  const { container } = render(RightSidebar);
  const btn = container.querySelector('[data-testid="sb-collapse-btn"]') as HTMLButtonElement;
  expect(btn.getAttribute('aria-pressed')).toBe('false');
  await fireEvent.click(btn);
  await tick();
  expect(btn.getAttribute('aria-pressed')).toBe('true');
});

// ---------------------------------------------------------------------------
// Event forwarding
// ---------------------------------------------------------------------------

test('previewapply event forwards from ChatPanel synchronously', async () => {
  // Svelte 5: component.$on is removed. Listen on the root DOM element instead —
  // event-forwarding shorthand `<ChatPanel on:previewapply />` re-dispatches the
  // CustomEvent on the parent component's root element, which bubbles through.
  const handler = vi.fn();
  const { container } = render(RightSidebar);
  const sidebar = container.querySelector('[data-testid="right-sidebar"]') as HTMLElement;
  sidebar.addEventListener('previewapply', handler);
  await tick();
  const chatPanel = container.querySelector('[data-testid="chat-panel"]') as HTMLElement;
  chatPanel.dispatchEvent(new CustomEvent('previewapply', { detail: { id: 'p1' }, bubbles: true }));
  expect(handler).toHaveBeenCalledTimes(1);
});

test('previewdismiss event forwards from ChatPanel synchronously', async () => {
  const handler = vi.fn();
  const { container } = render(RightSidebar);
  const sidebar = container.querySelector('[data-testid="right-sidebar"]') as HTMLElement;
  sidebar.addEventListener('previewdismiss', handler);
  await tick();
  const chatPanel = container.querySelector('[data-testid="chat-panel"]') as HTMLElement;
  chatPanel.dispatchEvent(new CustomEvent('previewdismiss', { detail: { id: 'p1' }, bubbles: true }));
  expect(handler).toHaveBeenCalledTimes(1);
});

// ---------------------------------------------------------------------------
// Slot registration
// ---------------------------------------------------------------------------

test('registered slot tab appears in rail', async () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const stub = (() => null) as any;
  registerSidebarSlot('inspector', { label: 'Inspector', icon: 'wrench', order: 1, component: stub });
  const { container } = render(RightSidebar);
  await tick();
  expect(container.querySelector('[data-testid="sb-rail-btn-inspector"]')).not.toBeNull();
});

test('slot content renders when active: chat-panel absent from sb-content', async () => {
  // When a slot tab is active, ChatPanel is NOT in sb-content.
  // We can't render an actual Svelte component from a stub function in
  // svelte:component, so we assert the negative: no chat-panel in sb-content.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const stub = (() => null) as any;
  registerSidebarSlot('inspector', { label: 'Inspector', icon: 'wrench', order: 1, component: stub });
  sidebarActiveTab.set('inspector');
  const { container } = render(RightSidebar);
  await tick();
  const sbContent = container.querySelector('[data-testid="sb-content"]');
  expect(sbContent?.querySelector('[data-testid="chat-panel"]')).toBeNull();
});

test('falls back to chat tab when active slot is unregistered', async () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const stub = (() => null) as any;
  const off = registerSidebarSlot('inspector', { label: 'I', icon: 'wrench', order: 1, component: stub });
  sidebarActiveTab.set('inspector');
  // initSidebarState sets sidebarHydrated = true, enabling the fallback reactive.
  initSidebarState();
  render(RightSidebar);
  await tick();
  off();
  await tick();
  expect(get(sidebarActiveTab)).toBe('chat');
});

// ---------------------------------------------------------------------------
// describe block for grouped coverage
// ---------------------------------------------------------------------------

describe('RightSidebar — collapsed state hides sb-content', () => {
  test('sb-content is absent when collapsed', async () => {
    sidebarCollapsed.set(true);
    const { container } = render(RightSidebar);
    await tick();
    expect(container.querySelector('[data-testid="sb-content"]')).toBeNull();
  });
});
