<script lang="ts">
  import { get } from 'svelte/store';
  import { selection, path, activeLens } from '$lib/stores';
  import {
    chatMode, chatDraft, chatMessages, chatPreviewCards,
    contextSubtextFor, suggestChipsFor, placeholderFor,
    type ChatMode,
  } from './chat-state';
  import ChatHead from './ChatHead.svelte';
  import ChatMessages from './ChatMessages.svelte';
  import ChatSuggestChips from './ChatSuggestChips.svelte';
  import ChatInputRow from './ChatInputRow.svelte';

  // Monotonic per-session id counter so rapid same-ms sends don't collide in
  // Svelte's keyed {#each m.id} list (Date.now() alone is not unique enough).
  let msgSeq = 0;

  $: subtext = contextSubtextFor($selection, $path);
  $: chips = suggestChipsFor($selection, $activeLens);
  $: placeholder = placeholderFor($chatMode);

  function handleSend(ev) {
    const e = ev as CustomEvent<{ text: string }>;
    const text = (e.detail?.text ?? '').trim();
    if (!text) return;

    const ctx = {
      selection: get(selection),
      path: get(path),
      lens: get(activeLens),
      mode: get(chatMode),
      text,
    };
    console.info('[phase-16-stub] chat:send', ctx);

    const uid = `u-${++msgSeq}`;
    const aid = `a-${++msgSeq}`;
    chatMessages.update((arr) => [
      ...arr,
      { id: uid, role: 'user', text },
      { id: aid, role: 'assistant', text: '(preview response will arrive in Phase 16)' },
    ]);
    chatDraft.set('');
  }

  function handleModeChange(ev) {
    const e = ev as CustomEvent<{ mode: ChatMode }>;
    chatMode.set(e.detail.mode);
  }

  function handleDraftChange(ev) {
    const e = ev as CustomEvent<{ value: string }>;
    chatDraft.set(e.detail.value);
  }

  function handleChipClick(ev) {
    const e = ev as CustomEvent<{ id: string; text: string }>;
    console.info('[phase-16-stub] chat:chip', { id: e.detail.id });
    chatDraft.set(e.detail.text);
  }

  function handlePreviewConfirm(ev) {
    const id = (ev as CustomEvent<{ id: string }>).detail.id;
    console.info('[phase-16-stub] chat:preview-confirm', { id });
    chatPreviewCards.update((arr) => arr.filter((c) => c.id !== id));
  }

  function handlePreviewAdjust(ev) {
    const id = (ev as CustomEvent<{ id: string }>).detail.id;
    console.info('[phase-16-stub] chat:preview-adjust', { id });
    chatPreviewCards.update((arr) => arr.filter((c) => c.id !== id));
  }

  function handlePreviewDiscard(ev) {
    const id = (ev as CustomEvent<{ id: string }>).detail.id;
    console.info('[phase-16-stub] chat:preview-discard', { id });
    chatPreviewCards.update((arr) => arr.filter((c) => c.id !== id));
  }
</script>

<aside class="region-chat" data-testid="chat-panel" aria-label="Chat">
  <section class="cp">
    <ChatHead {subtext} />
    <ChatMessages
      messages={$chatMessages}
      previews={$chatPreviewCards}
      on:confirm={handlePreviewConfirm}
      on:adjust={handlePreviewAdjust}
      on:discard={handlePreviewDiscard}
    />
    <ChatSuggestChips {chips} on:chipclick={handleChipClick} />
    <ChatInputRow
      mode={$chatMode}
      {placeholder}
      draft={$chatDraft}
      on:modechange={handleModeChange}
      on:draftchange={handleDraftChange}
      on:send={handleSend}
    />
  </section>
</aside>
