<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { SuggestChip } from './chat-state';

  export let chips = [] as SuggestChip[];

  const dispatch = createEventDispatcher<{
    chipclick: { id: string; text: string };
  }>();
</script>

<div
  class="cp-chips"
  data-testid="chat-suggest-chips"
  role="group"
  aria-label="Suggested prompts"
>
  {#each chips as chip, i (chip.id)}
    <button
      type="button"
      class="cp-chip"
      data-testid="chat-suggest-chip-{i}"
      on:click={() => dispatch('chipclick', { id: chip.id, text: chip.text })}
    >{chip.text}</button>
  {/each}
</div>
