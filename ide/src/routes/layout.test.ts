import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { activeLens, overlays, graph } from '$lib/stores';
import type { Lens } from '$lib/stores';
import Page from './+page.svelte';

beforeEach(() => {
  graph.set(null);
  activeLens.set('verify');
  overlays.set({ rules: false, verification: true, dataflow: false, dependencies: false, tests: false });
});

describe('+page.svelte — canonical 3-column layout', () => {
  it('test_root_mounts_three_regions', () => {
    const { container } = render(Page);

    expect(container.querySelector('[data-testid="region-titlebar"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="region-navigator"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="region-stage"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="chat-panel"]')).not.toBeNull();
  });

  it('test_removed_components_absent_from_shell', () => {
    const { container } = render(Page);

    expect(container.querySelector('.region-toolbar')).toBeNull();
    expect(container.querySelector('[data-testid="context-panel"]')).toBeNull();
    expect(container.querySelector('[data-testid="bottom-panel"]')).toBeNull();
  });

  it('test_app_root_has_no_bottom_collapsed_class', () => {
    const { container } = render(Page);
    const root = container.querySelector('[data-testid="app-root"]');
    expect(root).not.toBeNull();
    expect(root?.classList.contains('bottom-collapsed')).toBe(false);
  });

  it('test_activeLens_store_defaults_to_verify', () => {
    render(Page);
    expect(get(activeLens)).toBe('verify' satisfies Lens);
  });

  it('test_overlays_store_preserved_unchanged', () => {
    render(Page);
    const o = get(overlays);
    expect(o.rules).toBe(false);
    expect(o.verification).toBe(true);
    expect(o.dataflow).toBe(false);
    expect(o.dependencies).toBe(false);
    expect(o.tests).toBe(false);
  });
});
