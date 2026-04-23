<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { FlowNodeJson, Lens } from '$lib/types';
  import type { PillTone } from './lens';
  import { computeSwimNodeHint } from './lens';
  import { NODE_W, NODE_H, PAD } from './swim-layout';

  export let node      = null as unknown as FlowNodeJson;
  export let lens      = 'verify' as Lens;
  export let selected  = false;
  export let dimmed    = false;

  const dispatch = createEventDispatcher();

  $: hint = computeSwimNodeHint(node, lens);
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<g
  class="swim-node"
  class:swim-node-selected={selected}
  class:swim-node-dimmed={dimmed}
  data-tone={hint.tone}
  data-testid="swim-node-{node.id}"
  role="button"
  tabindex="0"
  on:click={() => dispatch('nodeclick')}
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
    y={node.y + PAD + NODE_H / 2 - 4}
    dominant-baseline="middle"
    text-anchor="middle"
    class="swim-node-label"
  >{node.label}</text>
  {#if hint.subLabel}
    <text
      x={node.x + PAD + NODE_W / 2}
      y={node.y + PAD + NODE_H / 2 + 12}
      dominant-baseline="middle"
      text-anchor="middle"
      class="swim-node-sublabel"
    >{hint.subLabel}</text>
  {/if}
</g>

<style>
  .swim-node-rect {
    fill: var(--surface-2);
    stroke: var(--ink-3);
    stroke-width: 1.5px;
    cursor: pointer;
  }
  .swim-node[data-tone="ok"]   .swim-node-rect { stroke: var(--ok);   }
  .swim-node[data-tone="fail"] .swim-node-rect { stroke: var(--fail); }
  .swim-node[data-tone="warn"] .swim-node-rect { stroke: var(--warn); }
  .swim-node[data-tone="muted"] .swim-node-rect { stroke: var(--ink-3); }

  .swim-node-selected .swim-node-rect {
    fill: color-mix(in srgb, var(--accent) 18%, var(--surface-2));
    stroke: var(--accent);
  }

  .swim-node-dimmed { opacity: 0.3; }

  .swim-node-label {
    font-size: 11px;
    fill: var(--ink);
    pointer-events: none;
  }
  .swim-node-sublabel {
    font-size: 9px;
    fill: var(--ink-3);
    pointer-events: none;
  }
</style>
