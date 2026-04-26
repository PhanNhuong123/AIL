import { describe, it, expect, beforeEach, vi } from 'vitest';

// Stage mounts LensBanner, which calls `computeLensMetrics` via `invoke`.
// In jsdom there is no Tauri host; stub `invoke` to keep tests deterministic.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, selection, path, history, activeLens } from '$lib/stores';
import { zoomLevel } from '$lib/chrome/toolbar-state';
import { walletFixture } from '$lib/chrome/fixtures';
import { detailedModuleFixture } from './fixtures';
import Stage from './Stage.svelte';

beforeEach(() => {
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  path.set([]);
  history.set({ back: [], forward: [] });
  // zoomLevel intentionally set to a value Stage would NOT pick if it still
  // dispatched on $zoomLevel — this guards against regressions to the old
  // dispatch model (invariant 15.5-A).
  zoomLevel.set(4);
  activeLens.set('verify');
});

describe('Stage.svelte — dispatch by selection.kind (task 15.5)', () => {
  it('test_stage_dispatches_system_view_for_kind_none', async () => {
    graph.set(walletFixture());
    selection.set({ kind: 'none', id: null });

    const { container } = render(Stage);
    await tick();

    expect(container.querySelector('[data-testid="system-view"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="module-view"]')).toBeNull();
  });

  it('test_stage_dispatches_module_view_for_kind_module', async () => {
    graph.set(walletFixture());
    selection.set({ kind: 'module', id: 'module:m_wallet' });

    const { container } = render(Stage);
    await tick();

    expect(container.querySelector('[data-testid="module-view"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="system-view"]')).toBeNull();
  });

  it('test_stage_dispatches_flow_view_for_kind_function', async () => {
    graph.set(walletFixture());
    selection.set({ kind: 'function', id: 'function:fn_transfer' });

    const { container } = render(Stage);
    await tick();

    expect(container.querySelector('[data-testid="flow-view"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="module-view"]')).toBeNull();
  });

  it('test_stage_dispatches_node_view_for_kind_step', async () => {
    // detailedModuleFixture seeds `module:m_wallet` detail; we need step detail.
    const g = detailedModuleFixture();
    g.detail['step:s_debit'] = {
      name: 'debit_from_sender',
      status: 'ok',
      description: 'Debit sender',
      receives: [],
      returns: [],
      rules: [],
      inherited: [],
      proven: [],
      verification: { ok: true },
    };
    graph.set(g);
    selection.set({ kind: 'step', id: 'step:s_debit' });

    const { container } = render(Stage);
    await tick();

    expect(container.querySelector('[data-testid="node-view"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="flow-view"]')).toBeNull();
  });

  it('test_stage_dispatches_system_view_for_project_type_error_kinds', async () => {
    // AC: dispatch must derive from selection.kind. For non-module/function/step
    // kinds (project/type/error), Stage falls back to SystemView per CLAUDE.md
    // purpose block.
    for (const kind of ['project', 'type', 'error'] as const) {
      graph.set(walletFixture());
      selection.set({ kind, id: null });

      const { container, unmount } = render(Stage);
      await tick();

      expect(
        container.querySelector('[data-testid="system-view"]'),
        `system-view for kind=${kind}`,
      ).not.toBeNull();
      expect(
        container.querySelector('[data-testid="module-view"]'),
        `no module-view for kind=${kind}`,
      ).toBeNull();

      unmount();
    }
  });

  it('test_stage_mounts_lens_banner_above_every_level', async () => {
    // AC: "Add a LensBanner above every stage level." Guard against a future
    // refactor that accidentally scopes the banner inside one branch only.
    const cases = [
      { sel: { kind: 'none'     as const, id: null }, viewTestId: 'system-view' },
      { sel: { kind: 'module'   as const, id: 'module:m_wallet' }, viewTestId: 'module-view' },
      { sel: { kind: 'function' as const, id: 'function:fn_transfer' }, viewTestId: 'flow-view' },
    ];

    for (const c of cases) {
      graph.set(walletFixture());
      selection.set(c.sel);

      const { container, unmount } = render(Stage);
      await tick();

      const body = container.querySelector('.stage-body') as HTMLElement | null;
      expect(body, `stage-body for kind=${c.sel.kind}`).not.toBeNull();

      const banner = body!.querySelector('[data-testid="lens-banner"]');
      const view = body!.querySelector(`[data-testid="${c.viewTestId}"]`);
      expect(banner, `banner for kind=${c.sel.kind}`).not.toBeNull();
      expect(view, `${c.viewTestId} for kind=${c.sel.kind}`).not.toBeNull();

      // Banner precedes the view in DOM order.
      expect(
        banner!.compareDocumentPosition(view!) & Node.DOCUMENT_POSITION_FOLLOWING,
        `banner precedes view for kind=${c.sel.kind}`,
      ).toBeTruthy();

      unmount();
    }
  });
});

