<script lang="ts">
  import { graph, selection } from '$lib/stores';
  import { zoomLevel } from '$lib/chrome/toolbar-state';
  import StageTabStrip from './StageTabStrip.svelte';
  import SystemView from './SystemView.svelte';
  import ModuleView from './ModuleView.svelte';
  import type { ModuleJson } from '$lib/types';

  function findModule(g, id) {
    const graphVal = g as import('$lib/types').GraphJson | null;
    const selId = id as string | null;
    if (!graphVal || !selId) return null;
    for (const m of graphVal.modules) {
      if (m.id === selId) return m as ModuleJson;
    }
    return null;
  }

  $: activeModule = findModule($graph, $selection.id);
</script>

<div class="stage-root" data-testid="stage-root">
  <StageTabStrip />
  <div class="stage-body">
    {#if !$graph}
      <div class="stage-placeholder" data-testid="stage-empty">
        No project loaded.
      </div>
    {:else if $zoomLevel === 0}
      <SystemView graph={$graph} />
    {:else if $zoomLevel === 1 && activeModule}
      <ModuleView module={activeModule} />
    {:else if $zoomLevel === 1}
      <div class="stage-placeholder" data-testid="stage-module-missing">
        Module not found.
      </div>
    {:else if $zoomLevel === 2}
      <div class="stage-placeholder" data-testid="stage-flow-placeholder">
        Flow view — 16.6.
      </div>
    {:else}
      <div class="stage-placeholder" data-testid="stage-node-placeholder">
        Node view — 16.6.
      </div>
    {/if}
  </div>
</div>
