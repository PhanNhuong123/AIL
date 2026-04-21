import { describe, it, expect, beforeEach } from 'vitest';
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
});
