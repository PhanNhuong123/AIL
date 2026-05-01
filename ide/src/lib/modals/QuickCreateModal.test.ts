import { describe, it, expect, beforeEach, vi, test } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import QuickCreateModal from './QuickCreateModal.svelte';
import { quickCreateModalOpen } from '$lib/stores';

beforeEach(() => quickCreateModalOpen.set(false));

describe('QuickCreateModal.svelte', () => {
  it('renders only when store open', async () => {
    const { container } = render(QuickCreateModal);
    expect(container.querySelector('[data-testid="qc-backdrop"]')).toBeNull();

    quickCreateModalOpen.set(true);
    await tick();

    expect(container.querySelector('[data-testid="qc-backdrop"]')).not.toBeNull();
  });

  // Svelte 5 `createEventDispatcher` only delivers events when a parent
  // template registered `on:event=` handlers. Isolated mounts can't observe
  // the dispatch directly, so unit tests pin the modal-side side effects;
  // dispatch -> parent wiring is verified from routes/layout.test.ts.

  it('cancel button closes the modal locally', async () => {
    quickCreateModalOpen.set(true);
    const { container } = render(QuickCreateModal);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="qc-cancel"]')!);
    await tick();

    expect(get(quickCreateModalOpen)).toBe(false);
  });

  it('Create button does NOT auto-close the modal (route closes after scaffoldProject)', async () => {
    quickCreateModalOpen.set(true);
    const { container } = render(QuickCreateModal);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="qc-create"]')!);
    await tick();

    expect(get(quickCreateModalOpen)).toBe(true);
  });

  it('Create with AI button does NOT auto-close the modal (route closes after runAgent)', async () => {
    quickCreateModalOpen.set(true);
    const { container } = render(QuickCreateModal);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="qc-create-ai"]')!);
    await tick();

    expect(get(quickCreateModalOpen)).toBe(true);
  });

  // -------------------------------------------------------------------------
  // Invariant 15.11-B: canonical modal trigger ownership
  // -------------------------------------------------------------------------

  it('invariant 15.11-B: does not register a global window keydown listener that opens modals', async () => {
    const spy = vi.spyOn(window, 'addEventListener');
    quickCreateModalOpen.set(true);
    render(QuickCreateModal);
    await tick();

    const keydownCalls = spy.mock.calls.filter((c) => c[0] === 'keydown');
    expect(keydownCalls).toHaveLength(0);
    spy.mockRestore();
  });

  it('ESC key closes the modal when open', async () => {
    quickCreateModalOpen.set(true);
    render(QuickCreateModal);
    await tick();

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await tick();

    expect(get(quickCreateModalOpen)).toBe(false);
  });

  it('ESC key is a no-op when the modal is closed', async () => {
    quickCreateModalOpen.set(false);
    render(QuickCreateModal);
    await tick();

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await tick();

    expect(get(quickCreateModalOpen)).toBe(false);
  });
});

// -------------------------------------------------------------------------
// 15.12-H: kind seg-control tests
// -------------------------------------------------------------------------

test('kind defaults to module on initial open', () => {
  quickCreateModalOpen.set(true);
  const { container } = render(QuickCreateModal);
  const moduleBtn = container.querySelector('[data-testid="qc-kind-btn-module"]');
  expect(moduleBtn?.getAttribute('aria-pressed')).toBe('true');
});

test('kind resets to module after close', async () => {
  quickCreateModalOpen.set(true);
  const { container } = render(QuickCreateModal);
  await tick();
  const fnBtn = container.querySelector('[data-testid="qc-kind-btn-function"]') as HTMLButtonElement;
  await fireEvent.click(fnBtn);
  // Close via the close (X) button
  const closeBtn = container.querySelector('[data-testid="qc-close"]') as HTMLButtonElement;
  if (closeBtn) await fireEvent.click(closeBtn);
  await tick();
  // Reopen
  quickCreateModalOpen.set(true);
  await tick();
  const moduleBtn = container.querySelector('[data-testid="qc-kind-btn-module"]');
  expect(moduleBtn?.getAttribute('aria-pressed')).toBe('true');
});

// -------------------------------------------------------------------------
// WCAG 2.1 SC 2.4.3 — focus trap
// -------------------------------------------------------------------------

test('focus-trap pulls initial focus into the dialog on open', async () => {
  quickCreateModalOpen.set(true);
  const { container } = render(QuickCreateModal);
  await tick();
  await Promise.resolve();

  const dialog = container.querySelector('.modal-dialog')!;
  expect(dialog.contains(document.activeElement)).toBe(true);
});

test('Tab from the last focusable wraps to the first (forward cycle)', async () => {
  quickCreateModalOpen.set(true);
  const { container } = render(QuickCreateModal);
  await tick();
  await Promise.resolve();

  const dialog = container.querySelector('.modal-dialog') as HTMLElement;
  const focusables = dialog.querySelectorAll<HTMLElement>(
    'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
  );
  const first = focusables[0];
  const last = focusables[focusables.length - 1];
  last.focus();
  expect(document.activeElement).toBe(last);

  await fireEvent.keyDown(dialog, { key: 'Tab' });
  expect(document.activeElement).toBe(first);
  expect(dialog.contains(document.activeElement)).toBe(true);
});

test('Shift+Tab from the first focusable wraps to the last (reverse cycle)', async () => {
  quickCreateModalOpen.set(true);
  const { container } = render(QuickCreateModal);
  await tick();
  await Promise.resolve();

  const dialog = container.querySelector('.modal-dialog') as HTMLElement;
  const focusables = dialog.querySelectorAll<HTMLElement>(
    'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
  );
  const first = focusables[0];
  const last = focusables[focusables.length - 1];
  first.focus();
  expect(document.activeElement).toBe(first);

  await fireEvent.keyDown(dialog, { key: 'Tab', shiftKey: true });
  expect(document.activeElement).toBe(last);
});

test('form state stays bound to inputs across kind selection', async () => {
  // Cannot observe the dispatched payload directly in an isolated mount
  // (Svelte 5 createEventDispatcher quirk — see comment above). Instead,
  // verify the visible form state — which is what the dispatch payload
  // snapshots — survives a kind toggle without being reset.
  quickCreateModalOpen.set(true);
  const { container } = render(QuickCreateModal);
  await tick();

  const ruleBtn = container.querySelector('[data-testid="qc-kind-btn-rule"]') as HTMLButtonElement;
  await fireEvent.click(ruleBtn);
  expect(ruleBtn.getAttribute('aria-pressed')).toBe('true');

  const nameInput = container.querySelector('[data-testid="qc-name"]') as HTMLInputElement;
  await fireEvent.input(nameInput, { target: { value: 'wallet' } });
  expect(nameInput.value).toBe('wallet');

  const descInput = container.querySelector('[data-testid="qc-description"]') as HTMLTextAreaElement;
  await fireEvent.input(descInput, { target: { value: 'Demo wallet' } });
  expect(descInput.value).toBe('Demo wallet');

  // Click Create — modal stays open (route owns the close), and form state
  // remains intact for the route handler's snapshot.
  const createBtn = container.querySelector('[data-testid="qc-create"]') as HTMLButtonElement;
  await fireEvent.click(createBtn);
  await tick();
  expect(nameInput.value).toBe('wallet');
  expect(descInput.value).toBe('Demo wallet');
  expect(ruleBtn.getAttribute('aria-pressed')).toBe('true');
});
