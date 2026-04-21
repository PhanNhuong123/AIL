/**
 * bottom-panel-state.ts — Bottom-panel-scoped stores and seed constants.
 *
 * Invariant: these stores have NO subscription to the global `graph` store.
 * Tab selection and collapse are driven by BottomPanel.svelte handlers only.
 *
 * Write rules:
 *   bottomActiveTab  — written ONLY by selectTab() in BottomPanel.svelte
 *   bottomCollapsed  — written ONLY by toggleCollapse() in BottomPanel.svelte
 *
 * Seed data uses static timestamp strings (no Date.now()) so tests stay
 * deterministic.
 */

import { writable, type Writable } from 'svelte/store';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type BottomTab = 'chat' | 'console';
export type ConsoleLevel = 'ok' | 'warn' | 'fail';

export interface ConsoleLine {
  id: string;
  level: ConsoleLevel;
  text: string;
  timestamp: string;
}

export interface ChatMessage {
  id: string;
  role: 'assistant' | 'user';
  text: string;
}

export interface PreviewCardModel {
  id: string;
  title: string;
  summary: string;
}

// ---------------------------------------------------------------------------
// Stores
// ---------------------------------------------------------------------------

export const bottomActiveTab: Writable<BottomTab> = writable('chat');
export const bottomCollapsed: Writable<boolean> = writable(false);

// ---------------------------------------------------------------------------
// Seed constants
// ---------------------------------------------------------------------------

export const ASSISTANT_SEED: ChatMessage = {
  id: 'seed-assistant-1',
  role: 'assistant',
  text: 'Hi — I see Check Balance has a failing rule. Ask me to fix it, or drill in to see the counterexample.',
};

export const PREVIEW_SEED: PreviewCardModel = {
  id: 'seed-preview-1',
  title: 'Proposed: add rate limiter before transfer',
  summary: 'Inserts RateLimiter node with 5 req/s before Billing.transfer.',
};

export const TIP_TEXT =
  'Tip: try "add a rate limiter before the transfer" or "why is Check Balance failing?"';

export const INPUT_PLACEHOLDER = 'Describe what you want to change…';

export const CONSOLE_SEED: ConsoleLine[] = [
  {
    id: 'c1',
    level: 'ok',
    text: 'Verified 47 nodes, 12 contracts, 0 failures',
    timestamp: '10:12:03',
  },
  {
    id: 'c2',
    level: 'warn',
    text: 'Billing › transfer: 1 step missing explicit type',
    timestamp: '10:12:04',
  },
  {
    id: 'c3',
    level: 'fail',
    text: 'Check Balance: counterexample (amount=-1)',
    timestamp: '10:12:05',
  },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Resets both stores to their defaults.
 * Called in test beforeEach and on IDE reset.
 */
export function resetBottomPanelState(): void {
  bottomActiveTab.set('chat');
  bottomCollapsed.set(false);
}
