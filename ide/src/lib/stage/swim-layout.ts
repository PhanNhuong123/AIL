/**
 * swim-layout.ts — Shared layout constants for FlowSwim, SwimNode, and
 * FlowchartCanvas.
 *
 * NODE_W / NODE_H control node dimensions and are shared across both
 * swim-lane rendering (FlowSwim.svelte, SwimNode.svelte) and the flowchart
 * canvas (FlowchartCanvas.svelte, FlowchartEdge.svelte) to keep node boxes,
 * port anchors, and edge geometry aligned. Changing these values here
 * propagates consistently to all consumers.
 *
 * PAD is swim-lane-specific row padding and is not used by FlowchartCanvas.
 *
 * GRID_SIZE is the dot-grid repeat unit used by the flowchart canvas
 * background pattern.
 */

export const NODE_W    = 120;
export const NODE_H    = 48;
export const PAD       = 24;
export const GRID_SIZE = 24;
