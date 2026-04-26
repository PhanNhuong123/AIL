<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import Icon from '$lib/icons/Icon.svelte';
  import ModuleCard from './ModuleCard.svelte';
  import { patchEffects } from '$lib/patch-effects';
  import type { ClusterJson, ModuleJson } from '$lib/types';
  import type { ClusterCounts } from './stage-state';

  export let cluster = null as unknown as ClusterJson;
  export let modules = [] as ModuleJson[];
  export let collapsed = false;
  export let counts = { total: 0, failing: 0, warn: 0 } as ClusterCounts;

  const dispatch = createEventDispatcher();

  $: addedIds    = $patchEffects.addedIds;
  $: modifiedIds = $patchEffects.modifiedIds;
  $: removedIds  = $patchEffects.removedIds;

  function patchStateFor(id) {
    if (addedIds.includes(id)) return 'added';
    if (modifiedIds.includes(id)) return 'modified';
    if (removedIds.includes(id)) return 'removed';
    return undefined;
  }

  function handleToggle() {
    dispatch('toggle', { id: cluster.id });
  }

  function handleKey(e) {
    const ev = e as KeyboardEvent;
    if (ev.key === 'Enter' || ev.key === ' ') {
      ev.preventDefault();
      handleToggle();
    }
  }
</script>

<section class="cluster-group" data-testid="cluster-group-{cluster.id}">
  <button
    class="cluster-header"
    data-testid="cluster-header-{cluster.id}"
    type="button"
    aria-expanded={!collapsed}
    on:click={handleToggle}
    on:keydown={handleKey}
  >
    <span
      class="cluster-collapse-icon"
      data-testid="cluster-collapse-icon-{cluster.id}"
      aria-hidden="true"
    >
      <Icon name={collapsed ? 'chevron-right' : 'chevron-down'} size={12} />
    </span>
    <span class="cluster-header-folder" aria-hidden="true">
      <Icon name="folder" size={12} />
    </span>
    <span class="cluster-header-name">{cluster.name}</span>
    <span
      class="cluster-header-count pill pill-muted"
      data-testid="cluster-header-count-modules-{cluster.id}"
    >{counts.total}</span>
    {#if counts.failing > 0}
      <span
        class="cluster-header-count pill pill-fail"
        data-testid="cluster-header-count-failing-{cluster.id}"
      >{counts.failing} failing</span>
    {:else}
      <span
        class="cluster-header-count pill pill-muted hidden-count"
        data-testid="cluster-header-count-failing-{cluster.id}"
      >0</span>
    {/if}
    {#if counts.warn > 0}
      <span
        class="cluster-header-count pill pill-warn"
        data-testid="cluster-header-count-warn-{cluster.id}"
      >{counts.warn} warn</span>
    {:else}
      <span
        class="cluster-header-count pill pill-muted hidden-count"
        data-testid="cluster-header-count-warn-{cluster.id}"
      >0</span>
    {/if}
  </button>

  {#if !collapsed}
    <div class="cluster-grid" data-testid="cluster-grid-{cluster.id}">
      {#each modules as mod (mod.id)}
        <div data-patch-state={patchStateFor(mod.id)}>
          <ModuleCard module={mod} />
        </div>
      {/each}
    </div>
  {/if}
</section>
