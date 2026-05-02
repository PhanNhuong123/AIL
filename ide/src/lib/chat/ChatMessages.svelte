<script lang="ts">
  import { tick } from 'svelte';
  import type { ChatMessage, PreviewCardModel } from './chat-state';
  import ChatPreviewCard from './ChatPreviewCard.svelte';

  export let messages = [] as ChatMessage[];
  export let previews = [] as PreviewCardModel[];

  let listEl = undefined as HTMLElement | undefined;

  // Auto-scroll the message log to the bottom whenever new content
  // arrives. Without this the agent can stream a long response and the
  // newest line stays below the fold while the user is left looking at
  // the seed message. Awaits `tick` so the new node is in the DOM before
  // we read scrollHeight.
  $: void scrollToBottom(messages.length, previews.length);
  async function scrollToBottom(_msgsLen, _previewsLen) {
    await tick();
    if (listEl) listEl.scrollTop = listEl.scrollHeight;
  }
</script>

<div
  class="cp-msgs"
  data-testid="chat-messages"
  role="log"
  aria-live="polite"
  aria-label="Chat messages"
  bind:this={listEl}
>
  {#each messages as m, i (m.id)}
    <div class="cp-msg cp-msg-{m.role}" data-testid="chat-message-{i}">
      <span class="cp-msg-role">{m.role === 'user' ? 'You' : 'AIL Assistant'}</span>
      <span class="cp-msg-text">{m.text}</span>
    </div>
  {/each}
  {#each previews as p (p.id)}
    <ChatPreviewCard card={p} on:confirm on:adjust on:discard />
  {/each}
</div>

<style>
  /* Long agent messages span multiple paragraphs / contain newlines —
   * preserve them so the assistant's structure isn't collapsed onto a
   * single visual line. */
  .cp-msg-text {
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
