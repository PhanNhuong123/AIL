import { describe, it, expect, beforeEach, vi } from 'vitest';
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
