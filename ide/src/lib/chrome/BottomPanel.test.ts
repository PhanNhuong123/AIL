import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import BottomPanel from './BottomPanel.svelte';
import {
  bottomActiveTab,
  bottomCollapsed,
  resetBottomPanelState,
  CONSOLE_SEED,
  ASSISTANT_SEED,
  TIP_TEXT,
} from './bottom-panel-state';

beforeEach(resetBottomPanelState);

describe('BottomPanel.svelte', () => {
  it('test_bottom_chat_console_switch', async () => {
    const { container } = render(BottomPanel);
    // chat is default; assert chat scroll present, console absent
    expect(container.querySelector('[data-testid="bp-chat-scroll"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="bp-console"]')).toBeNull();

    await fireEvent.click(container.querySelector('[data-testid="bp-tab-console"]')!);
    await tick();

    expect(get(bottomActiveTab)).toBe('console');
    expect(container.querySelector('[data-testid="bp-console"]')).not.toBeNull();
    expect(container.querySelectorAll('.bp-console-line').length).toBe(3);
  });

  it('test_bottom_collapse_toggle', async () => {
    const { container } = render(BottomPanel);
    expect(container.querySelector('.bp-body')).not.toBeNull();

    await fireEvent.click(container.querySelector('[data-testid="bp-collapse-btn"]')!);
    await tick();

    expect(container.querySelector('.bp-body')).toBeNull();
    expect(get(bottomCollapsed)).toBe(true);

    await fireEvent.click(container.querySelector('[data-testid="bp-collapse-btn"]')!);
    await tick();

    expect(container.querySelector('.bp-body')).not.toBeNull();
    expect(get(bottomCollapsed)).toBe(false);
  });

  it('renders assistant seed and tip text', () => {
    const { container } = render(BottomPanel);
    const scroll = container.querySelector('[data-testid="bp-chat-scroll"]')!;
    expect(scroll.textContent).toContain(ASSISTANT_SEED.text);
    expect(container.querySelector('.bp-tip')!.textContent).toContain(TIP_TEXT);
  });

  it('renders preview card with 3 action buttons', () => {
    const { container } = render(BottomPanel);
    expect(container.querySelector('[data-testid="bp-preview-card"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="bp-preview-confirm"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="bp-preview-adjust"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="bp-preview-discard"]')).not.toBeNull();
  });

  it('console tab seeds 3 lines with correct levels', async () => {
    bottomActiveTab.set('console');
    const { container } = render(BottomPanel);
    await tick();

    const lines = container.querySelectorAll('.bp-console-line');
    expect(lines.length).toBe(3);
    expect(lines[0].classList.contains('ok')).toBe(true);
    expect(lines[1].classList.contains('warn')).toBe(true);
    expect(lines[2].classList.contains('fail')).toBe(true);
    expect(lines[0].textContent).toContain(CONSOLE_SEED[0].text);
  });

  it('collapse does not alter active tab', async () => {
    bottomActiveTab.set('console');
    const { container } = render(BottomPanel);
    await tick();

    await fireEvent.click(container.querySelector('[data-testid="bp-collapse-btn"]')!);
    await tick();
    await fireEvent.click(container.querySelector('[data-testid="bp-collapse-btn"]')!);
    await tick();

    expect(get(bottomActiveTab)).toBe('console');
  });
});
