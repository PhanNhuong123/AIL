<script lang="ts">
  import {
    bottomActiveTab,
    bottomCollapsed,
    ASSISTANT_SEED,
    PREVIEW_SEED,
    TIP_TEXT,
    INPUT_PLACEHOLDER,
    CONSOLE_SEED,
  } from './bottom-panel-state';
  import type { BottomTab, ConsoleLevel } from './bottom-panel-state';
  import Icon from '$lib/icons/Icon.svelte';

  // Component-local draft — never a store
  let draft = '';

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  function selectTab(next) {
    bottomActiveTab.set(next as BottomTab);
  }

  function toggleCollapse() {
    bottomCollapsed.update((v) => !v);
  }

  function handleSend() {
    console.info('[phase-17-stub] send', { text: draft });
    draft = '';
  }

  function handleConfirm() {
    console.info('[phase-17-stub] preview: confirm');
  }

  function handleAdjust() {
    console.info('[phase-17-stub] preview: adjust');
  }

  function handleDiscard() {
    console.info('[phase-17-stub] preview: discard');
  }

  function levelIcon(level) {
    return (level as ConsoleLevel) === 'ok'
      ? '✓'
      : (level as ConsoleLevel) === 'warn'
        ? '⚠'
        : '✗';
  }
</script>

<section class="region-bottom bp" data-testid="bottom-panel">
  <!-- Head: tabs + collapse button -->
  <div class="bp-head">
    <button
      class="bp-tab"
      class:active={$bottomActiveTab === 'chat'}
      data-testid="bp-tab-chat"
      on:click={() => selectTab('chat')}
    >
      Chat
    </button>
    <button
      class="bp-tab"
      class:active={$bottomActiveTab === 'console'}
      data-testid="bp-tab-console"
      on:click={() => selectTab('console')}
    >
      Console
    </button>
    <button
      class="bp-collapse-btn"
      data-testid="bp-collapse-btn"
      on:click={toggleCollapse}
      title={$bottomCollapsed ? 'Expand panel' : 'Collapse panel'}
    >
      {#if $bottomCollapsed}
        <span class="bp-caret-up"><Icon name="chevron-down" /></span>
      {:else}
        <Icon name="chevron-down" />
      {/if}
    </button>
  </div>

  <!-- Body: hidden when collapsed -->
  {#if !$bottomCollapsed}
    <div class="bp-body">
      {#if $bottomActiveTab === 'chat'}
        <!-- Chat scroll: seed message → preview card → tip -->
        <div class="bp-chat-scroll" data-testid="bp-chat-scroll">
          <!-- Seeded assistant message -->
          <div class="bp-msg">
            <span class="bp-msg-role">AIL Assistant</span>
            <span>{ASSISTANT_SEED.text}</span>
          </div>

          <!-- Preview card (Phase-17 stub — rendered unconditionally in Phase 16) -->
          <div class="bp-preview" data-testid="bp-preview-card">
            <div class="bp-preview-title">{PREVIEW_SEED.title}</div>
            <div class="bp-preview-summary">{PREVIEW_SEED.summary}</div>
            <div class="bp-preview-actions">
              <button
                class="primary"
                data-testid="bp-preview-confirm"
                on:click={handleConfirm}
              >
                Confirm
              </button>
              <button data-testid="bp-preview-adjust" on:click={handleAdjust}>
                Adjust
              </button>
              <button data-testid="bp-preview-discard" on:click={handleDiscard}>
                Discard
              </button>
            </div>
          </div>
        </div>

        <!-- Tip line -->
        <div class="bp-tip">{TIP_TEXT}</div>

        <!-- Input row -->
        <div class="bp-input-row">
          <input
            type="text"
            bind:value={draft}
            placeholder={INPUT_PLACEHOLDER}
            data-testid="bp-input"
            on:keydown={(e) => {
              const ev = e as KeyboardEvent;
              if (ev.key === 'Enter') handleSend();
            }}
          />
          <button data-testid="bp-send-btn" on:click={handleSend}>Send</button>
        </div>
      {:else}
        <!-- Console tab -->
        <div class="bp-console" data-testid="bp-console">
          {#each CONSOLE_SEED as line, i}
            <div
              class="bp-console-line {line.level}"
              data-testid="bp-console-line-{i}"
            >
              <span class="bp-console-ts">{line.timestamp}</span>
              <span class="bp-console-icon">{levelIcon(line.level)}</span>
              <span class="bp-console-text">{line.text}</span>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</section>
