<script lang="ts">
  import { onMount } from 'svelte';
  import { graph, path, tweaksPanelOpen, activeLens, quickCreateModalOpen } from '$lib/stores';
  import type { Lens } from '$lib/stores';
  import { countPills, breadcrumbs } from './rollup';
  import TrafficLights from './TrafficLights.svelte';
  import Icon from '$lib/icons/Icon.svelte';

  const isMac = typeof navigator !== 'undefined' && navigator.platform.includes('Mac');
  const LENSES = ['structure', 'rules', 'verify', 'data', 'tests'] as const;

  $: crumbs = breadcrumbs($graph, $path);
  $: pills  = countPills($graph);

  function handleCrumbClick(index) {
    path.set($path.slice(0, index + 1));
  }

  function toggleTweaks() {
    tweaksPanelOpen.update((v) => !v);
  }

  function setLens(lens) {
    activeLens.set(lens as Lens);
  }

  function openQuickCreate() {
    quickCreateModalOpen.set(true);
  }

  onMount(() => {
    function handleKeydown(e) {
      const ev = e as KeyboardEvent;
      if ((ev.metaKey || ev.ctrlKey) && ev.key.toLowerCase() === 'k') {
        ev.preventDefault();
        openQuickCreate();
      }
    }
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
</script>

<header class="region-titlebar" data-testid="region-titlebar" data-tauri-drag-region>
  <TrafficLights variant={isMac ? 'mac' : 'generic'} />

  <div class="brand" data-tauri-drag-region>
    <span class="brand-name">AIL</span>
    <span class="brand-badge">v2</span>
  </div>

  {#if crumbs.length > 0}
    <nav class="breadcrumbs" aria-label="Navigation breadcrumbs" data-tauri-drag-region>
      {#each crumbs as crumb, i}
        {#if i > 0}
          <span class="crumb-sep" aria-hidden="true">
            <Icon name="chevron-right" size={10} />
          </span>
        {/if}
        <button
          class="crumb"
          on:click={() => handleCrumbClick(i)}
          aria-current={i === crumbs.length - 1 ? 'page' : undefined}
        >
          {crumb.name}
        </button>
      {/each}
    </nav>
  {/if}

  <div
    class="lens-group"
    role="group"
    aria-label="Active lens"
    data-testid="lens-group"
  >
    {#each LENSES as lens}
      <button
        type="button"
        class="lens-pill"
        class:active={$activeLens === lens}
        aria-pressed={$activeLens === lens}
        data-testid="lens-{lens}"
        on:click={() => setLens(lens)}
      >
        {lens}
      </button>
    {/each}
  </div>

  <div class="spacer" data-tauri-drag-region></div>

  {#if $graph !== null}
    <div class="pills" aria-label="Status summary">
      <span class="pill pill-ok" title="Verified nodes">
        <Icon name="check" size={11} />
        {pills.verified}
      </span>
      {#if pills.issues > 0}
        <span class="pill pill-issues" title="Issues">
          <Icon name="warn" size={11} />
          {pills.issues}
        </span>
      {/if}
    </div>
  {/if}

  <button
    type="button"
    class="new-btn"
    data-testid="new-btn"
    on:click={openQuickCreate}
    aria-label="New (Quick Create)"
    title="Create new node (Cmd/Ctrl+K)"
  >
    <Icon name="plus" size={11} />
    New
  </button>

  <button
    type="button"
    class="icon-btn"
    class:active={$tweaksPanelOpen}
    aria-pressed={$tweaksPanelOpen}
    on:click={toggleTweaks}
    aria-label="Tweaks"
    title="Tweaks"
  >
    <Icon name="tweaks" size={14} />
  </button>
</header>

<style>
  .brand {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-right: 8px;
    flex-shrink: 0;
  }

  .brand-name {
    font-weight: 700;
    font-size: 13px;
    color: var(--ink);
    letter-spacing: 0.04em;
  }

  .brand-badge {
    font-size: 9px;
    font-weight: 600;
    background: var(--accent);
    color: var(--surface);
    border-radius: var(--radius-sm);
    padding: 1px 4px;
    letter-spacing: 0.02em;
  }

  .breadcrumbs {
    display: flex;
    align-items: center;
    gap: 2px;
    overflow: hidden;
  }

  .crumb-sep {
    color: var(--ink-3);
    display: flex;
    align-items: center;
  }

  .crumb {
    background: none;
    border: none;
    color: var(--ink-2);
    font-size: 12px;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: var(--radius-sm);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 120px;
  }

  .crumb:last-child {
    color: var(--ink);
  }

  .crumb:hover {
    background: var(--surface-3);
    color: var(--ink);
  }

  .lens-group {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-left: 8px;
    background: var(--surface-3);
    border-radius: var(--radius-sm);
    padding: 2px;
    flex-shrink: 0;
  }

  .lens-pill {
    background: transparent;
    border: none;
    color: var(--ink-3);
    font-size: 11px;
    text-transform: capitalize;
    cursor: pointer;
    padding: 3px 10px;
    border-radius: var(--radius-sm);
  }

  .lens-pill:hover {
    color: var(--ink);
  }

  .lens-pill[aria-pressed="true"] {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    color: var(--accent);
  }

  .spacer {
    flex: 1;
    min-width: 8px;
  }

  .pills {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-right: 8px;
  }

  .pill {
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    padding: 2px 6px;
    border-radius: 10px;
  }

  .pill-ok {
    background: color-mix(in srgb, var(--ok) 15%, transparent);
    color: var(--ok);
  }

  .pill-issues {
    background: color-mix(in srgb, var(--warn) 15%, transparent);
    color: var(--warn);
  }

  .new-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding: 0 10px;
    margin-right: 4px;
    background: var(--accent);
    color: var(--surface);
    border: none;
    border-radius: var(--radius-sm);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
  }

  .new-btn:hover { opacity: 0.88; }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-3);
    cursor: pointer;
    flex-shrink: 0;
  }

  .icon-btn:hover {
    background: var(--surface-3);
    color: var(--ink);
  }

  .icon-btn.active {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    color: var(--accent);
  }
</style>
