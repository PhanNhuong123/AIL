<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  const dispatch = createEventDispatcher<{ zoomin: void; zoomout: void; zoomreset: void }>();
</script>

<div class="zoom-controls" data-testid="flowchart-zoom-controls">
  <button type="button" class="zoom-btn" data-testid="zoom-in-btn"
          on:click={() => dispatch('zoomin')} aria-label="Zoom in">+</button>
  <button type="button" class="zoom-btn" data-testid="zoom-out-btn"
          on:click={() => dispatch('zoomout')} aria-label="Zoom out">−</button>
  <button type="button" class="zoom-btn" data-testid="zoom-reset-btn"
          on:click={() => dispatch('zoomreset')} aria-label="Reset zoom">1:1</button>
</div>

<style>
  .zoom-controls {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    gap: 4px;
    z-index: 2;
  }

  .zoom-btn {
    height: 24px;
    min-width: 28px;
    padding: 0 10px;
    background: var(--surface-2);
    color: var(--ink-2);
    border: 1px solid var(--line-2);
    border-radius: var(--radius-sm);
    font-size: 12px;
    cursor: pointer;
  }

  .zoom-btn:hover { background: var(--surface-3); color: var(--ink); }
  .zoom-btn:active { background: var(--surface-3); }
</style>
