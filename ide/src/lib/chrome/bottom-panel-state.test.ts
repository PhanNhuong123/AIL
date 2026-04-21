import { describe, it, expect } from 'vitest';
import { get } from 'svelte/store';
import {
  bottomActiveTab,
  bottomCollapsed,
  resetBottomPanelState,
  CONSOLE_SEED,
} from './bottom-panel-state';

describe('bottom-panel-state', () => {
  it('defaults', () => {
    resetBottomPanelState();
    expect(get(bottomActiveTab)).toBe('chat');
    expect(get(bottomCollapsed)).toBe(false);
  });

  it('resetBottomPanelState restores defaults', () => {
    bottomActiveTab.set('console');
    bottomCollapsed.set(true);
    resetBottomPanelState();
    expect(get(bottomActiveTab)).toBe('chat');
    expect(get(bottomCollapsed)).toBe(false);
  });

  it('CONSOLE_SEED has 3 lines with levels ok/warn/fail', () => {
    expect(CONSOLE_SEED.length).toBe(3);
    expect(CONSOLE_SEED.map((l) => l.level)).toEqual(['ok', 'warn', 'fail']);
  });
});
