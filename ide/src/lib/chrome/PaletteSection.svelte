<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import NavigatorSection from './NavigatorSection.svelte';

  const dispatch = createEventDispatcher();

  const chips = [
    { kind: 'sequence', label: 'Sequence', suffix: '→' },
    { kind: 'async',    label: 'Async',    suffix: '⇢' },
    { kind: 'rule',     label: 'Rule',     suffix: '⚡' },
  ];

  function handleDragStart(e, kind) {
    if (e.dataTransfer) {
      e.dataTransfer.setData('application/x-ail-palette', kind);
    }
    dispatch('createdrag', { kind });
  }
</script>

<NavigatorSection title="PALETTE">
  <div class="palette-chips">
    {#each chips as chip}
      <button
        class="chip"
        draggable="true"
        on:dragstart={(e) => handleDragStart(e, chip.kind)}
        aria-label="Drag {chip.label} node"
      >
        <span class="chip-label">{chip.label}</span>
        <span class="chip-suffix">{chip.suffix}</span>
      </button>
    {/each}
  </div>
</NavigatorSection>

<style>
  .palette-chips {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px 8px;
  }

  .chip {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 8px;
    background: var(--surface-3);
    border: 1px solid var(--line-2);
    border-radius: var(--radius-sm);
    color: var(--ink-2);
    font-size: 12px;
    cursor: grab;
    text-align: left;
  }

  .chip:hover {
    background: var(--surface);
    color: var(--ink);
    border-color: var(--accent);
  }

  .chip-suffix {
    color: var(--ink-3);
    font-size: 11px;
  }
</style>
