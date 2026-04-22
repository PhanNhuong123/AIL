<script lang="ts">
  // Platform window-control buttons. Mac variant uses the macOS platform hex colours
  // (the only file in the project permitted to hard-code these values).
  // Click handlers are Phase 17 stubs; Tauri window API wiring arrives then.
  export let variant = 'mac' as 'mac' | 'generic';

  function handleClose()    { console.log('close'); }
  function handleMinimize() { console.log('minimize'); }
  function handleMaximize() { console.log('maximize'); }
</script>

<div class="traffic-lights" role="group" aria-label="Window controls" data-testid="traffic-lights-{variant}">
  {#if variant === 'mac'}
    <!-- Hex colours #ff5f57, #febc2e, #28c840 are the macOS platform UI standard.
         This is the only file in the project permitted to use hard-coded hex values. -->
    <button class="tl-btn" aria-label="Close"    style="background: #ff5f57;" on:click={handleClose}></button>
    <button class="tl-btn" aria-label="Minimize" style="background: #febc2e;" on:click={handleMinimize}></button>
    <button class="tl-btn" aria-label="Maximize" style="background: #28c840;" on:click={handleMaximize}></button>
  {:else}
    <button class="tl-btn tl-btn-generic tl-btn-close" aria-label="Close"    on:click={handleClose}></button>
    <button class="tl-btn tl-btn-generic tl-btn-min"   aria-label="Minimize" on:click={handleMinimize}></button>
    <button class="tl-btn tl-btn-generic tl-btn-max"   aria-label="Maximize" on:click={handleMaximize}></button>
  {/if}
</div>

<style>
  .traffic-lights {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-right: 12px;
  }

  .tl-btn {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: none;
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
  }

  .tl-btn:hover {
    filter: brightness(0.85);
  }

  .tl-btn-generic.tl-btn-close { background: color-mix(in srgb, var(--fail) 60%, var(--surface-3)); }
  .tl-btn-generic.tl-btn-min   { background: color-mix(in srgb, var(--warn) 60%, var(--surface-3)); }
  .tl-btn-generic.tl-btn-max   { background: color-mix(in srgb, var(--ok)   60%, var(--surface-3)); }
</style>
