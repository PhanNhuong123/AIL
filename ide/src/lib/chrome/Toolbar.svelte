<script lang="ts">
  import { graph, path, history, selection, paletteVisible } from '$lib/stores';
  import type { SelectionKind } from '$lib/stores';
  import { breadcrumbs } from './rollup';
  import { navigateTo, goBack, goForward, stageLevelForKind } from './toolbar-state';
  import ToolbarZoom from './ToolbarZoom.svelte';
  import ToolbarOverlays from './ToolbarOverlays.svelte';
  import Icon from '$lib/icons/Icon.svelte';

  $: crumbs = breadcrumbs($graph, $path);
  $: backDisabled  = $history.back.length === 0;
  $: fwdDisabled   = $history.forward.length === 0;

  function handleCrumbClick(i) {
    const idx = i as number;
    const newPath = $path.slice(0, idx + 1);
    const fullId = newPath[newPath.length - 1];
    const kind = fullId.slice(0, fullId.indexOf(':')) as SelectionKind;
    const level = stageLevelForKind(kind);
    navigateTo(newPath, kind, fullId, level);
  }

  function togglePalette() {
    paletteVisible.update(v => !v);
  }
</script>

<div class="region-toolbar">
  <!-- Back -->
  <button
    class="toolbar-btn icon-btn"
    data-testid="toolbar-back"
    aria-label="Back"
    title="Back"
    disabled={backDisabled}
    on:click={goBack}
  >
    <Icon name="chevron-left" size={13} />
  </button>

  <!-- Forward -->
  <button
    class="toolbar-btn icon-btn"
    data-testid="toolbar-forward"
    aria-label="Forward"
    title="Forward"
    disabled={fwdDisabled}
    on:click={goForward}
  >
    <Icon name="chevron-right" size={13} />
  </button>

  <span class="sep" aria-hidden="true"></span>

  <!-- Breadcrumbs -->
  {#if crumbs.length > 0}
    <nav class="breadcrumbs" aria-label="Toolbar breadcrumbs">
      {#each crumbs as crumb, i}
        {#if i > 0}
          <span class="crumb-sep" aria-hidden="true">
            <Icon name="chevron-right" size={10} />
          </span>
        {/if}
        <button
          class="crumb"
          data-testid="toolbar-crumb-{i}"
          aria-current={i === crumbs.length - 1 ? 'page' : undefined}
          on:click={() => handleCrumbClick(i)}
        >
          {crumb.name}
        </button>
      {/each}
    </nav>
  {/if}

  <div class="spacer"></div>

  <!-- Zoom pill -->
  <ToolbarZoom />

  <span class="sep" aria-hidden="true"></span>

  <!-- Overlay toggles -->
  <ToolbarOverlays />

  <span class="sep" aria-hidden="true"></span>

  <!-- Palette toggle -->
  <button
    class="toolbar-btn icon-btn"
    class:active={$paletteVisible}
    data-testid="toolbar-palette"
    aria-label="Palette"
    title="Palette"
    aria-pressed={$paletteVisible}
    on:click={togglePalette}
  >
    <Icon name="palette" size={13} />
  </button>
</div>

<style>
  .toolbar-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 26px;
    padding: 0 8px;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-3);
    cursor: pointer;
    flex-shrink: 0;
  }

  .toolbar-btn:hover:not(:disabled) {
    background: var(--surface-3);
    color: var(--ink);
  }

  .toolbar-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .toolbar-btn.active {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    color: var(--accent);
  }

  .icon-btn {
    width: 28px;
    padding: 0;
  }

  .sep {
    border-left: 1px solid var(--line-2);
    height: 16px;
    margin: 0 4px;
    flex-shrink: 0;
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

  .spacer {
    flex: 1;
    min-width: 8px;
  }
</style>
