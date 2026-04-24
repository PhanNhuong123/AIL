import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  chatMode, chatDraft, chatMessages, chatPreviewCards,
  resetChatState, CHAT_ASSISTANT_SEED, CHAT_PREVIEW_SEED, CHAT_PLACEHOLDERS,
  contextSubtextFor, suggestChipsFor, placeholderFor,
  type ChatMessage, type PreviewCardModel,
} from './chat-state';

beforeEach(() => resetChatState());

// ---------------------------------------------------------------------------
// contextSubtextFor
// ---------------------------------------------------------------------------

describe('chat-state — contextSubtextFor', () => {
  it('test_subtext_none_kind', () => {
    expect(contextSubtextFor({ kind: 'none', id: null }, [])).toBe('No selection');
  });

  it('test_subtext_null_id', () => {
    expect(contextSubtextFor({ kind: 'module', id: null }, [])).toBe('No selection');
  });

  it('test_subtext_project', () => {
    expect(contextSubtextFor({ kind: 'project', id: 'project:root' }, [])).toBe('Project');
  });

  it('test_subtext_module_prefixed_id', () => {
    expect(contextSubtextFor({ kind: 'module', id: 'module:billing' }, [])).toBe('Module: billing');
  });

  it('test_subtext_function_unprefixed_id', () => {
    expect(contextSubtextFor({ kind: 'function', id: 'fn_transfer' }, [])).toBe('Function: fn_transfer');
  });

  it('test_subtext_step', () => {
    expect(contextSubtextFor({ kind: 'step', id: 'step:s_transfer' }, [])).toBe('Step: s_transfer');
  });

  it('test_subtext_type', () => {
    expect(contextSubtextFor({ kind: 'type', id: 'type:Money' }, [])).toBe('Type: Money');
  });

  it('test_subtext_error', () => {
    expect(contextSubtextFor({ kind: 'error', id: 'error:E001' }, [])).toBe('Error: E001');
  });
});

// ---------------------------------------------------------------------------
// suggestChipsFor
// ---------------------------------------------------------------------------

describe('chat-state — suggestChipsFor', () => {
  it('test_chips_default_for_none', () => {
    const chips = suggestChipsFor({ kind: 'none', id: null }, 'verify');
    expect(chips.some((c) => c.text === 'Explain this project')).toBe(true);
  });

  it('test_chips_module_verify_has_verify_text', () => {
    const chips = suggestChipsFor({ kind: 'module', id: 'module:billing' }, 'verify');
    expect(chips.some((c) => c.text === 'Verify this module')).toBe(true);
  });

  it('test_chips_function_tests_has_generate_test', () => {
    const chips = suggestChipsFor({ kind: 'function', id: 'function:fn_transfer' }, 'tests');
    expect(chips.some((c) => c.text === 'Generate a test')).toBe(true);
  });

  it('test_chips_step_data_has_receives_returns', () => {
    const chips = suggestChipsFor({ kind: 'step', id: 'step:s_transfer' }, 'data');
    expect(chips.some((c) => c.text === 'Show receives/returns')).toBe(true);
  });

  it('test_chips_project_rules_uses_project_row', () => {
    const chips = suggestChipsFor({ kind: 'project', id: 'project:root' }, 'rules');
    // project has no rules row, falls back to project×*
    expect(chips.some((c) => c.text === 'Explain this project')).toBe(true);
  });

  it('test_chips_project_tests_uses_project_row', () => {
    const chips = suggestChipsFor({ kind: 'project', id: 'project:root' }, 'tests');
    // project has no tests row, falls back to project×*
    expect(chips.some((c) => c.text === 'Explain this project')).toBe(true);
  });

  it('test_chips_unmapped_type_tests_falls_back', () => {
    const chips = suggestChipsFor({ kind: 'type', id: 'type:Money' }, 'tests');
    // type has no tests row, falls back to type×*
    expect(chips.some((c) => c.text === 'Explain this type')).toBe(true);
  });

  it('test_chips_returns_fresh_array', () => {
    const a = suggestChipsFor({ kind: 'none', id: null }, 'verify');
    const b = suggestChipsFor({ kind: 'none', id: null }, 'verify');
    expect(a).not.toBe(b);
    a.push({ id: 'x', text: 'mutation' });
    expect(b.some((c) => c.text === 'mutation')).toBe(false);
  });

  it('test_chips_cap_at_3', () => {
    const combinations: Array<[Parameters<typeof suggestChipsFor>[0], Parameters<typeof suggestChipsFor>[1]]> = [
      [{ kind: 'module',   id: 'module:billing' },   'structure'],
      [{ kind: 'module',   id: 'module:billing' },   'verify'],
      [{ kind: 'function', id: 'fn_transfer' },       'data'],
      [{ kind: 'step',     id: 'step:s1' },           'rules'],
      [{ kind: 'type',     id: 'type:Money' },        'data'],
      [{ kind: 'error',    id: 'error:E001' },        'verify'],
    ];
    for (const [sel, lens] of combinations) {
      const chips = suggestChipsFor(sel, lens);
      expect(chips.length).toBeLessThanOrEqual(3);
    }
  });
});

// ---------------------------------------------------------------------------
// placeholderFor
// ---------------------------------------------------------------------------

describe('chat-state — placeholderFor', () => {
  it('test_placeholder_edit', () => {
    expect(placeholderFor('edit')).toBe(CHAT_PLACEHOLDERS.edit);
  });

  it('test_placeholder_ask', () => {
    expect(placeholderFor('ask')).toBe(CHAT_PLACEHOLDERS.ask);
  });

  it('test_placeholder_test', () => {
    expect(placeholderFor('test')).toBe(CHAT_PLACEHOLDERS.test);
  });
});

// ---------------------------------------------------------------------------
// resetChatState
// ---------------------------------------------------------------------------

describe('chat-state — resetChatState', () => {
  it('test_reset_restores_defaults', () => {
    chatMode.set('ask');
    chatDraft.set('hello');
    chatMessages.set([]);
    chatPreviewCards.set([]);

    resetChatState();

    expect(get(chatMode)).toBe('edit');
    expect(get(chatDraft)).toBe('');
    expect(get(chatMessages)).toEqual([CHAT_ASSISTANT_SEED]);
    expect(get(chatPreviewCards)).toEqual([CHAT_PREVIEW_SEED]);
  });
});
