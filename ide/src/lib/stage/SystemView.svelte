<script lang="ts">
  import { systemMode } from './stage-state';
  import SystemClusters from './SystemClusters.svelte';
  import SystemGrid from './SystemGrid.svelte';
  import SystemGraphPlaceholder from './SystemGraphPlaceholder.svelte';
  import type { GraphJson } from '$lib/types';
  import type { SystemMode } from './stage-state';

  export let graph = null as unknown as GraphJson;

  const MODES = ['Clusters', 'Grid', 'Graph'] as const;

  function setMode(m) {
    systemMode.set(m as SystemMode);
  }
</script>

<section class="system-view" data-testid="system-view">
  <header class="system-header">
    {#each MODES as m}
      <button
        class="system-mode-btn"
        class:active={$systemMode === m}
        data-testid="system-mode-btn-{m.toLowerCase()}"
        type="button"
        aria-pressed={$systemMode === m}
        on:click={() => setMode(m)}
      >{m}</button>
    {/each}
  </header>

  {#if $systemMode === 'Clusters'}
    <SystemClusters {graph} />
  {:else if $systemMode === 'Grid'}
    <SystemGrid {graph} />
  {:else}
    <SystemGraphPlaceholder />
  {/if}
</section>
