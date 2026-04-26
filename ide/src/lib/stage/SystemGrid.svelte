<script lang="ts">
  import { flip } from 'svelte/animate';
  import { fade } from 'svelte/transition';
  import ModuleCard from './ModuleCard.svelte';
  import { patchEffects } from '$lib/patch-effects';
  import type { GraphJson } from '$lib/types';

  export let graph = null as unknown as GraphJson;

  $: addedIds    = $patchEffects.addedIds;
  $: modifiedIds = $patchEffects.modifiedIds;
  $: removedIds  = $patchEffects.removedIds;

  function patchStateFor(id) {
    if (addedIds.includes(id)) return 'added';
    if (modifiedIds.includes(id)) return 'modified';
    if (removedIds.includes(id)) return 'removed';
    return undefined;
  }
</script>

<div class="system-grid-wrap" data-testid="system-grid">
  <div class="system-grid">
    {#each graph.modules as mod (mod.id)}
      <div animate:flip={{ duration: 300 }} out:fade={{ duration: 300 }} data-patch-state={patchStateFor(mod.id)}>
        <ModuleCard module={mod} />
      </div>
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
