<script lang="ts">
  import { get } from 'svelte/store';
  import { graph, selection, density } from '$lib/stores';
  import { navigateTo } from '$lib/chrome/toolbar-state';
  import StageTabStrip from './StageTabStrip.svelte';
  import LensBanner from './LensBanner.svelte';
  import SystemView from './SystemView.svelte';
  import ModuleView from './ModuleView.svelte';
  import FlowView from './FlowView.svelte';
  import NodeView from './NodeView.svelte';
  import type { ModuleJson, FunctionJson, NodeDetail, FlowchartJson } from '$lib/types';
  type SelectedNodeDetailShape = { id: string; detail: NodeDetail } | null;
  import { getFlowchart, isTauri } from '$lib/bridge';

  // Phase 16.3: override prop for freshly-verified node detail. When set and
  // its id matches the current selection, it takes precedence over the
  // graph-derived detail so the UI shows post-verification outcomes without
  // needing a full graph reload. Shape is { id, detail } so matching uses
  // the real node id rather than a fictional sub-field.
  export let selectedNodeDetail = null as SelectedNodeDetailShape;

  function findModule(g, id) {
    const graphVal = g as import('$lib/types').GraphJson | null;
    const selId = id as string | null;
    if (!graphVal || !selId) return null;
    for (const m of graphVal.modules) {
      if (m.id === selId) return m as ModuleJson;
    }
    return null;
  }

  function findFunction(g, id) {
    const graphVal = g as import('$lib/types').GraphJson | null;
    const selId = id as string | null;
    if (!graphVal || !selId) return null;
    for (const m of graphVal.modules) {
      for (const fn_ of m.functions) {
        if (fn_.id === selId) return fn_ as FunctionJson;
      }
    }
    return null;
  }

  function findStepDetail(g, id) {
    const graphVal = g as import('$lib/types').GraphJson | null;
    const selId = id as string | null;
    if (!graphVal || !selId) return null;
    const d = graphVal.detail[selId];
    return (d as NodeDetail | undefined) ?? null;
  }

  // Real-flowchart fetch via the Tauri bridge. The previous Phase-16 stub
  // returned a hardcoded `flowFixture()` for every selected function, which
  // meant FlowView's Swim/Flowchart modes showed the same fictional 7-node
  // graph regardless of which function the user selected from a real
  // project. The bridge's `getFlowchart(fnId)` IPC returns the actual
  // FlowchartJson.
  //
  // Tracks the in-flight request so a slower response from a previous
  // selection cannot overwrite a newer one (paired-id { id, flowchart }
  // shape, same pattern Phase 16.3 used for selectedNodeDetail). Outside
  // Tauri (browser preview / vite dev), we skip the IPC and FlowView
  // renders an empty canvas — never the misleading stub.
  let activeFlowchartPair = null as { id: string; flowchart: FlowchartJson | null } | null;
  let lastFlowchartReq = 0;

  async function refetchFlowchart(fnId) {
    const reqId = ++lastFlowchartReq;
    if (!isTauri()) {
      if (reqId === lastFlowchartReq) {
        activeFlowchartPair = { id: fnId, flowchart: null };
      }
      return;
    }
    try {
      const fc = await getFlowchart(fnId);
      if (reqId === lastFlowchartReq) {
        activeFlowchartPair = { id: fnId, flowchart: fc };
      }
    } catch {
      if (reqId === lastFlowchartReq) {
        activeFlowchartPair = { id: fnId, flowchart: null };
      }
    }
  }

  $: kind                 = $selection.kind;
  $: stagePad = $density === 'compact'
    ? '16px 20px 60px'
    : $density === 'cozy'
    ? '24px 28px 80px'
    : '32px 36px 100px'; // 'comfortable' (default)
  $: stageKey = kind ?? 'none';
  $: activeModule         = findModule($graph, $selection.id);
  $: activeFunction       = findFunction($graph, $selection.id);
  $: graphDetail          = findStepDetail($graph, $selection.id);
  $: if (activeFunction && (!activeFlowchartPair || activeFlowchartPair.id !== activeFunction.id)) {
    refetchFlowchart(activeFunction.id);
  }
  $: activeFlowchart = activeFunction && activeFlowchartPair && activeFlowchartPair.id === activeFunction.id
    ? activeFlowchartPair.flowchart
    : null;
  $: activeFunctionDetail = activeFunction
      ? ($graph?.detail[activeFunction.id] as NodeDetail | undefined) ?? null
      : null;

  // Phase 16.3: prefer selectedNodeDetail when its id matches the current
  // selection. Uses the paired { id, detail } shape for a direct id compare.
  $: overrideMatchesSelection = selectedNodeDetail !== null && selectedNodeDetail.id === ($selection.id ?? '');
  $: activeDetail = overrideMatchesSelection && selectedNodeDetail !== null ? selectedNodeDetail.detail : graphDetail;

  // Banner scope tracks the selected id for module/function/step so the stats
  // stay aligned with the user's selection even when the entity lookup fails
  // ("not found" placeholder) — invariant 15.5-B. Other kinds (project, type,
  // error, none) fall back to project-scope (null).
  $: scopeId = kind === 'module' || kind === 'function' || kind === 'step'
      ? $selection.id
      : null;

  // Phase 17.4 — Jump to a peer step from the conflict section in NodeView.
  // Walks the graph to find the module and function ancestors of the peer step,
  // then calls navigateTo with the correct 3-segment path. If the peer is absent
  // (removed by the watcher between the sheaf run and the jump click), no-op.
  export function handleStepJump(peerId) {
    const g = get(graph);
    if (!g) return;
    for (const mod of g.modules) {
      for (const fn_ of mod.functions) {
        const step = (fn_.steps ?? []).find((s) => s.id === peerId);
        if (step) {
          const newPath = [mod.id, fn_.id, peerId];
          navigateTo(newPath, 'step', peerId, 4);
          return;
        }
      }
    }
    // R3 mitigation: peer absent (removed by watcher). No-op silently.
  }
</script>

<div class="stage-root" data-testid="stage-root">
  <StageTabStrip />
  <div class="stage-body">
    {#key stageKey}
      <div class="stage-inner" style="padding: {stagePad};" data-testid="stage-inner">
        {#if !$graph}
          <div class="stage-placeholder" data-testid="stage-empty">No project loaded.</div>
        {:else}
          <LensBanner {scopeId} />
          {#if kind === 'module' && activeModule}
            <ModuleView module={activeModule} />
          {:else if kind === 'module'}
            <div class="stage-placeholder" data-testid="stage-module-missing">Module not found.</div>
          {:else if kind === 'function' && activeFunction}
            <FlowView fn={activeFunction} flowchart={activeFlowchart} detail={activeFunctionDetail} />
          {:else if kind === 'function'}
            <div class="stage-placeholder" data-testid="stage-flow-missing">Function not found.</div>
          {:else if kind === 'step' && activeDetail && $selection.id}
            <NodeView stepId={$selection.id} detail={activeDetail} on:jumptonode={(e) => handleStepJump(e.detail.peerId)} />
          {:else if kind === 'step'}
            <div class="stage-placeholder" data-testid="stage-node-missing">Node detail not found.</div>
          {:else}
            <SystemView graph={$graph} />
          {/if}
        {/if}
      </div>
    {/key}
  </div>
</div>
