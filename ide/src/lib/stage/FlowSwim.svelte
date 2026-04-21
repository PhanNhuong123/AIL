<script lang="ts">
  import type { FlowchartJson } from '$lib/types';
  import { flowSelectedNodeId } from './flow-state';
  import { navigateTo } from '$lib/chrome/toolbar-state';
  import { path } from '$lib/stores';
  import { get } from 'svelte/store';

  export let flowchart = { nodes: [], edges: [] } as FlowchartJson;

  const NODE_W = 120;
  const NODE_H = 48;
  const PAD    = 24;

  // Collect unique Y lanes (swim lanes by Y coordinate)
  $: lanes = buildLanes(flowchart.nodes);

  function buildLanes(nodes) {
    const map = new Map();
    for (const n of nodes) {
      const bucket = map.get(n.y) ?? [];
      bucket.push(n);
      map.set(n.y, bucket);
    }
    return [...map.entries()]
      .sort(([a], [b]) => a - b)
      .map(([y, ns]) => ({ y, nodes: ns.sort((a, b) => a.x - b.x) }));
  }

  function handleNodeClick(node) {
    const curPath = get(path);
    navigateTo(
      [...curPath, `step:${node.id}`],
      'step',
      `step:${node.id}`,
      4,
    );
  }

  $: selectedId = $flowSelectedNodeId;
</script>

<div class="flow-swim" data-testid="flow-swim">
  <svg
    width="100%"
    height={lanes.length > 0 ? lanes[lanes.length - 1].y + NODE_H + PAD * 2 : 200}
    class="swim-svg"
  >
    <!-- Horizontal lane dividers -->
    {#each lanes as lane, i}
      {#if i > 0}
        <line
          x1={0} y1={lane.y - PAD / 2 + PAD}
          x2="100%" y2={lane.y - PAD / 2 + PAD}
          class="swim-lane-line"
        />
      {/if}
    {/each}

    <!-- Nodes -->
    {#each flowchart.nodes as node (node.id)}
      <!-- svelte-ignore a11y-click-events-have-key-events -->
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <g
        class="swim-node"
        class:swim-node-selected={selectedId === node.id}
        on:click={() => {
          flowSelectedNodeId.set(node.id);
          handleNodeClick(node);
        }}
        data-testid="swim-node-{node.id}"
        role="button"
        tabindex="0"
      >
        <rect
          x={node.x + PAD}
          y={node.y + PAD}
          width={NODE_W}
          height={NODE_H}
          rx="6"
          class="swim-node-rect"
        />
        <text
          x={node.x + PAD + NODE_W / 2}
          y={node.y + PAD + NODE_H / 2 + 1}
          dominant-baseline="middle"
          text-anchor="middle"
          class="swim-node-label"
        >{node.label}</text>
      </g>
    {/each}

    <!-- Edges (simple straight lines in swim view) -->
    {#each flowchart.edges as edge (edge.from + '->' + edge.to)}
      {@const fromN = flowchart.nodes.find((n) => n.id === edge.from)}
      {@const toN   = flowchart.nodes.find((n) => n.id === edge.to)}
      {#if fromN && toN}
        <line
          x1={fromN.x + PAD + NODE_W / 2}
          y1={fromN.y + PAD + NODE_H}
          x2={toN.x   + PAD + NODE_W / 2}
          y2={toN.y   + PAD}
          class="swim-edge"
        />
      {/if}
    {/each}
  </svg>
</div>

<style>
  .flow-swim {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--surface);
  }

  .swim-svg {
    display: block;
  }

  .swim-lane-line {
    stroke: var(--line);
    stroke-width: 1px;
    stroke-dasharray: 4 4;
  }

  .swim-node-rect {
    fill: var(--surface-2);
    stroke: var(--ink-3);
    stroke-width: 1.5px;
    cursor: pointer;
  }

  .swim-node-selected .swim-node-rect {
    fill: color-mix(in srgb, var(--accent) 18%, var(--surface-2));
    stroke: var(--accent);
  }

  .swim-node-label {
    font-size: 11px;
    fill: var(--ink);
    pointer-events: none;
  }

  .swim-edge {
    stroke: var(--ink-3);
    stroke-width: 1.5px;
  }
</style>
