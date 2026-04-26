<script lang="ts">
  import { activeLens } from '$lib/stores';
  import { systemMode } from './stage-state';
  import SystemClusters from './SystemClusters.svelte';
  import SystemGrid from './SystemGrid.svelte';
  import SystemGraph from './SystemGraph.svelte';
  import { computeSystemHeadSummary } from './lens';
  import type { GraphJson } from '$lib/types';
  import type { SystemMode } from './stage-state';

  export let graph = null as unknown as GraphJson;

  const MODES = ['Clusters', 'Grid', 'Graph'] as const;

  function setMode(m) {
    systemMode.set(m as SystemMode);
  }

  $: headSummary = graph ? computeSystemHeadSummary(graph, $activeLens) : null;
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
    {#if headSummary}
      <div class="head-actions" data-testid={headSummary.testid}>
        {#each headSummary.chips as chip}
          <span class="head-chip" data-tone={chip.tone}>{chip.label}</span>
        {/each}
      </div>
    {/if}
  </header>

  {#if $systemMode === 'Clusters'}
    <SystemClusters {graph} />
  {:else if $systemMode === 'Grid'}
    <SystemGrid {graph} />
  {:else}
    <SystemGraph {graph} />
  {/if}
</section>

<style>
  .head-actions { display: flex; gap: 8px; margin-left: auto; align-items: center; }
  .head-chip { font-size: 12px; color: var(--ink-3); background: var(--bg-2); border: 1px solid var(--border); border-radius: 999px; padding: 2px 8px; }
</style>
