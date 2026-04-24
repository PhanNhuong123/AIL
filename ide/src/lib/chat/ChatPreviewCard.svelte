<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { PreviewCardModel } from './chat-state';

  export let card = null as PreviewCardModel | null;

  const dispatch = createEventDispatcher<{
    confirm: { id: string };
    adjust:  { id: string };
    discard: { id: string };
  }>();
</script>

{#if card}
<div class="cp-preview" data-testid="chat-preview-card" data-preview-id={card.id}>
  <div class="cp-preview-title">{card.title}</div>
  <div class="cp-preview-summary">{card.summary}</div>
  <div class="cp-preview-actions">
    <button
      type="button"
      class="cp-preview-btn primary"
      data-testid="chat-preview-confirm"
      on:click={() => dispatch('confirm', { id: card.id })}
    >Confirm</button>
    <button
      type="button"
      class="cp-preview-btn"
      data-testid="chat-preview-adjust"
      on:click={() => dispatch('adjust', { id: card.id })}
    >Adjust</button>
    <button
      type="button"
      class="cp-preview-btn"
      data-testid="chat-preview-discard"
      on:click={() => dispatch('discard', { id: card.id })}
    >Discard</button>
  </div>
</div>
{/if}
