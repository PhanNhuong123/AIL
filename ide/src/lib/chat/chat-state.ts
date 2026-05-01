/**
 * chat-state.ts — Feature-scoped stores and helpers for ChatPanel.
 *
 * Ownership:
 *   (a) chatMode       — the active input mode ('edit' | 'ask' | 'test')
 *   (b) chatDraft      — current draft string in the input row
 *   (c) chatMessages   — the conversation message list (seed + exchange)
 *   (d) chatPreviewCards — pending AI-proposed change cards
 *   (e) isAgentRunning — Phase 16: true between runAgent() and agent-complete
 *   (f) currentRunId   — Phase 16: stringified run id returned by run_agent
 *
 * Write rules:
 *   chatMode       — written ONLY by handleModeChange in ChatPanel.svelte
 *                    OR chip dispatch (handleChipClick does NOT change mode)
 *   chatDraft      — written ONLY by handleDraftChange, handleChipClick,
 *                    and handleSend (clears to '') in ChatPanel.svelte
 *   chatMessages   — written ONLY by handleSend in ChatPanel.svelte,
 *                    the route-level onAgentStep/onAgentMessage/
 *                    onAgentComplete handlers in +page.svelte (guarded by
 *                    currentRunId), and resetChatState()
 *   chatPreviewCards — written ONLY by handlePreviewConfirm / Adjust /
 *                    Discard in ChatPanel.svelte, the route-level
 *                    onAgentMessage handler (preview append), the route-level
 *                    handlePreviewApply handler (remove on apply), and
 *                    resetChatState()
 *   isAgentRunning — written ONLY by handleSend (true before await),
 *                    handleSend.catch (false on reject), handleStop (false
 *                    on ack), the route-level onAgentComplete handler
 *                    (false on matching runId), and resetChatState()
 *   currentRunId   — written ONLY by handleSend (set to runAgent resolve
 *                    value), handleSend.catch (null), handleStop (null),
 *                    the route-level onAgentComplete handler (null on
 *                    matching runId), and resetChatState()
 *
 * INVARIANT 15.10-B: No file under ide/src/lib/chat/** may import from
 * $lib/chrome/bottom-panel-state.ts. Types ChatMessage, PreviewCardModel,
 * SuggestChip are re-declared locally here.
 *
 * INVARIANT 16.1-C: PreviewCardModel.patch is a typed GraphPatchJson
 * imported from $lib/types (NOT bottom-panel-state). Preview Apply is
 * applied only by the +page.svelte handler through applyGraphPatch;
 * chat components never write graph or selection stores.
 */

import { writable } from 'svelte/store';
import type { Writable } from 'svelte/store';
import type { Selection } from '$lib/stores';
import type { GraphPatchJson, Lens } from '$lib/types';

// ---------------------------------------------------------------------------
// Types — re-declared locally (NOT imported from bottom-panel-state)
// ---------------------------------------------------------------------------

export type ChatRole = 'assistant' | 'user';
export type ChatMode = 'edit' | 'ask' | 'test';

export interface ChatMessage {
  id: string;
  role: ChatRole;
  text: string;
}

export interface PreviewCardModel {
  id: string;
  title: string;
  summary: string;
  /** Phase 16: optional canonical graph patch applied by `+page.svelte`
   *  handler on `previewapply`. Absent on seed cards and cards that carry
   *  pure-informational previews. */
  patch?: GraphPatchJson;
  /** Phase 16: run id that produced this card. */
  runId?: string;
  /** Phase 16: agent messageId that produced this card. */
  messageId?: string;
}

export interface SuggestChip {
  id: string;
  text: string;
}

// ---------------------------------------------------------------------------
// Seed constants
// ---------------------------------------------------------------------------

export const CHAT_ASSISTANT_SEED: ChatMessage = {
  id: 'chat-seed-assistant-1',
  role: 'assistant',
  text: 'Hi — I am your AIL assistant. Open or create a project, then ask me to explain, verify, or modify it.',
};

export const CHAT_PLACEHOLDERS: Record<ChatMode, string> = {
  edit: 'Describe what you want to change…',
  ask:  'Ask about this module, function, or step…',
  test: 'Describe a test scenario to run…',
};

// ---------------------------------------------------------------------------
// Stores
// ---------------------------------------------------------------------------

export const chatMode:         Writable<ChatMode>           = writable('edit');
export const chatDraft:        Writable<string>             = writable('');
export const chatMessages:     Writable<ChatMessage[]>      = writable([CHAT_ASSISTANT_SEED]);
// Preview cards default empty: live cards arrive from the agent stream
// (`onAgentMessage` preview append). The previous seed card referenced a
// fictional rate-limiter rule which misled users on an empty workspace.
export const chatPreviewCards: Writable<PreviewCardModel[]> = writable([]);
export const isAgentRunning:   Writable<boolean>            = writable(false);
export const currentRunId:     Writable<string | null>      = writable(null);

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

