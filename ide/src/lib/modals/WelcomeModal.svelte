<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { welcomeModalOpen, welcomeNotice } from '$lib/stores';
  import { setWelcomeDismissed } from '$lib/chat/sidebar-state';
  import { focusTrap } from './focus-trap';

  // The component dispatches `start` / `open` / `tutorial` events (one per
  // card). The route shell (`+page.svelte`) owns the actual project loading
  // because modals must NOT import `$lib/bridge.ts` (modals/CLAUDE.md).
  // The route closes the modal explicitly via the `welcomeModalOpen` store
  // after the side-effect resolves so transient failures keep the UI open.
  // Note: untyped `createEventDispatcher()` because esrap rejects generic
  // type annotations on it (invariant 16.2-E).
  const dispatch = createEventDispatcher();

  const cards = [
    {
      key: 'start',
      icon: '🎯',
      title: 'Start a new project',
      blurb: 'Begin from an empty workspace.',
    },
    {
      key: 'open',
      icon: '📂',
      title: 'Open an existing',
      blurb: 'Browse to a .ail project.',
    },
    {
      key: 'tutorial',
      icon: '🎓',
      title: 'Try the tutorial',
      blurb: 'Walk through the sample graph.',
    },
  ];

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  function close() {
    // Clear any browser-preview notice so a stale "Open is unavailable…"
    // message does not flash on the next re-open of the modal.
    welcomeNotice.set('');
    // Persist the dismissal flag so the modal does not re-open on every
    // reload. Previously the ✕ / ESC paths dropped the flag and only the
    // success-load route in `loadAndCloseWelcome` set it, which meant a
    // user who explicitly closed Welcome would still be greeted by it on
    // the next launch — annoying after the first dismissal.
    setWelcomeDismissed(true);
    welcomeModalOpen.set(false);
  }

  // Note: param types omitted because esrap rejects them in `.svelte`
  // script blocks (invariant 16.2-E). `key` is a string from `card.key`.
  // Pass `{ bubbles: true }` so tests + parents that listen via DOM
  // `addEventListener` (rather than the deprecated `$on`) still receive
  // the event — Svelte 5's `createEventDispatcher` defaults to non-bubbling.
  // The `as never` cast bypasses the incomplete `DispatchOptions` TS type
  // (it omits `bubbles` though the runtime supports it).
  function onCard(key) {
    dispatch(key, undefined, { bubbles: true } as never);
  }

  function handleBackdrop(e) {
    if (e.target === e.currentTarget) {
      close();
    }
  }

  // ESC closes the Welcome modal. The listener is attached for the component
  // lifetime (the modal stays mounted under `{#if $welcomeModalOpen}` from
  // routes/+page.svelte's perspective the wrapper component is always
  // present), and gates on the open store so ESC is a no-op when the modal
  // is hidden. Compatible with invariant 15.11-B: the listener triggers only
  // the close path, never the open path.
  onMount(() => {
    function handleKey(e) {
      if (e.key === 'Escape' && get(welcomeModalOpen)) close();
    }
    if (typeof document !== 'undefined') {
      document.addEventListener('keydown', handleKey);
    }
    return () => {
      if (typeof document !== 'undefined') {
        document.removeEventListener('keydown', handleKey);
      }
    };
  });
</script>

{#if $welcomeModalOpen}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
  <div
    class="modal-backdrop"
    data-testid="welcome-backdrop"
    on:click={handleBackdrop}
    role="dialog"
    aria-modal="true"
    aria-label="Welcome to AIL IDE"
  >
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
      class="modal-dialog welcome"
      tabindex="-1"
      use:focusTrap
      on:click|stopPropagation
    >
      <header class="modal-header">
        <span class="modal-title">Welcome to AIL IDE</span>
        <button
          class="modal-close-btn"
          data-testid="welcome-close"
          on:click={close}
          aria-label="Close"
        >
          ✕
        </button>
      </header>

      <div class="welcome-grid">
        {#each cards as card}
          <button
            class="welcome-card"
            data-testid="welcome-card-{card.key}"
            aria-label="{card.title} — {card.blurb}"
            on:click={() => onCard(card.key)}
          >
            <span class="welcome-icon" aria-hidden="true">{card.icon}</span>
            <span class="welcome-title">{card.title}</span>
            <span class="welcome-blurb">{card.blurb}</span>
          </button>
        {/each}
      </div>

      {#if $welcomeNotice}
        <p
          class="welcome-notice"
          data-testid="welcome-notice"
          role="status"
          aria-live="polite"
        >
          {$welcomeNotice}
        </p>
      {/if}
    </div>
  </div>
{/if}
