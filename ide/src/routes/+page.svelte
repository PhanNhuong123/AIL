<script lang="ts">
  import TitleBar from '$lib/chrome/TitleBar.svelte';
  import Outline from '$lib/chrome/Outline.svelte';
  import Stage from '$lib/stage/Stage.svelte';
  import ChatPanel from '$lib/chat/ChatPanel.svelte';
  import WelcomeModal from '$lib/modals/WelcomeModal.svelte';
  import QuickCreateModal from '$lib/modals/QuickCreateModal.svelte';
  import TweaksPanel from '$lib/modals/TweaksPanel.svelte';
  import '../styles/tokens.css';
  import '../styles/chrome.css';
  import '../styles/stage.css';
</script>

<div class="app-root" data-testid="app-root">
  <TitleBar />
  <Outline />
  <main class="region-stage" data-testid="region-stage"><Stage /></main>
  <ChatPanel />
</div>

<WelcomeModal />
<QuickCreateModal />
<TweaksPanel />
