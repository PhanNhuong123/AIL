<script lang="ts">
  import TitleBar from '$lib/chrome/TitleBar.svelte';
  import Toolbar from '$lib/chrome/Toolbar.svelte';
  import Navigator from '$lib/chrome/Navigator.svelte';
  import Stage from '$lib/stage/Stage.svelte';
  import ContextPanel from '$lib/chrome/ContextPanel.svelte';
  import BottomPanel from '$lib/chrome/BottomPanel.svelte';
  import WelcomeModal from '$lib/modals/WelcomeModal.svelte';
  import QuickCreateModal from '$lib/modals/QuickCreateModal.svelte';
  import TweaksPanel from '$lib/modals/TweaksPanel.svelte';
  import { bottomCollapsed } from '$lib/chrome/bottom-panel-state';
  import '../styles/tokens.css';
  import '../styles/chrome.css';
  import '../styles/stage.css';
</script>

<div class="app-root" class:bottom-collapsed={$bottomCollapsed}>
  <TitleBar />
  <Navigator />
  <Toolbar />
  <main class="region-stage">
    <Stage />
  </main>
  <ContextPanel />
  <BottomPanel />
</div>

<WelcomeModal />
<QuickCreateModal />
<TweaksPanel />
