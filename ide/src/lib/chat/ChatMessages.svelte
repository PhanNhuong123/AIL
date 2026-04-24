<script lang="ts">
  import type { ChatMessage, PreviewCardModel } from './chat-state';
  import ChatPreviewCard from './ChatPreviewCard.svelte';

  export let messages = [] as ChatMessage[];
  export let previews = [] as PreviewCardModel[];
</script>

<div
  class="cp-msgs"
  data-testid="chat-messages"
  role="log"
  aria-live="polite"
  aria-label="Chat messages"
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
