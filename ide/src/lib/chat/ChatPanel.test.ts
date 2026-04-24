import { describe, it, expect, beforeEach, vi } from 'vitest';

// Establish the Tauri mock surface BEFORE importing bridge.ts. This pattern
// is new to the chat folder — 16.1 tests are the first to need it.
const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, selection, path, activeLens, overlays } from '$lib/stores';
import {
  chatMode, chatDraft, chatMessages, chatPreviewCards,
  isAgentRunning, currentRunId,
  resetChatState, CHAT_ASSISTANT_SEED, CHAT_PREVIEW_SEED, CHAT_PLACEHOLDERS,
} from './chat-state';
import ChatPanel from './ChatPanel.svelte';

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue('r-default');
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  path.set([]);
  activeLens.set('verify');
  overlays.set({ rules: false, verification: true, dataflow: false, dependencies: false, tests: false });
  resetChatState();
});

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

  it('test_chat_head_renders', () => {
    const { container } = render(ChatPanel);
    expect(container.querySelector('[data-testid="chat-head"]')).not.toBeNull();
  });

  it('test_chat_head_subtext_none', async () => {
    const { container } = render(ChatPanel);
    await tick();
    expect(container.querySelector('[data-testid="chat-context-subtext"]')?.textContent).toBe('No selection');
  });

  it('test_chat_head_subtext_module', async () => {
    selection.set({ kind: 'module', id: 'module:billing' });
    const { container } = render(ChatPanel);
    await tick();
    expect(container.querySelector('[data-testid="chat-context-subtext"]')?.textContent).toBe('Module: billing');
  });

  it('test_chat_head_subtext_step', async () => {
    selection.set({ kind: 'step', id: 'step:s_transfer' });
    const { container } = render(ChatPanel);
    await tick();
    expect(container.querySelector('[data-testid="chat-context-subtext"]')?.textContent).toBe('Step: s_transfer');
  });

  it('test_messages_render_seed', async () => {
    const { container } = render(ChatPanel);
    await tick();
    expect(container.querySelector('[data-testid="chat-message-0"]')?.textContent).toContain(CHAT_ASSISTANT_SEED.text);
  });

  it('test_message_append_on_send', async () => {
    const { container } = render(ChatPanel);
    await tick();
    const input = container.querySelector('[data-testid="chat-input"]') as HTMLInputElement;
    const sendBtn = container.querySelector('[data-testid="chat-send-btn"]') as HTMLButtonElement;
    fireEvent.input(input, { target: { value: 'hello' } });
    await tick();
    fireEvent.click(sendBtn);
    await tick();
    // Phase 16: ChatPanel appends the user message; the assistant reply
    // arrives later through the route-level `onAgentMessage` handler.
    expect(get(chatMessages).length).toBe(2);
    expect(container.querySelector('[data-testid="chat-message-1"]')?.textContent).toContain('hello');
    expect(get(chatDraft)).toBe('');
  });

  it('test_message_append_on_enter_key', async () => {
    const { container } = render(ChatPanel);
    await tick();
    const input = container.querySelector('[data-testid="chat-input"]') as HTMLInputElement;
    fireEvent.input(input, { target: { value: 'hello' } });
    await tick();
    fireEvent.keyDown(input, { key: 'Enter' });
    await tick();
    const msgs = get(chatMessages);
    expect(msgs.length).toBe(2);
    expect(msgs[1].role).toBe('user');
    expect(msgs[1].text).toBe('hello');
    expect(get(chatDraft)).toBe('');
  });

  it('test_empty_send_is_noop', async () => {
    const { container } = render(ChatPanel);
    await tick();
    fireEvent.click(container.querySelector('[data-testid="chat-send-btn"]') as HTMLButtonElement);
    await tick();
    expect(get(chatMessages).length).toBe(1);
    expect(invoke).not.toHaveBeenCalled();
  });

  it('test_preview_card_renders_seed', async () => {
    const { container } = render(ChatPanel);
    await tick();
    const cards = container.querySelectorAll('[data-testid="chat-preview-card"]');
    expect(cards.length).toBe(1);
    expect(cards[0].textContent).toContain(CHAT_PREVIEW_SEED.title);
  });

  it('test_preview_confirm_does_not_mutate_cards_or_invoke_bridge', async () => {
    // 16.1-C: Chat components MUST NOT write graph/selection and MUST NOT
    // remove preview cards directly. Parent (+page.svelte) handles that
    // on `previewapply`. The actual dispatch -> parent wiring is covered
    // in src/routes/page.test.ts.
    const { container } = render(ChatPanel);
    await tick();
    const cardsBefore = get(chatPreviewCards).length;
    const invokeCountBefore = invoke.mock.calls.length;
    fireEvent.click(container.querySelector('[data-testid="chat-preview-confirm"]') as HTMLButtonElement);
    await tick();
    expect(get(chatPreviewCards).length).toBe(cardsBefore);
    expect(invoke.mock.calls.length).toBe(invokeCountBefore);
  });

  it('test_preview_adjust_quotes_summary_into_draft', async () => {
    const { container } = render(ChatPanel);
    await tick();
    fireEvent.click(container.querySelector('[data-testid="chat-preview-adjust"]') as HTMLButtonElement);
    await tick();
    expect(get(chatDraft)).toBe(CHAT_PREVIEW_SEED.summary);
    // Card stays visible (Refine does not remove it; user edits + sends).
    expect(get(chatPreviewCards).length).toBe(1);
  });

  it('test_preview_discard_does_not_mutate_cards_directly', async () => {
    // Discard is delegated via `previewdismiss`; parent removes the card.
    const { container } = render(ChatPanel);
    await tick();
    const cardsBefore = get(chatPreviewCards).length;
    fireEvent.click(container.querySelector('[data-testid="chat-preview-discard"]') as HTMLButtonElement);
    await tick();
    expect(get(chatPreviewCards).length).toBe(cardsBefore);
  });

  it('test_chips_render_default_when_no_selection', async () => {
    const { container } = render(ChatPanel);
    await tick();
    expect(container.querySelector('[data-testid="chat-suggest-chip-0"]')?.textContent).toBe('Explain this project');
  });

  it('test_chips_adapt_to_module_structure', async () => {
    selection.set({ kind: 'module', id: 'module:billing' });
    activeLens.set('structure');
    const { container } = render(ChatPanel);
    await tick();
    const chipTexts = Array.from(container.querySelectorAll('[data-testid^="chat-suggest-chip-"]')).map((el) => el.textContent ?? '');
    expect(chipTexts).toContain('List functions');
  });

  it('test_chips_adapt_on_lens_change_without_reset', async () => {
    selection.set({ kind: 'module', id: 'module:billing' });
    activeLens.set('structure');
    const { container } = render(ChatPanel);
    await tick();
    chatMode.set('ask');
    chatDraft.set('typing');
    await tick();
    activeLens.set('verify');
    await tick();
    const chipTexts = Array.from(container.querySelectorAll('[data-testid^="chat-suggest-chip-"]')).map((el) => el.textContent ?? '');
    expect(chipTexts).toContain('Verify this module');
    expect(get(chatMode)).toBe('ask');
    expect(get(chatDraft)).toBe('typing');
    expect(get(chatMessages).length).toBe(1);
  });

  it('test_chip_click_copies_text_to_draft', async () => {
    const { container } = render(ChatPanel);
    await tick();
    fireEvent.click(container.querySelector('[data-testid="chat-suggest-chip-0"]') as HTMLButtonElement);
    await tick();
    expect(get(chatDraft)).toBe('Explain this project');
    expect(get(chatMessages).length).toBe(1);
  });

  it('test_placeholder_edit_default', async () => {
    const { container } = render(ChatPanel);
    await tick();
    const input = container.querySelector('[data-testid="chat-input"]') as HTMLInputElement;
    expect(input.getAttribute('placeholder')).toBe(CHAT_PLACEHOLDERS.edit);
  });

  it('test_placeholder_ask_after_mode_switch', async () => {
    const { container } = render(ChatPanel);
    await tick();
    fireEvent.click(container.querySelector('[data-testid="chat-mode-btn-ask"]') as HTMLButtonElement);
    await tick();
    const input = container.querySelector('[data-testid="chat-input"]') as HTMLInputElement;
    expect(input.getAttribute('placeholder')).toBe(CHAT_PLACEHOLDERS.ask);
  });

  it('test_placeholder_test_after_mode_switch', async () => {
    const { container } = render(ChatPanel);
    await tick();
    fireEvent.click(container.querySelector('[data-testid="chat-mode-btn-test"]') as HTMLButtonElement);
    await tick();
    const input = container.querySelector('[data-testid="chat-input"]') as HTMLInputElement;
    expect(input.getAttribute('placeholder')).toBe(CHAT_PLACEHOLDERS.test);
  });

  it('test_mode_switch_preserves_draft', async () => {
    const { container } = render(ChatPanel);
    await tick();
    const input = container.querySelector('[data-testid="chat-input"]') as HTMLInputElement;
    fireEvent.input(input, { target: { value: 'hello' } });
    await tick();
    fireEvent.click(container.querySelector('[data-testid="chat-mode-btn-ask"]') as HTMLButtonElement);
    await tick();
    expect(get(chatDraft)).toBe('hello');
    expect((container.querySelector('[data-testid="chat-input"]') as HTMLInputElement).value).toBe('hello');
  });

  it('test_mode_aria_pressed_tracks_active', async () => {
    const { container } = render(ChatPanel);
    await tick();
    const editBtn = container.querySelector('[data-testid="chat-mode-btn-edit"]') as HTMLButtonElement;
    const askBtn  = container.querySelector('[data-testid="chat-mode-btn-ask"]')  as HTMLButtonElement;
    const testBtn = container.querySelector('[data-testid="chat-mode-btn-test"]') as HTMLButtonElement;
    expect(editBtn.getAttribute('aria-pressed')).toBe('true');
    expect(askBtn.getAttribute('aria-pressed')).toBe('false');
    expect(testBtn.getAttribute('aria-pressed')).toBe('false');
    fireEvent.click(askBtn);
    await tick();
    expect(editBtn.getAttribute('aria-pressed')).toBe('false');
    expect(askBtn.getAttribute('aria-pressed')).toBe('true');
    expect(testBtn.getAttribute('aria-pressed')).toBe('false');
  });

  it('test_activeLens_change_preserves_chat_state', async () => {
    const { container } = render(ChatPanel);
    await tick();
    const input = container.querySelector('[data-testid="chat-input"]') as HTMLInputElement;
    const sendBtn = container.querySelector('[data-testid="chat-send-btn"]') as HTMLButtonElement;
    fireEvent.input(input, { target: { value: 'check this' } });
    await tick();
    fireEvent.click(sendBtn);
    await tick();
    fireEvent.click(container.querySelector('[data-testid="chat-mode-btn-ask"]') as HTMLButtonElement);
    await tick();
    const msgCountBefore     = get(chatMessages).length;
    const modeBefore         = get(chatMode);
    const previewCountBefore = get(chatPreviewCards).length;
    activeLens.set('data');
    await tick();
    expect(get(chatMessages).length).toBe(msgCountBefore);
    expect(get(chatMode)).toBe(modeBefore);
    expect(get(chatPreviewCards).length).toBe(previewCountBefore);
  });

  it('test_no_console_testid_in_chat_panel', () => {
    const { container } = render(ChatPanel);
    const panel = container.querySelector('[data-testid="chat-panel"]') as HTMLElement;
    expect(panel.querySelector('[data-testid="bp-console"]')).toBeNull();
    expect(panel.querySelector('[data-testid="bp-tab-console"]')).toBeNull();
    const hasConsoleLabel = Array.from(panel.querySelectorAll('button, span, div'))
      .some((el) => el.textContent?.trim() === 'Console');
    expect(hasConsoleLabel).toBe(false);
  });

  it('test_no_bottom_panel_testid_in_chat_panel', () => {
    const { container } = render(ChatPanel);
    const panel = container.querySelector('[data-testid="chat-panel"]') as HTMLElement;
    expect(panel.querySelector('[data-testid="bottom-panel"]')).toBeNull();
  });

  it('test_send_does_not_write_global_stores', async () => {
    const { container } = render(ChatPanel);
    await tick();
    const selectionBefore = get(selection);
    const pathBefore      = get(path);
    const lensBefore      = get(activeLens);
    const overlaysBefore  = get(overlays);
    const input = container.querySelector('[data-testid="chat-input"]') as HTMLInputElement;
    const sendBtn = container.querySelector('[data-testid="chat-send-btn"]') as HTMLButtonElement;
    fireEvent.input(input, { target: { value: 'test message' } });
    await tick();
    fireEvent.click(sendBtn);
    await tick();
    const confirmBtn = container.querySelector('[data-testid="chat-preview-confirm"]') as HTMLButtonElement | null;
    if (confirmBtn) { fireEvent.click(confirmBtn); await tick(); }
    expect(get(selection)).toEqual(selectionBefore);
    expect(get(path)).toEqual(pathBefore);
    expect(get(activeLens)).toBe(lensBefore);
    expect(get(overlays)).toEqual(overlaysBefore);
  });

  // -----------------------------------------------------------------------
  // Phase 16 task 16.1 — backend wiring
  // -----------------------------------------------------------------------

  it('test_send_invokes_run_agent_with_sendtime_context', async () => {
    // 16.1-A: context read at SEND time. Set stores AFTER mount, then send.
    const { container } = render(ChatPanel);
    await tick();
    selection.set({ kind: 'function', id: 'mod/fn' });
    path.set(['mod', 'fn']);
    activeLens.set('rules');
    chatMode.set('ask');
    await tick();

    invoke.mockResolvedValueOnce('r-99');
    const input = container.querySelector('[data-testid="chat-input"]') as HTMLInputElement;
    fireEvent.input(input, { target: { value: 'inspect' } });
    await tick();
    fireEvent.click(container.querySelector('[data-testid="chat-send-btn"]') as HTMLButtonElement);
    await Promise.resolve();
    await tick();

    const call = invoke.mock.calls.find((c) => c[0] === 'run_agent');
    expect(call).toBeTruthy();
    const req = call![1].req;
    expect(req.text).toBe('inspect');
    expect(req.selectionKind).toBe('function');
    expect(req.selectionId).toBe('mod/fn');
    expect(req.path).toEqual(['mod', 'fn']);
    expect(req.lens).toBe('rules');
    expect(req.mode).toBe('ask');

    // currentRunId is set after the invoke promise resolves.
    await tick();
    expect(get(currentRunId)).toBe('r-99');
  });

  it('test_send_blocked_while_agent_running', async () => {
    const { container } = render(ChatPanel);
    await tick();
    // Simulate an in-flight run.
    isAgentRunning.set(true);
    await tick();

    const before = invoke.mock.calls.length;
    const msgsBefore = get(chatMessages).length;

    const input = container.querySelector('[data-testid="chat-input"]') as HTMLInputElement;
    fireEvent.input(input, { target: { value: 'second send' } });
    await tick();
    fireEvent.click(container.querySelector('[data-testid="chat-send-btn"]') as HTMLButtonElement);
    await tick();

    // No invoke, no user message appended.
    const runAgentCalls = invoke.mock.calls.slice(before).filter((c) => c[0] === 'run_agent');
    expect(runAgentCalls.length).toBe(0);
    expect(get(chatMessages).length).toBe(msgsBefore);
  });

  it('test_stop_button_visible_only_when_running', async () => {
    const { container } = render(ChatPanel);
    await tick();
    expect(container.querySelector('[data-testid="chat-stop-btn"]')).toBeNull();

    isAgentRunning.set(true);
    await tick();
    expect(container.querySelector('[data-testid="chat-stop-btn"]')).not.toBeNull();

    isAgentRunning.set(false);
    await tick();
    expect(container.querySelector('[data-testid="chat-stop-btn"]')).toBeNull();
  });

  it('test_stop_click_calls_cancel_agent_run_with_current_id', async () => {
    const { container } = render(ChatPanel);
    isAgentRunning.set(true);
    currentRunId.set('r-42');
    await tick();
    invoke.mockResolvedValueOnce({ cancelled: true });

    fireEvent.click(container.querySelector('[data-testid="chat-stop-btn"]') as HTMLButtonElement);
    await tick();

    const call = invoke.mock.calls.find((c) => c[0] === 'cancel_agent_run');
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ runId: 'r-42' });
  });

  it('test_send_error_clears_running_state_and_appends_error_message', async () => {
    const { container } = render(ChatPanel);
    await tick();
    invoke.mockRejectedValueOnce(new Error('spawn failed'));
    const input = container.querySelector('[data-testid="chat-input"]') as HTMLInputElement;
    fireEvent.input(input, { target: { value: 'go' } });
    await tick();
    fireEvent.click(container.querySelector('[data-testid="chat-send-btn"]') as HTMLButtonElement);
    // Give the rejected promise a microtask to resolve.
    await Promise.resolve();
    await Promise.resolve();
    await tick();

    expect(get(isAgentRunning)).toBe(false);
    expect(get(currentRunId)).toBeNull();
    const msgs = get(chatMessages);
    const last = msgs[msgs.length - 1];
    expect(last.role).toBe('assistant');
    expect(last.text).toContain('Agent error');
    expect(last.text).toContain('spawn failed');
  });
});
