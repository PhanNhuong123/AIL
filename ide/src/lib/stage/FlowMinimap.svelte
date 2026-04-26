<script lang="ts">
  import { get } from 'svelte/store';
  import { path } from '$lib/stores';
  import { navigateTo } from '$lib/chrome/toolbar-state';
  import type { FlowchartJson, FlowNodeJson } from '$lib/types';

  // Presentational: takes data + selection as props (15.9-B precedent).
  // No store imports for selection or flow state.
  export let flowchart = null as FlowchartJson | null;
  export let selectedId = null as string | null;

  function handleDotClick(node) {
    const curPath = get(path);
    navigateTo([...curPath, `step:${node.id}`], 'step', `step:${node.id}`, 4);
  }
</script>

<div class="minimap" data-testid="flow-minimap">
  {#if flowchart}
    {#each flowchart.nodes as node (node.id)}
      <button
        type="button"
        class="mm-dot"
        class:mm-dot-selected={node.id === selectedId}
        data-testid="mm-dot-{node.id}"
        aria-label={`Navigate to ${node.label ?? node.id}`}
        on:click={() => handleDotClick(node)}
      ></button>
    {/each}
  {/if}
</div>
