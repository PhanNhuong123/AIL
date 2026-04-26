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

  it('cancel button closes modal', async () => {
    quickCreateModalOpen.set(true);
    const { container } = render(QuickCreateModal);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="qc-cancel"]')!);
    await tick();

    expect(get(quickCreateModalOpen)).toBe(false);
  });

  it('create button closes modal (phase-17 stub)', async () => {
    quickCreateModalOpen.set(true);
    const { container } = render(QuickCreateModal);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="qc-create"]')!);
    await tick();

    expect(get(quickCreateModalOpen)).toBe(false);
  });

  it('create-with-ai button closes modal', async () => {
    quickCreateModalOpen.set(true);
    const { container } = render(QuickCreateModal);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="qc-create-ai"]')!);
    await tick();

    expect(get(quickCreateModalOpen)).toBe(false);
  });

  // -------------------------------------------------------------------------
  // Invariant 15.11-B: canonical modal trigger ownership
  // -------------------------------------------------------------------------

  it('invariant 15.11-B: does not register a global keydown listener that opens modals', async () => {
    const spy = vi.spyOn(window, 'addEventListener');
    quickCreateModalOpen.set(true);
    render(QuickCreateModal);
    await tick();

    const keydownCalls = spy.mock.calls.filter((c) => c[0] === 'keydown');
    expect(keydownCalls).toHaveLength(0);
    spy.mockRestore();
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

test('kind is included in create handler payload', async () => {
  const spy = vi.spyOn(console, 'info').mockImplementation(() => {});
  quickCreateModalOpen.set(true);
  const { container } = render(QuickCreateModal);
  await tick();
  const ruleBtn = container.querySelector('[data-testid="qc-kind-btn-rule"]') as HTMLButtonElement;
  await fireEvent.click(ruleBtn);
  const createBtn = container.querySelector('[data-testid="qc-create"]') as HTMLButtonElement;
  if (createBtn) await fireEvent.click(createBtn);
  await tick();
  // Verify the payload included kind: 'rule'
  const calls = spy.mock.calls.find((c) => typeof c[0] === 'string' && c[0].includes('quick-create'));
  expect(calls).toBeDefined();
  expect(calls?.[1]).toMatchObject({ kind: 'rule' });
  spy.mockRestore();
});
