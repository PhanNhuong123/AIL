<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { get } from 'svelte/store';
  import { selection, path, activeLens } from '$lib/stores';
  import { runAgent, cancelAgentRun } from '$lib/bridge';
  import type { AgentRunRequest } from '$lib/types';
  import {
    chatMode, chatDraft, chatMessages, chatPreviewCards,
    isAgentRunning, currentRunId,
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

  // Canonical parent channel for graph-mutating preview actions. The host
  // (`+page.svelte`) binds `on:previewapply` / `on:previewdismiss` and owns
  // the `graph` / `selection` writes (invariant 16.1-C). Untyped dispatch
  // to match the rest of this component's event style.
  const dispatch = createEventDispatcher();

  $: subtext = contextSubtextFor($selection, $path);
  $: chips = suggestChipsFor($selection, $activeLens);
  $: placeholder = placeholderFor($chatMode);

  async function handleSend(ev) {
    const e = ev as CustomEvent<{ text: string }>;
    const text = (e.detail?.text ?? '').trim();
    if (!text) return;

    // In-flight guard — 3a-1 HIGH #3 (double-send) + invariant 16.1-B.
    // MUST be set BEFORE any await so synchronous double-clicks cannot
    // both pass the check.
    if (get(isAgentRunning)) return;
    isAgentRunning.set(true);

    // Read send-time context through get(store) — invariant 16.1-A.
    const sel = get(selection);
    const req = {
      text,
      selectionKind: sel.kind,
      selectionId:   sel.id,
      path:          get(path),
      lens:          get(activeLens),
      mode:          get(chatMode),
    } as AgentRunRequest;

    const uid = `u-${++msgSeq}`;
    chatMessages.update((arr) => [
      ...arr,
      { id: uid, role: 'user', text },
    ]);
    chatDraft.set('');

    try {
      const runId = await runAgent(req);
      currentRunId.set(runId);
    } catch (err) {
      const aid = `a-${++msgSeq}`;
      chatMessages.update((arr) => [
        ...arr,
        { id: aid, role: 'assistant', text: `Agent error: ${String(err)}` },
      ]);
      isAgentRunning.set(false);
      currentRunId.set(null);
    }
  }

  async function handleStop() {
    const id = get(currentRunId);
    if (!id) return;
    try {
      await cancelAgentRun(id);
    } catch {
      // Best-effort: the route-level agent-complete listener clears the
      // running state on the cancellation event. If the backend never
      // emits it (e.g. IPC failure), fall back to a local clear here.
      isAgentRunning.set(false);
      currentRunId.set(null);
    }
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
    chatDraft.set(e.detail.text);
  }

  function handlePreviewConfirm(ev) {
    const id = (ev as CustomEvent<{ id: string }>).detail.id;
    const card = get(chatPreviewCards).find((c) => c.id === id);
    if (!card) return;
    // Dispatch upward — the route-level handler applies the patch and
    // removes the card. Chat components never write `graph` / `selection`
    // directly (invariant 16.1-C).
    dispatch('previewapply', card);
  }

  function handlePreviewAdjust(ev) {
    const id = (ev as CustomEvent<{ id: string }>).detail.id;
    const card = get(chatPreviewCards).find((c) => c.id === id);
    if (!card) return;
    // Refine: pre-populate the draft with the card summary so the user
    // can edit and resend. Card stays visible until the user sends.
    chatDraft.set(card.summary);
  }

  function handlePreviewDiscard(ev) {
    const id = (ev as CustomEvent<{ id: string }>).detail.id;
    dispatch('previewdismiss', { id });
  }
</script>

<aside class="region-chat" data-testid="chat-panel" aria-label="Chat">
  <section class="cp">
    <ChatHead
      {subtext}
      isRunning={$isAgentRunning}
      on:stop={handleStop}
    />
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
      isRunning={$isAgentRunning}
      on:modechange={handleModeChange}
      on:draftchange={handleDraftChange}
      on:send={handleSend}
    />
  </section>
</aside>
