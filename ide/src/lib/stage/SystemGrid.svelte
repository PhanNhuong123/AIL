<script lang="ts">
  import ModuleCard from './ModuleCard.svelte';
  import type { GraphJson } from '$lib/types';

  export let graph = null as unknown as GraphJson;
</script>

<div class="system-grid-wrap" data-testid="system-grid">
  <div class="system-grid">
    {#each graph.modules as mod (mod.id)}
      <ModuleCard module={mod} />
    {/each}
  </div>
  {#if graph.externals.length > 0}
    <section
      class="system-grid-externals"
      data-testid="system-grid-externals"
    >
      <header class="system-grid-externals-header">Externals</header>
      <ul class="system-grid-externals-list">
        {#each graph.externals as ext (ext.id)}
          <li class="system-grid-external" data-testid="system-grid-external-{ext.id}">
            <span class="system-grid-external-name">{ext.name}</span>
            {#if ext.description}
              <span class="system-grid-external-desc">{ext.description}</span>
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>
