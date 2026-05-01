import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import WelcomeModal from './WelcomeModal.svelte';
import { welcomeModalOpen } from '$lib/stores';

beforeEach(() => welcomeModalOpen.set(false));

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
});
