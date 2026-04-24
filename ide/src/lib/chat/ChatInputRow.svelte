<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { ChatMode } from './chat-state';

  export let mode = 'edit' as ChatMode;
  export let placeholder = '' as string;
  export let draft = '' as string;

  const dispatch = createEventDispatcher<{
    modechange: { mode: ChatMode };
    draftchange: { value: string };
    send: { text: string };
  }>();

  function setMode(next) {
    const m = next as ChatMode;
    dispatch('modechange', { mode: m });
  }

  function onInput(e) {
    const el = e.target as HTMLInputElement;
    dispatch('draftchange', { value: el.value });
  }

  function onKeyDown(e) {
    const ev = e as KeyboardEvent;
    if (ev.key === 'Enter') {
      dispatch('send', { text: draft });
    }
  }

  function onSendClick() {
    dispatch('send', { text: draft });
  }
</script>

<div class="cp-modes" role="group" aria-label="Chat mode">
  <button
    type="button"
    class="cp-mode-btn"
    class:active={mode === 'edit'}
    aria-pressed={mode === 'edit'}
    data-testid="chat-mode-btn-edit"
    on:click={() => setMode('edit')}
  >Edit</button>
  <button
    type="button"
    class="cp-mode-btn"
    class:active={mode === 'ask'}
    aria-pressed={mode === 'ask'}
    data-testid="chat-mode-btn-ask"
    on:click={() => setMode('ask')}
  >Ask</button>
  <button
    type="button"
    class="cp-mode-btn"
    class:active={mode === 'test'}
    aria-pressed={mode === 'test'}
    data-testid="chat-mode-btn-test"
    on:click={() => setMode('test')}
  >Test</button>
</div>

<div class="cp-input-row">
  <input
    type="text"
    class="cp-input"
    data-testid="chat-input"
    aria-label="Chat input"
    value={draft}
    {placeholder}
    on:input={onInput}
    on:keydown={onKeyDown}
  />
  <button
    type="button"
    class="cp-send-btn"
    data-testid="chat-send-btn"
    on:click={onSendClick}
  >Send</button>
</div>
