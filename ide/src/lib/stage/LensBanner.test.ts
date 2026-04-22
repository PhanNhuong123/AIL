import { describe, it, expect, beforeEach, vi } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { activeLens } from '$lib/stores';
import type { Lens, LensStats } from '$lib/types';
import LensBanner from './LensBanner.svelte';
import { hueTokenFor } from './lens-banner-copy';

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(null);
  activeLens.set('verify');
});

function flushMicrotasks(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

describe('LensBanner.svelte — hue and stats (task 15.5)', () => {
  it('test_lens_banner_applies_hue_token_per_lens', async () => {
    const lenses: Lens[] = ['structure', 'rules', 'verify', 'data', 'tests'];
    for (const lens of lenses) {
      activeLens.set(lens);
      const { container, unmount } = render(LensBanner, { props: { scopeId: null } });
      await tick();

      const banner = container.querySelector('[data-testid="lens-banner"]') as HTMLElement;
      expect(banner, `banner present for ${lens}`).not.toBeNull();
      expect(banner.getAttribute('data-lens')).toBe(lens);

      const style = banner.getAttribute('style') ?? '';
      expect(style, `style contains hue token for ${lens}`).toContain(`var(${hueTokenFor(lens)})`);

      unmount();
    }
  });

  it('test_lens_banner_renders_stats_for_each_variant', async () => {
    const cases: Array<{ lens: Lens; stats: LensStats; expect: string }> = [
      {
        lens: 'structure',
        stats: { lens: 'structure', modules: 2, functions: 3, steps: 5, nodes: 10 },
        expect: '2 modules · 3 fns · 5 steps',
      },
      {
        lens: 'rules',
        stats: { lens: 'rules', total: 7, unproven: 1, broken: 0 },
        expect: '7 rules · 1 unproven · 0 broken',
      },
      {
        lens: 'verify',
        stats: { lens: 'verify', proven: 4, unproven: 2, counterexamples: 1 },
        expect: '4 proven · 2 unproven · 1 cex',
      },
      {
        lens: 'data',
        stats: { lens: 'data', types: ['Money', 'Account'], signals: 3 },
        expect: '3 signals · 2 types',
      },
      {
        lens: 'tests',
        stats: { lens: 'tests', total: 8, passing: 6, failing: 2 },
        expect: '8 tests · 6 passing · 2 failing',
      },
    ];

    for (const c of cases) {
      invokeMock.mockReset();
      invokeMock.mockResolvedValueOnce(c.stats);
      activeLens.set(c.lens);

      const { container, unmount } = render(LensBanner, { props: { scopeId: null } });
      await tick();
      await flushMicrotasks();
      await tick();

      const statsEl = container.querySelector('[data-testid="lens-banner-stats"]');
      expect(statsEl?.textContent?.trim(), `stats text for ${c.lens}`).toBe(c.expect);

      unmount();
    }
  });

  it('test_lens_banner_refetches_and_updates_stats_when_active_lens_changes', async () => {
    const verifyStats: LensStats = {
      lens: 'verify', proven: 3, unproven: 0, counterexamples: 0,
    };
    const rulesStats: LensStats = {
      lens: 'rules', total: 9, unproven: 2, broken: 1,
    };
    invokeMock.mockResolvedValueOnce(verifyStats).mockResolvedValueOnce(rulesStats);
    activeLens.set('verify');

    const { container, unmount } = render(LensBanner, {
      props: { scopeId: 'module:m_wallet' },
    });
    await tick();
    await flushMicrotasks();
    await tick();

    const statsEl = container.querySelector('[data-testid="lens-banner-stats"]');
    expect(statsEl?.textContent?.trim()).toBe('3 proven · 0 unproven · 0 cex');

    activeLens.set('rules');
    await tick();
    await flushMicrotasks();
    await tick();

    // Both invocations observed on the mock, and the rendered stats reflect the
    // new lens response — guards against silent breakage of the stats
    // assignment inside the reactive refetch.
    const calls = invokeMock.mock.calls.filter((c) => c[0] === 'compute_lens_metrics');
    const lenses = calls.map((c) => (c[1] as { lens: Lens }).lens);
    expect(lenses).toContain('verify');
    expect(lenses).toContain('rules');

    expect(statsEl?.textContent?.trim()).toBe('9 rules · 2 unproven · 1 broken');

    unmount();
  });
});