export function resetChatState(): void {
  chatMode.set('edit');
  chatDraft.set('');
  chatMessages.set([CHAT_ASSISTANT_SEED]);
  chatPreviewCards.set([]);
  isAgentRunning.set(false);
  currentRunId.set(null);
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

export function placeholderFor(mode: ChatMode): string {
  return CHAT_PLACEHOLDERS[mode];
}

/** Returns the text after the first ':' in id, or id if no ':' present. */
function lastSegmentName(id: string): string {
  const colon = id.indexOf(':');
  return colon === -1 ? id : id.slice(colon + 1);
}

/**
 * Returns a human-readable context subtext for the current selection.
 * The path parameter is unused in 15.10 — reserved for Phase 16 extension.
 */
export function contextSubtextFor(selection: Selection, _path: string[]): string {
  if (selection.kind === 'none' || selection.id === null) return 'No selection';
  switch (selection.kind) {
    case 'project':  return 'Project';
    case 'module':   return 'Module: '   + lastSegmentName(selection.id);
    case 'function': return 'Function: ' + lastSegmentName(selection.id);
    case 'step':     return 'Step: '     + lastSegmentName(selection.id);
    case 'type':     return 'Type: '     + lastSegmentName(selection.id);
    case 'error':    return 'Error: '    + lastSegmentName(selection.id);
    default:         return 'No selection';
  }
}

// ---------------------------------------------------------------------------
// Chip table
// ---------------------------------------------------------------------------

type ChipLens = Lens | '*';

const DEFAULT_CHIP_TEXTS: string[] = [
  'Explain this project',
  'Show failing rules',
  'Where do I start?',
];

export const DEFAULT_CHIPS: SuggestChip[] = DEFAULT_CHIP_TEXTS.map((text, i) => ({
  id: `chip-default-${i + 1}`,
  text,
}));

type KindKey = Selection['kind'];

const CHIP_TABLE: Partial<Record<KindKey, Partial<Record<ChipLens, string[]>>>> = {
  module: {
    structure: ['List functions', 'Show module dependencies', 'Explain this module'],
    rules:     ['List module rules', 'What rules are unproven?', 'Add a rule here'],
    verify:    ['Verify this module', 'Show broken rules', "What's failing?"],
    data:      ['List inputs/outputs', 'Show signal flow', 'Which types are used?'],
    tests:     ['Run module tests', 'Show failing tests', 'Generate a test'],
    '*':       ['Explain this module', 'Verify this module', 'Add a function'],
  },
  function: {
    structure: ['List steps', 'Show step order', 'Explain this function'],
    rules:     ['List function rules', 'Unproven rules?', 'Add a rule'],
    verify:    ['Verify this function', 'Why is it failing?', 'Show counterexamples'],
    data:      ['Show receives/returns', 'Trace data flow', 'List types touched'],
    tests:     ['Run function tests', 'Failing tests?', 'Generate a test'],
    '*':       ['Explain this function', 'Verify this function', 'Add a step'],
  },
  step: {
    structure: ['Explain this step', 'Show call site', 'Show neighboring steps'],
    rules:     ['Show step rules', 'Unproven here?', 'Add a rule to this step'],
    verify:    ['Why is this failing?', 'Show counterexample', 'Propose a fix'],
    data:      ['Show receives/returns', 'Show types', 'Trace inputs'],
    tests:     ['Run this step test', 'Generate a test for this step', 'Show test history'],
    '*':       ['Explain this step', 'Verify this step', 'Propose a fix'],
  },
  type: {
    data: ['Where is this type used?', 'Show fields', 'Rename this type'],
    '*':  ['Explain this type', 'Where is it used?'],
  },
  error: {
    verify: ['Which node raises this?', 'Show failing rules', 'Propose a fix'],
    '*':    ['Where is this raised?', 'Explain this error'],
  },
  project: {
    '*': ['Explain this project', 'Verify the whole project', 'Show health summary'],
  },
};

function makeChips(kind: KindKey, lensOrStar: ChipLens, texts: string[]): SuggestChip[] {
  return texts.slice(0, 3).map((text, i) => ({
    id: `chip-${kind}-${lensOrStar}-${i + 1}`,
    text,
  }));
}

/**
 * Returns adaptive suggest chips based on current selection and active lens.
 * Always returns a fresh array (no aliasing). At most 3 chips.
 *
 * Falls back to DEFAULT_CHIPS when selection is none/null or no mapping found.
 */
export function suggestChipsFor(selection: Selection, lens: Lens): SuggestChip[] {
  if (selection.kind === 'none' || selection.id === null) {
    return [...DEFAULT_CHIPS];
  }

  const kind = selection.kind;
  const kindMap = CHIP_TABLE[kind];
  if (!kindMap) return [...DEFAULT_CHIPS];

  const lensTexts = kindMap[lens as ChipLens];
  if (lensTexts) {
    return makeChips(kind, lens, lensTexts);
  }

  const starTexts = kindMap['*'];
  if (starTexts) {
    return makeChips(kind, '*', starTexts);
  }

  return [...DEFAULT_CHIPS];
}
