<script lang="ts">
  import { graph, selection } from '$lib/stores';
  import { zoomLevel, pickerOpen, pickerItems, navigateTo, zoomIn, zoomOut } from './toolbar-state';
  import type { PickerItem } from './toolbar-state';
  import Icon from '$lib/icons/Icon.svelte';

  function getLevelName(lv) {
    const n = lv as number;
    if (n === 0) return 'System';
    if (n === 1) return 'Module';
    if (n === 2) return 'Workflow';
    return 'Detail';
  }

  $: label = getLevelName($zoomLevel);

  // Zoom-out disabled when at System level
  $: disabledOut = $zoomLevel === 0;

  // Zoom-in disabled when at Detail level, OR at Workflow level with no steps
  $: disabledIn = (() => {
    if ($zoomLevel === 4) return true;
    if ($zoomLevel === 2) {
      const g = $graph;
      const sel = $selection;
      if (!g || !sel.id) return true;
      for (const mod of g.modules) {
        const fn_ = mod.functions.find(f => f.id === sel.id);
        if (fn_) return (fn_.steps ?? []).length === 0;
      }
      return true;
    }
    return false;
  })();

  function handlePickerSelect(item) {
    const it = item as PickerItem;
    pickerOpen.set(false);
    const lvl = it.kind === 'module' ? 1 : it.kind === 'function' ? 2 : 4;
    navigateTo(it.newPath, it.kind, it.id, lvl);
  }
</script>

<div class="zoom-pill">
  <button
    class="toolbar-btn"
    data-testid="toolbar-zoom-out"
    aria-label="Zoom out"
    title="Zoom out"
    disabled={disabledOut}
    on:click={zoomOut}
  >
    <Icon name="minus" size={13} />
  </button>

  <span class="zoom-label" data-testid="toolbar-zoom-label">{label}</span>

  <div class="zoom-in-wrap">
    <button
      class="toolbar-btn"
      data-testid="toolbar-zoom-in"
      aria-label="Zoom in"
      title="Zoom in"
      disabled={disabledIn}
      on:click={zoomIn}
    >
      <Icon name="plus" size={13} />
    </button>

    {#if $pickerOpen}
      <div class="zoom-picker" data-testid="toolbar-zoom-picker">
        {#each $pickerItems as item}
          <button
            class="picker-item"
            data-testid="toolbar-picker-item-{item.id}"
            on:click={() => handlePickerSelect(item)}
          >
            {item.name}
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .zoom-pill {
    display: flex;
    align-items: center;
    gap: 2px;
    border: 1px solid var(--line-2);
    border-radius: var(--radius-sm);
    padding: 0 2px;
  }

  .zoom-in-wrap {
    position: relative;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 26px;
    padding: 0 6px;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-3);
    cursor: pointer;
  }

  .toolbar-btn:hover:not(:disabled) {
    background: var(--surface-3);
    color: var(--ink);
  }

  .toolbar-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .zoom-label {
    font-size: 11px;
    color: var(--ink-2);
    padding: 0 4px;
    white-space: nowrap;
    min-width: 52px;
    text-align: center;
    user-select: none;
  }

  .zoom-picker {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    min-width: 120px;
    z-index: 100;
    display: flex;
    flex-direction: column;
    padding: 4px 0;
    box-shadow: var(--shadow-2);
  }

  .picker-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 6px 12px;
    background: none;
    border: none;
    color: var(--ink-2);
    font-size: 12px;
    cursor: pointer;
  }

  .picker-item:hover {
    background: var(--surface-3);
    color: var(--ink);
  }
</style>
