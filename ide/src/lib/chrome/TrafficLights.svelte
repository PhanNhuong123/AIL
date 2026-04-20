<script lang="ts">
  // macOS window-control buttons (platform UI exception — fixed hex colours allowed).
  // Click handlers are placeholders; Tauri window API wiring arrives in Phase 17.
  function handleClose()    { console.log('close'); }
  function handleMinimize() { console.log('minimize'); }
  function handleMaximize() { console.log('maximize'); }
</script>

<div class="traffic-lights" role="group" aria-label="Window controls">
  <!-- Hex colours #ff5f57, #febc2e, #28c840 are the macOS platform UI standard.
       This is the only file in the project permitted to use hard-coded hex values. -->
  <button
    class="tl-btn"
    aria-label="Close"
    style="background: #ff5f57;"
    on:click={handleClose}
  ></button>
  <button
    class="tl-btn"
    aria-label="Minimize"
    style="background: #febc2e;"
    on:click={handleMinimize}
  ></button>
  <button
    class="tl-btn"
    aria-label="Maximize"
    style="background: #28c840;"
    on:click={handleMaximize}
  ></button>
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
</style>
