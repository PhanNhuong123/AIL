<script lang="ts">
  import type { FlowchartJson } from '$lib/types';
  import {
    flowViewport,
    flowNodePositions,
    flowSelectedNodeId,
    flowDraftEdge,
    createdEdges,
    seedPositions,
  } from './flow-state';
  import type { DraftEdge } from './flow-state';
  import { reduce, emptyState, hitTest } from './flow-interaction';
  import { NODE_W, NODE_H, GRID_SIZE } from './swim-layout';
  import FlowchartShape from './FlowchartShape.svelte';
  import FlowchartEdge from './FlowchartEdge.svelte';
  import FlowchartZoomControls from './FlowchartZoomControls.svelte';
  import { get } from 'svelte/store';

  export let flowchart = { nodes: [], edges: [] } as FlowchartJson;

  // Seed positions when flowchart changes
  $: { seedPositions(flowchart.nodes); }

  // Build node bounds for hit-testing from current positions
  function getNodeBounds() {
    const positions = get(flowNodePositions);
    return flowchart.nodes.map((n) => {
      const pos = positions.get(n.id) ?? { x: n.x, y: n.y };
      return { id: n.id, x: pos.x, y: pos.y, w: NODE_W, h: NODE_H };
    });
  }

  function svgCoordsFromEvent(e) {
    const svg = (e.currentTarget as SVGElement).closest('svg');
    if (!svg) return { svgX: e.offsetX, svgY: e.offsetY };
    const pt = (svg as SVGSVGElement).createSVGPoint();
    pt.x = e.clientX;
    pt.y = e.clientY;
    const vp = get(flowViewport);
    const raw = pt.matrixTransform((svg as SVGSVGElement).getScreenCTM()!.inverse());
    return {
      svgX: (raw.x - vp.x) / vp.k,
      svgY: (raw.y - vp.y) / vp.k,
    };
  }

  // Interaction reducer state — kept local, synced to stores on each transition
  let iState = emptyState();

  function dispatch(ev) {
    // Re-sync from all stores before reducing so that state set externally
    // (e.g. by FlowSwim on a previous mount, or by other components) is never
    // clobbered by a stale iState snapshot (invariant 15.8-A / 15.7-A).
    iState = {
      ...iState,
      viewport: get(flowViewport),
      positions: get(flowNodePositions),
      selectedNodeId: get(flowSelectedNodeId),
      draftEdge: get(flowDraftEdge),
      createdEdges: get(createdEdges),
    };
    iState = reduce(iState, ev);
    flowViewport.set(iState.viewport);
    flowNodePositions.set(iState.positions);
    flowSelectedNodeId.set(iState.selectedNodeId);
    flowDraftEdge.set(iState.draftEdge);
    createdEdges.set(iState.createdEdges);
  }

  function onWheel(e) {
    e.preventDefault();
    dispatch({
      type: 'wheel',
      dx: e.deltaX,
      dy: e.deltaY,
      ctrlKey: e.ctrlKey,
      metaKey: e.metaKey,
    });
    iState = { ...iState, viewport: get(flowViewport) };
  }

  function onMouseDown(e) {
    if (e.button !== 0) return;
    const { svgX, svgY } = svgCoordsFromEvent(e);
    const bounds = getNodeBounds();
    const selId = iState.selectedNodeId;
    const hit = hitTest(bounds, selId, svgX, svgY);

    if (hit.hit === 'port') {
      dispatch({ type: 'mousedown-port', nodeId: hit.nodeId, port: hit.port, tipX: svgX, tipY: svgY });
    } else if (hit.hit === 'node') {
      dispatch({ type: 'mousedown-node', nodeId: hit.nodeId, svgX, svgY });
    } else {
      dispatch({ type: 'mousedown-background' });
    }
    iState = { ...iState, positions: get(flowNodePositions), viewport: get(flowViewport) };
  }

  function onMouseMove(e) {
    if (iState.mode === 'idle') return;
    const { svgX, svgY } = svgCoordsFromEvent(e);
    dispatch({ type: 'mousemove', svgX, svgY });
    iState = { ...iState, positions: get(flowNodePositions) };
  }

  function onMouseUp(e) {
    if (iState.mode === 'idle') return;
    const { svgX, svgY } = svgCoordsFromEvent(e);
    const bounds = getNodeBounds();
    const hit = hitTest(bounds, null, svgX, svgY);
    if (hit.hit === 'node') {
      dispatch({ type: 'mouseup-node', nodeId: hit.nodeId });
    } else {
      dispatch({ type: 'mouseup-background' });
    }
    iState = { ...iState, positions: get(flowNodePositions), draftEdge: get(flowDraftEdge) };
  }

  $: vp = $flowViewport;
  $: positions = $flowNodePositions;
  $: selectedId = $flowSelectedNodeId;
  $: draftEdge = $flowDraftEdge as DraftEdge | null;
  // Tag each edge with a unique key to avoid duplicates in keyed each block.
  $: allEdges = [
    ...flowchart.edges.map((e, i) => ({ ...e, _key: `base-${i}-${e.from}->${e.to}` })),
    ...$createdEdges.map((e, i) => ({ ...e, _key: `created-${i}-${e.from}->${e.to}` })),
  ];

  function nodePos(n) {
    return positions.get(n.id) ?? { x: n.x, y: n.y };
  }
