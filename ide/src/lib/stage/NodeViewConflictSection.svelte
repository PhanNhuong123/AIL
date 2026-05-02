<script lang="ts">
  import type { SheafConflictEntry } from '$lib/types';

  export let conflicts = [] as SheafConflictEntry[];
  export let currentNodeId = '';
  export let onJump = (() => {}) as (peerId: string) => void;

  $: peers = conflicts.map((c) => {
    const isANow = c.nodeA === currentNodeId;
    return {
      peerId: isANow ? c.nodeB : c.nodeA,
      myConstraints: isANow ? c.conflictingA : c.conflictingB,
      peerConstraints: isANow ? c.conflictingB : c.conflictingA,
      overlapIndex: c.overlapIndex,
    };
  });
</script>

{#if conflicts.length > 0}
<section class="conflict-section" data-testid="node-view-conflict-section">
  <header class="conflict-header">Localized contradictions</header>
  {#each peers as p (p.overlapIndex)}
    <article class="conflict-card" data-testid="conflict-card-{p.overlapIndex}">
      <div class="conflict-side">
        <span class="conflict-side-label">This step:</span>
        <ul>{#each p.myConstraints as c}<li>{c}</li>{/each}</ul>
      </div>
      <div class="conflict-side">
        <span class="conflict-side-label">Conflicts with:</span>
        <ul>{#each p.peerConstraints as c}<li>{c}</li>{/each}</ul>
      </div>
      <button
        type="button"
        class="conflict-jump-btn"
        data-testid="conflict-jump-{p.peerId}"
        aria-label={'Jump to peer step ' + p.peerId}
        on:click={() => onJump(p.peerId)}
      >Jump to peer →</button>
    </article>
  {/each}
</section>
{/if}

<style>
  .conflict-section {
    border-left: 3px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 8%, transparent);
    padding: 12px;
    margin-top: 12px;
    border-radius: 4px;
  }
  .conflict-header {
    font-weight: 600;
    margin-bottom: 8px;
  }
  .conflict-card {
    border: 1px solid color-mix(in srgb, var(--warn) 30%, transparent);
    border-radius: 4px;
    padding: 8px;
    margin-bottom: 8px;
  }
  .conflict-side { margin-bottom: 6px; }
  .conflict-side-label {
    font-size: 11px;
    color: var(--ink-3);
    text-transform: uppercase;
  }
  .conflict-side ul { margin: 4px 0 0 16px; padding: 0; }
  .conflict-jump-btn {
    background: var(--warn);
    color: var(--surface);
    border: none;
    border-radius: 3px;
    padding: 4px 10px;
    font-size: 12px;
    cursor: pointer;
  }
  .conflict-jump-btn:hover {
    background: color-mix(in srgb, var(--warn) 80%, white);
  }
</style>
