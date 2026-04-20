<script lang="ts">
  import TitleBar from '$lib/chrome/TitleBar.svelte';
  import Toolbar from '$lib/chrome/Toolbar.svelte';
  import Navigator from '$lib/chrome/Navigator.svelte';
  import Stage from '$lib/stage/Stage.svelte';
  import ContextPanel from '$lib/chrome/ContextPanel.svelte';
  import BottomPanel from '$lib/chrome/BottomPanel.svelte';
  import '../styles/tokens.css';
  import '../styles/chrome.css';
  import '../styles/stage.css';
</script>

<div class="app-root">
  <TitleBar />
  <Navigator />
  <Toolbar />
  <main class="region-stage">
    <Stage />
  </main>
  <ContextPanel />
  <BottomPanel />
</div>