</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div
  class="flowchart-canvas"
  data-testid="flowchart-canvas"
  on:wheel={onWheel}
  on:mousedown={onMouseDown}
  on:mousemove={onMouseMove}
  on:mouseup={onMouseUp}
  role="presentation"
>
  <svg width="100%" height="100%" class="flowchart-svg">
    <defs>
      <marker id="arrow-ok" markerWidth="8" markerHeight="8" refX="6" refY="3" orient="auto">
        <path d="M0,0 L0,6 L8,3 z" fill="var(--ok)"/>
      </marker>
      <marker id="arrow-err" markerWidth="8" markerHeight="8" refX="6" refY="3" orient="auto">
        <path d="M0,0 L0,6 L8,3 z" fill="var(--fail)"/>
      </marker>
      <marker id="arrow-neutral" markerWidth="8" markerHeight="8" refX="6" refY="3" orient="auto">
        <path d="M0,0 L0,6 L8,3 z" fill="var(--ink-3)"/>
      </marker>
      <pattern id="canvas-grid" width={GRID_SIZE} height={GRID_SIZE} patternUnits="userSpaceOnUse">
        <circle cx="1" cy="1" r="1" fill="var(--line-2)" />
      </pattern>
    </defs>

    <!-- Dot-grid background: fixed to viewport (outside the pan/zoom <g>) -->
    <rect width="100%" height="100%" fill="url(#canvas-grid)" data-testid="canvas-grid" />

    <g transform="translate({vp.x},{vp.y}) scale({vp.k})">
      <!-- Edges -->
      {#each allEdges as edge (edge._key)}
        {@const fromNode = flowchart.nodes.find((n) => n.id === edge.from)}
        {@const toNode   = flowchart.nodes.find((n) => n.id === edge.to)}
        {#if fromNode && toNode}
          <FlowchartEdge
            {edge}
            {fromNode}
            {toNode}
            fromPos={nodePos(fromNode)}
            toPos={nodePos(toNode)}
          />
        {/if}
      {/each}

      <!-- Draft edge while dragging-port -->
      {#if draftEdge}
        {@const fromNode = flowchart.nodes.find((n) => n.id === draftEdge.fromId)}
        {#if fromNode}
          {@const fp = nodePos(fromNode)}
          <line
            x1={fp.x + NODE_W / 2}
            y1={fp.y + NODE_H}
            x2={draftEdge.tipX}
            y2={draftEdge.tipY}
            stroke="var(--accent)"
            stroke-width="1.5"
            stroke-dasharray="4 2"
          />
        {/if}
      {/if}

      <!-- Nodes -->
      {#each flowchart.nodes as node (node.id)}
        {@const pos = nodePos(node)}
        <FlowchartShape
          kind={node.kind}
          id={node.id}
          label={node.label}
          x={pos.x}
          y={pos.y}
          w={NODE_W}
          h={NODE_H}
          status={node.status}
          selected={selectedId === node.id}
        />
        <!-- Port circles when this node is selected (invariant 15.8-B) -->
        {#if selectedId === node.id}
          <circle cx={pos.x + NODE_W / 2} cy={pos.y}              r="5" class="port-circle" data-testid="port-top-{node.id}"/>
          <circle cx={pos.x + NODE_W}     cy={pos.y + NODE_H / 2} r="5" class="port-circle" data-testid="port-right-{node.id}"/>
          <circle cx={pos.x + NODE_W / 2} cy={pos.y + NODE_H}     r="5" class="port-circle" data-testid="port-bottom-{node.id}"/>
          <circle cx={pos.x}              cy={pos.y + NODE_H / 2} r="5" class="port-circle" data-testid="port-left-{node.id}"/>
        {/if}
      {/each}
    </g>
  </svg>

  <FlowchartZoomControls
    on:zoomin={() => { dispatch({ type: 'zoom-in' }); iState = { ...iState, viewport: get(flowViewport) }; }}
    on:zoomout={() => { dispatch({ type: 'zoom-out' }); iState = { ...iState, viewport: get(flowViewport) }; }}
    on:zoomreset={() => { dispatch({ type: 'zoom-reset' }); iState = { ...iState, viewport: get(flowViewport) }; }}
  />
</div>

<style>
  .flowchart-canvas {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    background: var(--surface);
    position: relative;
    cursor: default;
  }

  .flowchart-svg {
    display: block;
    width: 100%;
    height: 100%;
  }

  .port-circle {
    fill: var(--accent);
    stroke: var(--surface);
    stroke-width: 1.5px;
    cursor: crosshair;
  }
</style>
