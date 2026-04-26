<script lang="ts">
  import type { FlowEdgeJson, FlowNodeJson } from '$lib/types';
  import { NODE_W as W, NODE_H as H } from './swim-layout';

  export let edge     = {} as FlowEdgeJson;
  export let fromNode = undefined as FlowNodeJson | undefined;
  export let toNode   = undefined as FlowNodeJson | undefined;
  export let fromPos  = undefined as { x: number; y: number } | undefined;
  export let toPos    = undefined as { x: number; y: number } | undefined;

  $: fx = (fromPos?.x ?? fromNode?.x ?? 0) + W / 2;
  $: fy = (fromPos?.y ?? fromNode?.y ?? 0) + H;
  $: tx = (toPos?.x ?? toNode?.x ?? 0) + W / 2;
  $: ty = (toPos?.y ?? toNode?.y ?? 0);

  $: cpY = (fy + ty) / 2;
  $: d = `M ${fx} ${fy} C ${fx} ${cpY}, ${tx} ${cpY}, ${tx} ${ty}`;

  $: colorClass = edge.style === 'ok'  || edge.label === 'yes' ? 'flowchart-edge-ok'
                : edge.style === 'err' || edge.label === 'no'  ? 'flowchart-edge-err'
                : 'flowchart-edge-neutral';

  $: markerId = colorClass === 'flowchart-edge-ok'  ? 'url(#arrow-ok)'
              : colorClass === 'flowchart-edge-err'  ? 'url(#arrow-err)'
              : 'url(#arrow-neutral)';

  $: midX = (fx + tx) / 2;
  $: midY = (fy + ty) / 2;
</script>

<!-- Phase 17: edge animation deferred from 16.2 — GraphPatchJson has no edge arrays; flowchart edges depend on Phase 17 sheaf-aware projection. -->
<g class="flowchart-edge {colorClass}" data-testid="flowchart-edge-{edge.from}-{edge.to}">
  <path {d} marker-end={markerId} class="edge-path"/>
  {#if edge.label}
    <text x={midX} y={midY - 4} text-anchor="middle" class="edge-label"
          data-testid="edge-label-{edge.from}-{edge.to}">{edge.label}</text>
  {/if}
</g>

<style>
  .edge-path {
    fill: none;
    stroke-width: 1.5px;
  }

  .flowchart-edge-ok .edge-path      { stroke: var(--ok); }
  .flowchart-edge-err .edge-path     { stroke: var(--fail); }
  .flowchart-edge-neutral .edge-path { stroke: var(--ink-3); }

  .flowchart-edge-ok .edge-label      { fill: var(--ok); }
  .flowchart-edge-err .edge-label     { fill: var(--fail); }
  .flowchart-edge-neutral .edge-label { fill: var(--ink-3); }

  .edge-label {
    font-size: 10px;
    pointer-events: none;
  }
</style>
