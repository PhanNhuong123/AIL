<script lang="ts">
  import { welcomeModalOpen } from '$lib/stores';

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
    welcomeModalOpen.set(false);
  }

  function onCard(key) {
    console.info('[phase-17-stub] welcome:', key);
    close();
  }

  function handleBackdrop(e) {
    if (e.target === e.currentTarget) {
      close();
    }
  }
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
    <div class="modal-dialog welcome" on:click|stopPropagation>
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
            on:click={() => onCard(card.key)}
          >
            <span class="welcome-icon">{card.icon}</span>
            <span class="welcome-title">{card.title}</span>
            <span class="welcome-blurb">{card.blurb}</span>
          </button>
        {/each}
      </div>
    </div>
  </div>
{/if}
