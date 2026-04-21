<script lang="ts">
  import { tweaksPanelOpen, theme, density } from '$lib/stores';
  import type { Density } from '$lib/stores';

  // Reactive derived — is current theme light?
  $: isLight = $theme === 'light';

  const shortcuts = [
    { key: '⌘K', label: 'Quick Create' },
    { key: '⌘P', label: 'Command Palette' },
    { key: '⌘1..4', label: 'Zoom level' },
    { key: '⌘[ / ⌘]', label: 'Navigate back/forward' },
    { key: 'Esc', label: 'Close modal' },
  ];

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  function close() {
    tweaksPanelOpen.set(false);
  }

  function handleBackdrop(e) {
    if (e.target === e.currentTarget) {
      close();
    }
  }

  function toggleTheme() {
    const next = isLight ? 'dark' : 'light';
    theme.set(next);
    if (typeof document !== 'undefined') {
      document.documentElement.classList.toggle('light', next === 'light');
    }
  }

  function setAccent(e) {
    const value = (e.currentTarget as HTMLInputElement).value;
    if (typeof document !== 'undefined') {
      document.documentElement.style.setProperty('--accent', value);
    }
  }

  function setDensity(next) {
    density.set(next as Density);
  }
</script>

{#if $tweaksPanelOpen}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
  <div
    class="modal-backdrop tweaks-backdrop"
    data-testid="tweaks-backdrop"
    on:click={handleBackdrop}
    role="complementary"
    aria-label="Tweaks"
  >
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
    <aside
      class="tweaks-panel"
      data-testid="tweaks-panel"
      on:click|stopPropagation
    >
      <header class="modal-header">
        <span class="modal-title">Tweaks</span>
        <button
          class="modal-close-btn"
          data-testid="tweaks-close"
          on:click={close}
          aria-label="Close"
        >
          ✕
        </button>
      </header>

      <div class="tweaks-body">
        <!-- Theme -->
        <div class="tweaks-section">
          <span class="tweaks-section-label">Theme</span>
          <button
            class="tweaks-theme-btn"
            data-testid="tweaks-theme-toggle"
            on:click={toggleTheme}
          >
            {isLight ? 'Light — switch to Dark' : 'Dark — switch to Light'}
          </button>
        </div>

        <!-- Accent colour -->
        <div class="tweaks-section">
          <span class="tweaks-section-label">Accent colour</span>
          <div class="tweaks-accent-row">
            <input
              type="color"
              data-testid="tweaks-accent-input"
              value="#2997ff"
              on:input={setAccent}
            />
            <span class="tweaks-hint">Click to pick</span>
          </div>
        </div>

        <!-- Density -->
        <div class="tweaks-section">
          <span class="tweaks-section-label">Density</span>
          <div class="tweaks-segmented">
            <button
              class:active={$density === 'compact'}
              data-testid="tweaks-density-compact"
              on:click={() => setDensity('compact')}
            >
              Compact
            </button>
            <button
              class:active={$density === 'cozy'}
              data-testid="tweaks-density-cozy"
              on:click={() => setDensity('cozy')}
            >
              Cozy
            </button>
            <button
              class:active={$density === 'comfortable'}
              data-testid="tweaks-density-comfortable"
              on:click={() => setDensity('comfortable')}
            >
              Comfy
            </button>
          </div>
        </div>

        <!-- Keyboard shortcuts -->
        <div class="tweaks-section">
          <span class="tweaks-section-label">Shortcuts</span>
          <ul class="tweaks-shortcuts">
            {#each shortcuts as s}
              <li>
                <span>{s.label}</span>
                <kbd>{s.key}</kbd>
              </li>
            {/each}
          </ul>
        </div>
      </div>
    </aside>
  </div>
{/if}
