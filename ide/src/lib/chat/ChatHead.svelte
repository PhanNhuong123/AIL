<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let subtext = '' as string;
  export let isRunning = false;

  const dispatch = createEventDispatcher();

  function onStop() {
    dispatch('stop');
  }
</script>

<div class="cp-head" data-testid="chat-head">
  <span class="cp-head-title">Assistant</span>
  <span class="cp-head-sub" data-testid="chat-context-subtext">{subtext}</span>
  {#if isRunning}
    <button
      type="button"
      class="cp-head-stop"
      data-testid="chat-stop-btn"
      on:click={onStop}
    >
      Stop
    </button>
  {/if}
</div>