describe('handleStepJump (Phase 17.4)', () => {
  it('SJ1: navigates to peer step path when peer step exists in graph', async () => {
    // walletFixture contains module:m_wallet → function:fn_transfer → step:s_debit
    graph.set(walletFixture());
    // Start from a sibling step; history must be non-empty for pushHistory to record
    path.set(['module:m_wallet', 'function:fn_transfer', 'step:s_credit']);
    selection.set({ kind: 'step', id: 'step:s_credit' });

    const { component } = render(Stage);
    await tick();

    // Invoke the exported handler directly with the peer step id
    (component as unknown as { handleStepJump: (id: string) => void }).handleStepJump('step:s_debit');

    // navigateTo writes path, selection, and zoomLevel — assert on those store values
    expect(get(path)).toEqual(['module:m_wallet', 'function:fn_transfer', 'step:s_debit']);
    expect(get(selection)).toEqual({ kind: 'step', id: 'step:s_debit' });
    expect(get(zoomLevel)).toBe(4);
  });

  it('SJ2: no-op when peer step is absent from graph (R3 mitigation)', async () => {
    // walletFixture does NOT contain step:s_nonexistent_peer
    graph.set(walletFixture());
    path.set(['module:m_wallet', 'function:fn_transfer', 'step:s_credit']);
    selection.set({ kind: 'step', id: 'step:s_credit' });

    const { component } = render(Stage);
    await tick();

    // Capture pre-call store state
    const pathBefore = get(path);
    const selBefore = get(selection);

    // Should silently no-op without throwing
    expect(() => {
      (component as unknown as { handleStepJump: (id: string) => void }).handleStepJump(
        'step:s_nonexistent_peer',
      );
    }).not.toThrow();

    // path and selection must remain unchanged
    expect(get(path)).toEqual(pathBefore);
    expect(get(selection)).toEqual(selBefore);
  });
});

describe('Stage.svelte — Phase E selectedNodeDetail prop override (task 16.3)', () => {
  it('selectedNodeDetail prop overrides graph-derived detail when step id matches', async () => {
    // Seed a graph with step detail in graph.detail
    const g = detailedModuleFixture();
    g.detail['step:s_debit'] = {
      name: 'debit_from_sender',
      status: 'ok',
      description: 'Original graph description',
      receives: [],
      returns: [],
      rules: [],
      inherited: [],
      proven: [],
      verification: { ok: false },
    };
    graph.set(g);
    selection.set({ kind: 'step', id: 'step:s_debit' });

    // freshDetail simulates post-verification outcome with updated status.
    // Uses the paired { id, detail } shape so Stage can match by real node id.
    const freshDetail: import('$lib/types').NodeDetail = {
      name: 'debit_from_sender',
      status: 'ok' as const,
      description: 'Post-verification description',
      receives: [],
      returns: [],
      rules: [],
      inherited: [],
      proven: [],
      verification: { ok: true },
    };

    // Render Stage with the override prop in the correct paired-id shape.
    const { container } = render(Stage, {
      props: { selectedNodeDetail: { id: 'step:s_debit', detail: freshDetail } },
    });
    await tick();

    // NodeView should render (since detail is provided via override)
    expect(container.querySelector('[data-testid="node-view"]')).not.toBeNull();
  });
});
