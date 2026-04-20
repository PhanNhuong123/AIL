<script lang="ts">
  import ClusterGroup from './ClusterGroup.svelte';
  import { clusterCollapsed, toggleCluster, groupByCluster } from './stage-state';
  import type { GraphJson } from '$lib/types';

  export let graph = null as unknown as GraphJson;

  $: groups = groupByCluster(graph);

  function onToggle(event) {
    const e = event as CustomEvent<{ id: string }>;
    toggleCluster(e.detail.id);
  }
</script>

<div class="system-clusters" data-testid="system-clusters">
  {#each groups as g (g.cluster.id)}
    <ClusterGroup
      cluster={g.cluster}
      modules={g.modules}
      collapsed={$clusterCollapsed.has(g.cluster.id)}
      counts={g.counts}
      on:toggle={onToggle}
    />
  {/each}
</div>
