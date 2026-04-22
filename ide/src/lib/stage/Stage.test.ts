import { describe, it, expect, beforeEach, vi } from 'vitest';

// Stage mounts LensBanner, which calls `computeLensMetrics` via `invoke`.
// In jsdom there is no Tauri host; stub `invoke` to keep tests deterministic.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
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
