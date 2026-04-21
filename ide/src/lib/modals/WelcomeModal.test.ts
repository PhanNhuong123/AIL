import { describe, it, expect, beforeEach } from 'vitest';
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

  it('test_welcome_dismiss_on_card_click', async () => {
    welcomeModalOpen.set(true);
    const { container } = render(WelcomeModal);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="welcome-card-start"]')!);
    await tick();

    expect(get(welcomeModalOpen)).toBe(false);
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
});
