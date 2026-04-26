import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import NodeViewConflictSection from './NodeViewConflictSection.svelte';
import type { SheafConflictEntry } from '$lib/types';

const makeConflict = (overlapIndex: number, nodeA: string, nodeB: string): SheafConflictEntry => ({
  overlapIndex,
  nodeA,
  nodeB,
  conflictingA: [`constraint-from-${nodeA}`],
  conflictingB: [`constraint-from-${nodeB}`],
});

describe('NodeViewConflictSection.svelte', () => {
  it('NV1: renders nothing when conflicts is empty', async () => {
    const { container } = render(NodeViewConflictSection, {
      props: { conflicts: [], currentNodeId: 'step-a', onJump: () => {} },
    });
    await tick();
    expect(container.querySelector('[data-testid="node-view-conflict-section"]')).toBeNull();
  });

  it('NV2: renders peer entries with both sides constraints visible', async () => {
    const conflict = makeConflict(0, 'step-a', 'step-b');
    const { container } = render(NodeViewConflictSection, {
      props: {
        conflicts: [conflict],
        currentNodeId: 'step-a',
        onJump: () => {},
      },
    });
    await tick();

    const section = container.querySelector('[data-testid="node-view-conflict-section"]');
    expect(section).not.toBeNull();

    const card = container.querySelector('[data-testid="conflict-card-0"]');
    expect(card).not.toBeNull();

    // Both sides' constraints visible
    expect(card?.textContent).toContain('constraint-from-step-a');
    expect(card?.textContent).toContain('constraint-from-step-b');
  });

  it('NV3: jump button click invokes onJump callback with peer id', async () => {
    const onJump = vi.fn();
    const conflict = makeConflict(0, 'step-a', 'step-b');
    const { container } = render(NodeViewConflictSection, {
      props: { conflicts: [conflict], currentNodeId: 'step-a', onJump },
    });
    await tick();

    // When currentNodeId === nodeA, peer is nodeB
    const jumpBtn = container.querySelector('[data-testid="conflict-jump-step-b"]') as HTMLButtonElement;
    expect(jumpBtn).not.toBeNull();
    fireEvent.click(jumpBtn);
    await tick();

    expect(onJump).toHaveBeenCalledOnce();
    expect(onJump).toHaveBeenCalledWith('step-b');
  });

  it('NV4: peer side determined correctly when currentNodeId === nodeB (swaps sides)', async () => {
    const conflict = makeConflict(0, 'step-a', 'step-b');
    const onJump = vi.fn();
    const { container } = render(NodeViewConflictSection, {
      props: {
        conflicts: [conflict],
        currentNodeId: 'step-b', // currentNodeId is nodeB this time
        onJump,
      },
    });
    await tick();

    // peer should now be nodeA (step-a), my constraints are conflictingB
    const jumpBtn = container.querySelector('[data-testid="conflict-jump-step-a"]') as HTMLButtonElement;
    expect(jumpBtn).not.toBeNull();

    const card = container.querySelector('[data-testid="conflict-card-0"]');
    // "This step:" section should show conflictingB constraints
    const sides = card?.querySelectorAll('.conflict-side');
    expect(sides).not.toBeNull();
    // First side = "This step:" — must contain the B constraint
    expect(sides?.[0]?.textContent).toContain('constraint-from-step-b');
    // Second side = "Conflicts with:" — must contain the A constraint
    expect(sides?.[1]?.textContent).toContain('constraint-from-step-a');

    fireEvent.click(jumpBtn);
    await tick();
    expect(onJump).toHaveBeenCalledWith('step-a');
  });
});
