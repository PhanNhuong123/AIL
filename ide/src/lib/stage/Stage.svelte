<script lang="ts">
  import { graph, selection } from '$lib/stores';
  import { zoomLevel } from '$lib/chrome/toolbar-state';
  import StageTabStrip from './StageTabStrip.svelte';
  import SystemView from './SystemView.svelte';
  import ModuleView from './ModuleView.svelte';
  import FlowView from './FlowView.svelte';
  import NodeView from './NodeView.svelte';
  import type { ModuleJson, FunctionJson, NodeDetail, FlowchartJson } from '$lib/types';
  import { flowFixture } from './fixtures';

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

  // Phase 16 stub; Phase 17 wires getFlowchart.
  function resolveFlowchart(_fnId) {
    const fx = flowFixture();
    return fx.flowchart as FlowchartJson;
  }

  $: activeModule         = findModule($graph, $selection.id);
  $: activeFunction       = findFunction($graph, $selection.id);
  $: activeDetail         = findStepDetail($graph, $selection.id);
  $: activeFlowchart      = activeFunction ? resolveFlowchart(activeFunction.id) : null;
  $: activeFunctionDetail = activeFunction
      ? ($graph?.detail[activeFunction.id] as NodeDetail | undefined) ?? null
      : null;
</script>

<div class="stage-root" data-testid="stage-root">
  <StageTabStrip />
  <div class="stage-body">
    {#if !$graph}
      <div class="stage-placeholder" data-testid="stage-empty">No project loaded.</div>
    {:else if $zoomLevel === 0}
      <SystemView graph={$graph} />
    {:else if $zoomLevel === 1 && activeModule}
      <ModuleView module={activeModule} />
    {:else if $zoomLevel === 1}
      <div class="stage-placeholder" data-testid="stage-module-missing">Module not found.</div>
    {:else if $zoomLevel === 2 && activeFunction}
      <FlowView fn={activeFunction} flowchart={activeFlowchart} detail={activeFunctionDetail} />
    {:else if $zoomLevel === 2}
      <div class="stage-placeholder" data-testid="stage-flow-missing">Function not found.</div>
    {:else if $zoomLevel === 4 && activeDetail && $selection.id}
      <NodeView stepId={$selection.id} detail={activeDetail} />
    {:else}
      <div class="stage-placeholder" data-testid="stage-node-missing">Node detail not found.</div>
    {/if}
  </div>
</div>
