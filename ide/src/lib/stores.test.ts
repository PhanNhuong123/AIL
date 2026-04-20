import { describe, it, expect } from 'vitest';
import { get } from 'svelte/store';
import {
  graph,
  selection,
  overlays,
  path,
  history,
  paletteVisible,
  theme,
  density,
} from './stores';

describe('stores initial values', () => {
  it('graph starts null', () => {
    expect(get(graph)).toBeNull();
  });

  it('selection starts as none', () => {
    expect(get(selection)).toEqual({ kind: 'none', id: null });
  });

  it('overlays default to verify-only', () => {
    expect(get(overlays)).toEqual({ verify: true, coverage: false, agent: false });
  });

  it('path starts empty', () => {
    expect(get(path)).toEqual([]);
  });

  it('history starts empty both directions', () => {
    expect(get(history)).toEqual({ back: [], forward: [] });
  });

  it('paletteVisible starts closed', () => {
    expect(get(paletteVisible)).toBe(false);
  });

  it('theme starts dark', () => {
    expect(get(theme)).toBe('dark');
  });

  it('density starts comfortable', () => {
    expect(get(density)).toBe('comfortable');
  });
});
