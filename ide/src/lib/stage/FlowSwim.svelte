<script lang="ts">
  import type { FlowchartJson, FlowEdgeJson, FunctionJson } from '$lib/types';
  import { flowSelectedNodeId, flowFocusedNodeId } from './flow-state';
  import { navigateTo } from '$lib/chrome/toolbar-state';
  import { path, activeLens } from '$lib/stores';
  import { get } from 'svelte/store';
  import SwimNode from './SwimNode.svelte';
  import { NODE_W, NODE_H, PAD } from './swim-layout';

  export let flowchart = { nodes: [], edges: [] } as FlowchartJson;
  export let fn = null as FunctionJson | null;

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

  function isNeighbor(nodeId, focusedId, edges) {
    return (edges as FlowEdgeJson[]).some(
      (e) => (e.from === focusedId && e.to === nodeId) || (e.to === focusedId && e.from === nodeId),
    );
  }
</script>

<div class="flow-swim" data-testid="flow-swim" aria-label={fn ? 'Flow for ' + fn.name : undefined}>
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
      <SwimNode
        {node}
        lens={$activeLens}
        selected={$flowSelectedNodeId === node.id}
        dimmed={$flowFocusedNodeId !== null && $flowFocusedNodeId !== node.id && !isNeighbor(node.id, $flowFocusedNodeId, flowchart.edges)}
        on:nodeclick={() => { flowSelectedNodeId.set(node.id); handleNodeClick(node); }}
      />
    {/each}

    <!-- Edges (simple straight lines in swim view) -->
    <!-- Phase 17: edge animation deferred from 16.2 — GraphPatchJson has no edge arrays; flowchart edges depend on Phase 17 sheaf-aware projection. -->
    {#each flowchart.edges as edge (edge.from + '->' + edge.to)}
      {@const fromN = flowchart.nodes.find((n) => n.id === edge.from)}
      {@const toN   = flowchart.nodes.find((n) => n.id === edge.to)}
      {#if fromN && toN}
        {@const midX = (fromN.x + PAD + NODE_W / 2 + toN.x + PAD + NODE_W / 2) / 2}
        {@const midY = (fromN.y + PAD + NODE_H + toN.y + PAD) / 2}
        <line
          x1={fromN.x + PAD + NODE_W / 2}
          y1={fromN.y + PAD + NODE_H}
          x2={toN.x   + PAD + NODE_W / 2}
          y2={toN.y   + PAD}
          class="swim-edge"
          data-testid={edge.label ? `swim-branch-edge-${edge.label}` : undefined}
        />
        {#if edge.label}
          <text
            x={midX}
            y={midY}
            dominant-baseline="middle"
            text-anchor="middle"
            class="swim-edge-label"
          >{edge.label}</text>
        {/if}
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

  .swim-edge {
    stroke: var(--ink-3);
    stroke-width: 1.5px;
  }

  .swim-edge-label {
    font-size: 10px;
    fill: var(--ink-3);
    pointer-events: none;
  }
</style>
