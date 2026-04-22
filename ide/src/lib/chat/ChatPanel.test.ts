import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/svelte';
import ChatPanel from './ChatPanel.svelte';

describe('ChatPanel.svelte', () => {
  it('test_chat_panel_has_testid', () => {
    const { container } = render(ChatPanel);
    expect(container.querySelector('[data-testid="chat-panel"]')).not.toBeNull();
  });

  it('test_chat_panel_has_region_chat_class', () => {
    const { container } = render(ChatPanel);
    const aside = container.querySelector('aside');
    expect(aside).not.toBeNull();
    expect(aside?.classList.contains('region-chat')).toBe(true);
  });

  it('test_chat_panel_renders_placeholder', () => {
    const { container } = render(ChatPanel);
    const text = container.textContent ?? '';
    expect(text).toContain('Chat');
  });
});
