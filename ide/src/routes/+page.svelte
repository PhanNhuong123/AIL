<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import TitleBar from '$lib/chrome/TitleBar.svelte';
  import Outline from '$lib/chrome/Outline.svelte';
  import Stage from '$lib/stage/Stage.svelte';
  import ChatPanel from '$lib/chat/ChatPanel.svelte';
  import WelcomeModal from '$lib/modals/WelcomeModal.svelte';
  import QuickCreateModal from '$lib/modals/QuickCreateModal.svelte';
  import TweaksPanel from '$lib/modals/TweaksPanel.svelte';
  import { onGraphUpdated, startWatchProject } from '$lib/bridge';
  import { graph, selection } from '$lib/stores';
  import { applyGraphPatch, reconcileSelectionAfterPatch } from '$lib/graph-patch';
  import '../styles/tokens.css';
  import '../styles/chrome.css';
  import '../styles/stage.css';

  // Track the project id we've already started a watcher for. Re-fires only
  // when `loadProject` returns a different project (re-load scenario).
  let watchedProjectId = null as string | null;

  onMount(() => {
    let unlisten = null as (() => void) | null;
    let destroyed = false;
    onGraphUpdated((patch) => {
      // Invariant 15.11-C: patch application writes ONLY `graph` and
      // `selection`. Every other store (node-view, chat, flow) is preserved.
      graph.update((g) => (g ? applyGraphPatch(g, patch) : g));
      const next = get(graph);
      if (next) {
        selection.update((s) => reconcileSelectionAfterPatch(s, patch, next));
      }
    }).then((fn) => {
      if (destroyed) fn();
      else unlisten = fn;
    });
    return () => {
      destroyed = true;
      if (unlisten) unlisten();
    };
  });

  // Start the watcher once a project becomes loaded. Tracks project id to
  // support re-load: on a new project the id changes and we start again.
  $: {
    const g = $graph;
    const newId = g?.project.id ?? null;
    if (newId && newId !== watchedProjectId) {
      watchedProjectId = newId;
      startWatchProject().catch((e) => console.warn('[watcher] start failed', e));
    }
  }
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
