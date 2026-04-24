<script lang="ts">
  import type { Lens, NodeDetail } from '$lib/types';
  import { computeNodeDetailSummary } from './lens';
  import { hueTokenFor } from './lens-banner-copy';

  export let lens = 'verify' as Lens;
  export let detail = null as NodeDetail | null;

  $: summary = computeNodeDetailSummary(detail, lens);
  $: hueVar = 'var(' + hueTokenFor(lens) + ')';
</script>

<div
  class="node-detail-lens-section tone-{summary.tone}"
  data-testid="node-detail-lens-section"
  style="--lens-hue: {hueVar}"
>
  <div
    class="node-detail-lens-container"
    data-testid="node-detail-lens-{lens}"
  >
    {#if summary.heading.length > 0}
      <span
        class="node-detail-lens-heading"
        data-testid="node-detail-lens-heading"
      >{summary.heading}</span>
    {/if}
    {#if summary.items.length > 0}
      <ul class="node-detail-lens-items">
        {#each summary.items as item, i}
          <li
            class="node-detail-lens-item"
            data-testid="node-detail-lens-item-{i}"
          >{item}</li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .node-detail-lens-section {
    margin-top: 10px;
    padding: 8px 10px;
    border-left: 3px solid var(--lens-hue);
    background: var(--surface-2);
    border-radius: var(--radius-sm);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .node-detail-lens-heading {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--lens-hue);
    font-weight: 600;
  }

  .node-detail-lens-items {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .node-detail-lens-item {
    font-size: 11px;
    color: var(--ink-2);
    line-height: 1.4;
    word-break: break-word;
  }
</style>
