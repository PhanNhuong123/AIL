import { describe, it, expect, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { graph, selection, overlays, path, history, paletteVisible } from '$lib/stores';
import { zoomLevel, pickerOpen, pickerItems } from './toolbar-state';
import { walletFixture } from './fixtures';
import Toolbar from './Toolbar.svelte';

beforeEach(() => {
  graph.set(null);
  selection.set({ kind: 'none', id: null });
  overlays.set({ rules: false, verification: true, dataflow: false, dependencies: false, tests: false });
  path.set([]);
  history.set({ back: [], forward: [] });
  paletteVisible.set(false);
  zoomLevel.set(0);
  pickerOpen.set(false);
  pickerItems.set([]);
});

describe('Toolbar.svelte', () => {
  it('test_back_disabled_on_empty_history', async () => {
    const { container } = render(Toolbar);
    await tick();

    const backBtn = container.querySelector('[data-testid="toolbar-back"]');
    expect(backBtn).not.toBeNull();
    expect((backBtn as HTMLButtonElement).disabled).toBe(true);
  });

  it('test_forward_enabled_after_back', async () => {
    // Set up a path and prime history: push one entry then go back
    graph.set(walletFixture());
    path.set(['project:root']);
    history.set({ back: [JSON.stringify(['project:root'])], forward: [] });
    // Simulate goBack: move back entry to forward
    history.set({ back: [], forward: [JSON.stringify(['project:root'])] });
    path.set([]);

    const { container } = render(Toolbar);
    await tick();

    const fwdBtn = container.querySelector('[data-testid="toolbar-forward"]') as HTMLButtonElement;
    expect(fwdBtn).not.toBeNull();
    expect(fwdBtn.disabled).toBe(false);
    expect(get(history).forward.length).toBe(1);
  });

  it('test_breadcrumb_click_navigates', async () => {
    graph.set(walletFixture());
    path.set(['project:root', 'module:m_wallet', 'function:fn_transfer']);
    selection.set({ kind: 'function', id: 'function:fn_transfer' });

    const { container } = render(Toolbar);
    await tick();

    const crumb0 = container.querySelector('[data-testid="toolbar-crumb-0"]');
    expect(crumb0).not.toBeNull();
    fireEvent.click(crumb0!);
    await tick();

    const p = get(path);
    expect(p).toEqual(['project:root']);
    expect(get(selection).kind).toBe('project');
    expect(get(selection).id).toBe('project:root');
    expect(get(history).back.length).toBe(1);
  });

  it('test_zoom_out_decrements_level', async () => {
    graph.set(walletFixture());
    path.set(['project:root', 'module:m_wallet']);
    selection.set({ kind: 'module', id: 'module:m_wallet' });
    zoomLevel.set(1);

    const { container } = render(Toolbar);
    await tick();

    const zoomOutBtn = container.querySelector('[data-testid="toolbar-zoom-out"]');
    expect(zoomOutBtn).not.toBeNull();
    fireEvent.click(zoomOutBtn!);
    await tick();

    expect(get(zoomLevel)).toBe(0);
    const label = container.querySelector('[data-testid="toolbar-zoom-label"]');
    expect(label?.textContent).toBe('System');
  });

  it('test_zoom_in_drills_single_child', async () => {
    // m_auth has exactly 1 function: fn_login
    graph.set(walletFixture());
    path.set(['project:root', 'module:m_auth']);
    selection.set({ kind: 'module', id: 'module:m_auth' });
    zoomLevel.set(1);

    const { container } = render(Toolbar);
    await tick();

    const zoomInBtn = container.querySelector('[data-testid="toolbar-zoom-in"]');
    expect(zoomInBtn).not.toBeNull();
    fireEvent.click(zoomInBtn!);
    await tick();

    expect(get(zoomLevel)).toBe(2);
    expect(get(selection).id).toBe('function:fn_login');
    expect(get(pickerOpen)).toBe(false);
  });

  it('test_zoom_in_multi_child_opens_picker', async () => {
    // m_wallet has 2 functions: fn_transfer, fn_balance
    graph.set(walletFixture());
    path.set(['project:root', 'module:m_wallet']);
    selection.set({ kind: 'module', id: 'module:m_wallet' });
    zoomLevel.set(1);

    const { container } = render(Toolbar);
    await tick();

    const zoomInBtn = container.querySelector('[data-testid="toolbar-zoom-in"]');
    expect(zoomInBtn).not.toBeNull();
    fireEvent.click(zoomInBtn!);
    await tick();

    expect(get(pickerOpen)).toBe(true);
    const picker = container.querySelector('[data-testid="toolbar-zoom-picker"]');
    expect(picker).not.toBeNull();
    const items = container.querySelectorAll('[data-testid^="toolbar-picker-item-"]');
    expect(items.length).toBe(2);
  });

  it('test_overlay_toggles_independent', async () => {
    const { container } = render(Toolbar);
    await tick();

    // Click rules overlay — verification should remain true
    const rulesBtn = container.querySelector('[data-testid="toolbar-overlay-rules"]');
    expect(rulesBtn).not.toBeNull();
    fireEvent.click(rulesBtn!);
    await tick();

    const o = get(overlays);
    expect(o.rules).toBe(true);
    expect(o.verification).toBe(true); // still true — independent toggle [16.4-A]
  });

  it('test_overlay_state_persists_on_navigation', async () => {
    graph.set(walletFixture());
    path.set(['project:root', 'module:m_wallet']);
    selection.set({ kind: 'module', id: 'module:m_wallet' });

    // Activate rules overlay
    overlays.update(o => ({ ...o, rules: true }));

    const { container } = render(Toolbar);
    await tick();

    // Click crumb-0 (project:root) to navigate
    const crumb0 = container.querySelector('[data-testid="toolbar-crumb-0"]');
    expect(crumb0).not.toBeNull();
    fireEvent.click(crumb0!);
    await tick();

    // Overlay state must survive navigation
    expect(get(overlays).rules).toBe(true);
  });

  it('test_palette_toggle_updates_navigator', async () => {
    const { container } = render(Toolbar);
    await tick();

    expect(get(paletteVisible)).toBe(false);

    const paletteBtn = container.querySelector('[data-testid="toolbar-palette"]');
    expect(paletteBtn).not.toBeNull();

    fireEvent.click(paletteBtn!);
    await tick();
    expect(get(paletteVisible)).toBe(true);

    fireEvent.click(paletteBtn!);
    await tick();
    expect(get(paletteVisible)).toBe(false);
  });

  it('test_verify_overlay_default_on', async () => {
    // Fresh stores (beforeEach resets overlays to the default shape)
    const { container } = render(Toolbar);
    await tick();

    const o = get(overlays);
    expect(o.verification).toBe(true);
    expect(o.rules).toBe(false);
    expect(o.dataflow).toBe(false);
    expect(o.dependencies).toBe(false);
    expect(o.tests).toBe(false);
  });
});
