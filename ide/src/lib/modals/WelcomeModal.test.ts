import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import WelcomeModal from './WelcomeModal.svelte';
import { welcomeModalOpen, welcomeNotice } from '$lib/stores';
import { setWelcomeDismissed, getWelcomeDismissed } from '$lib/chat/sidebar-state';

beforeEach(() => {
  welcomeModalOpen.set(false);
  welcomeNotice.set('');
  // Welcome-dismissed is now persisted on close — wipe it between tests
  // so each case starts from "fresh user, never dismissed".
  if (typeof localStorage !== 'undefined') {
    try { localStorage.removeItem('ail3_welcome_dismissed_v1'); } catch { /* ignore */ }
  }
  setWelcomeDismissed(false);
});

describe('WelcomeModal.svelte', () => {
  it('test_welcome_shows_3_cards', async () => {
    welcomeModalOpen.set(true);
    const { container } = render(WelcomeModal);
    await tick();

    expect(container.querySelector('[data-testid="welcome-card-start"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="welcome-card-open"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="welcome-card-tutorial"]')).not.toBeNull();

    const start = container.querySelector('[data-testid="welcome-card-start"]')!;
    expect(start.textContent).toContain('Start a new project');
  });

  it('card click does NOT auto-close modal (route closes after success)', async () => {
    // Svelte 5 `createEventDispatcher` only delivers events to a parent that
    // wired them via `on:event=`; isolated mounts cannot observe the
    // dispatch directly. The dispatch -> parent wiring is verified in
    // routes/layout.test.ts where `+page.svelte` is mounted. This unit test
    // pins the modal-side behaviour: clicking a card MUST NOT close the
    // modal (closes review finding N1.b — old stub closed on click).
    welcomeModalOpen.set(true);
    const { container } = render(WelcomeModal);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="welcome-card-start"]')!);
    await tick();
    expect(get(welcomeModalOpen)).toBe(true);

    await fireEvent.click(container.querySelector('[data-testid="welcome-card-open"]')!);
    await fireEvent.click(container.querySelector('[data-testid="welcome-card-tutorial"]')!);
    await tick();
    expect(get(welcomeModalOpen)).toBe(true);
  });

  it('backdrop click closes modal; click inside does not', async () => {
    welcomeModalOpen.set(true);
    const { container } = render(WelcomeModal);
    await tick();

    const backdrop = container.querySelector('[data-testid="welcome-backdrop"]')!;
    // Clicking backdrop directly: target === currentTarget so it closes
    await fireEvent.click(backdrop);
    await tick();

    expect(get(welcomeModalOpen)).toBe(false);

    // Reopen and click inside dialog — stopPropagation prevents close
    welcomeModalOpen.set(true);
    await tick();

    const dialog = container.querySelector('.modal-dialog');
    expect(dialog).not.toBeNull();
    await fireEvent.click(dialog!);
    await tick();
    expect(get(welcomeModalOpen)).toBe(true);
  });

  it('close button closes modal', async () => {
    welcomeModalOpen.set(true);
    const { container } = render(WelcomeModal);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="welcome-close"]')!);
    await tick();

    expect(get(welcomeModalOpen)).toBe(false);
  });

  it('invariant 15.11-B: does not register a global window keydown listener (open path)', async () => {
    const spy = vi.spyOn(window, 'addEventListener');
    welcomeModalOpen.set(true);
    render(WelcomeModal);
    await tick();

    const keydownCalls = spy.mock.calls.filter((c) => c[0] === 'keydown');
    expect(keydownCalls).toHaveLength(0);
    spy.mockRestore();
  });

  it('ESC key closes the modal when open', async () => {
    welcomeModalOpen.set(true);
    render(WelcomeModal);
    await tick();

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await tick();

    expect(get(welcomeModalOpen)).toBe(false);
  });

  it('ESC key is a no-op when the modal is closed', async () => {
    welcomeModalOpen.set(false);
    render(WelcomeModal);
    await tick();

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await tick();

    expect(get(welcomeModalOpen)).toBe(false);
  });

  // Acceptance review 2026-05-01 (Story S1.A3): closing the Welcome modal
  // via the ✕ button must persist the dismissed flag so the modal does not
  // re-open on every reload. Previously only the success-load route in
  // `loadAndCloseWelcome` set the flag; the ✕/ESC paths dropped it.
  it('✕ close persists welcome-dismissed flag', async () => {
    expect(getWelcomeDismissed()).toBe(false);
    welcomeModalOpen.set(true);
    const { container } = render(WelcomeModal);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="welcome-close"]')!);
    await tick();

    expect(get(welcomeModalOpen)).toBe(false);
    expect(getWelcomeDismissed()).toBe(true);
  });

  it('ESC close persists welcome-dismissed flag', async () => {
    expect(getWelcomeDismissed()).toBe(false);
    welcomeModalOpen.set(true);
    render(WelcomeModal);
    await tick();

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await tick();

    expect(get(welcomeModalOpen)).toBe(false);
    expect(getWelcomeDismissed()).toBe(true);
  });

  // -------------------------------------------------------------------------
  // WCAG 2.1 SC 2.4.3 — focus trap
  // -------------------------------------------------------------------------

  it('focus-trap pulls initial focus into the dialog on open', async () => {
    welcomeModalOpen.set(true);
    const { container } = render(WelcomeModal);
    await tick();
    // The trap defers initial focus via queueMicrotask — flush it.
    await Promise.resolve();

    const dialog = container.querySelector('.modal-dialog')!;
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  it('Tab from the last focusable wraps to the first (forward cycle)', async () => {
    welcomeModalOpen.set(true);
    const { container } = render(WelcomeModal);
    await tick();
    await Promise.resolve();

    const dialog = container.querySelector('.modal-dialog') as HTMLElement;
    const focusables = dialog.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [tabindex]:not([tabindex="-1"])',
    );
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    last.focus();
    expect(document.activeElement).toBe(last);

    await fireEvent.keyDown(dialog, { key: 'Tab' });
    expect(document.activeElement).toBe(first);
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  it('Shift+Tab from the first focusable wraps to the last (reverse cycle)', async () => {
    welcomeModalOpen.set(true);
    const { container } = render(WelcomeModal);
    await tick();
    await Promise.resolve();

    const dialog = container.querySelector('.modal-dialog') as HTMLElement;
    const focusables = dialog.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [tabindex]:not([tabindex="-1"])',
    );
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    first.focus();
    expect(document.activeElement).toBe(first);

    await fireEvent.keyDown(dialog, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(last);
  });

  // -------------------------------------------------------------------------
  // MINOR-3 acceptance review (2026-05-01) — inline notice rendering
  // -------------------------------------------------------------------------

  it('does not render the notice element when welcomeNotice is empty', async () => {
    welcomeModalOpen.set(true);
    welcomeNotice.set('');
    const { container } = render(WelcomeModal);
    await tick();
    expect(container.querySelector('[data-testid="welcome-notice"]')).toBeNull();
  });

  it('renders the inline notice with role="status" when welcomeNotice is set', async () => {
    welcomeModalOpen.set(true);
    welcomeNotice.set('Open is unavailable in browser preview.');
    const { container } = render(WelcomeModal);
    await tick();
    const notice = container.querySelector('[data-testid="welcome-notice"]');
    expect(notice).not.toBeNull();
    expect(notice?.getAttribute('role')).toBe('status');
    expect(notice?.getAttribute('aria-live')).toBe('polite');
    expect(notice?.textContent ?? '').toContain('browser preview');
  });

  it('close() clears welcomeNotice so a stale message does not flash on re-open', async () => {
    welcomeModalOpen.set(true);
    welcomeNotice.set('Open is unavailable in browser preview.');
    const { container } = render(WelcomeModal);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="welcome-close"]')!);
    await tick();

    expect(get(welcomeModalOpen)).toBe(false);
    expect(get(welcomeNotice)).toBe('');
  });

  it('Tab when focus has escaped the dialog pulls it back to the first focusable', async () => {
    welcomeModalOpen.set(true);
    const { container } = render(WelcomeModal);
    await tick();
    await Promise.resolve();

    const dialog = container.querySelector('.modal-dialog') as HTMLElement;
    const focusables = dialog.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [tabindex]:not([tabindex="-1"])',
    );
    const first = focusables[0];

    // Simulate an outside element stealing focus.
    const outside = document.createElement('button');
    document.body.appendChild(outside);
    outside.focus();
    expect(document.activeElement).toBe(outside);

    await fireEvent.keyDown(dialog, { key: 'Tab' });
    expect(document.activeElement).toBe(first);
    expect(dialog.contains(document.activeElement)).toBe(true);

    document.body.removeChild(outside);
  });
});
