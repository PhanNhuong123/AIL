<script lang="ts">
  import { activeLens, path } from '$lib/stores';
  import { get } from 'svelte/store';
  import { navigateTo } from '$lib/chrome/toolbar-state';
  import {
    SG_VIEWBOX,
    computeModulePositions,
    filterRelations,
    badgeFor,
  } from './system-graph-layout';
  import type { GraphJson } from '$lib/types';

  export let graph = null as unknown as GraphJson;

  function legendFor(lens) {
    if (lens === 'verify') return 'Lens: Verify · badge = failing/warn fns';
    if (lens === 'rules') return 'Lens: Rules · async edges hidden';
    if (lens === 'data') return 'Lens: Data · only data edges';
    if (lens === 'tests') return 'Lens: Tests · badge = fn count';
    return 'Lens: Structure · badge = fn count';
  }

  $: layout = graph
    ? computeModulePositions(graph.modules ?? [], graph.clusters ?? [])
    : { modulePositions: [], clusterPositions: [] };
  $: edges = graph ? filterRelations(graph.relations ?? [], $activeLens) : [];
  $: moduleById = new Map((graph?.modules ?? []).map((m) => [m.id, m]));
  $: posById = new Map(layout.modulePositions.map((p) => [p.id, p]));

  let hoveredId = null as string | null;

  $: if (hoveredId !== null && !moduleById.has(hoveredId)) hoveredId = null;

  function handleClick(id) {
    const curPath = get(path);
    navigateTo([...curPath, id], 'module', id, 1);
  }
</script>

<div class="system-graph-wrap" data-testid="system-graph">
  <svg
    viewBox="0 0 {SG_VIEWBOX.w} {SG_VIEWBOX.h}"
    preserveAspectRatio="xMidYMid meet"
    class="system-graph-svg"
  >
    {#each layout.clusterPositions as cp (cp.id)}
      <circle
        class="sg-cluster-ring"
        data-testid="sg-cluster-{cp.id}"
        cx={cp.cx}
        cy={cp.cy}
        r={cp.r}
        style="stroke: {cp.color};"
      />
      <text class="sg-cluster-label" x={cp.cx} y={cp.cy - cp.r - 10} text-anchor="middle">{cp.label}</text>
    {/each}

    {#each edges as edge (edge.from + '→' + edge.to)}
      {@const a = posById.get(edge.from)}
      {@const b = posById.get(edge.to)}
      {#if a && b}
        <line
          class="sg-edge"
          class:sg-edge-dim={hoveredId !== null && edge.from !== hoveredId && edge.to !== hoveredId}
          x1={a.x}
          y1={a.y}
          x2={b.x}
          y2={b.y}
        />
      {/if}
    {/each}

    {#each layout.modulePositions as mp (mp.id)}
      {@const m = moduleById.get(mp.id)}
      {@const badge = badgeFor(m ?? { id: mp.id }, $activeLens)}
      <g
        data-testid="sg-module-{mp.id}"
        on:mouseenter={() => (hoveredId = mp.id)}
        on:mouseleave={() => (hoveredId = null)}
        on:click={() => handleClick(mp.id)}
        on:keydown={(e) => { if (e.key === 'Enter') handleClick(mp.id); }}
        role="button"
        tabindex="0"
      >
        <circle
          class="sg-module-circle"
          class:sg-module-dim={hoveredId !== null && hoveredId !== mp.id}
          cx={mp.x}
          cy={mp.y}
          r="18"
          data-tone={badge.tone}
        />
        <text class="sg-module-label" x={mp.x} y={mp.y + 32} text-anchor="middle">{m?.name ?? mp.id}</text>
        <g transform="translate({mp.x + 14}, {mp.y - 14})">
          <circle class="sg-badge-circle" r="9" />
          <text class="sg-badge-text" text-anchor="middle" dominant-baseline="middle" y="3">{badge.label}</text>
        </g>
      </g>
    {/each}

    <g class="sg-legend" data-testid="sg-legend" transform="translate(20, {SG_VIEWBOX.h - 30})">
      <text>{legendFor($activeLens)}</text>
    </g>
  </svg>
</div>
