<script lang="ts">
  import TitleBar from '$lib/chrome/TitleBar.svelte';
  import Navigator from '$lib/chrome/Navigator.svelte';
  import ContextPanel from '$lib/chrome/ContextPanel.svelte';
  import BottomPanel from '$lib/chrome/BottomPanel.svelte';
  import '../styles/tokens.css';
  import '../styles/chrome.css';
  import '../styles/stage.css';
</script>

<div class="app-root">
  <TitleBar />
  <Navigator />
  <main class="region-stage">
    <div class="stage-placeholder">Stage — 16.5 lands here.</div>
  </main>
  <ContextPanel />
  <BottomPanel />
</div>
