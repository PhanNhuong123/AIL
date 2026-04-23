/**
 * swim-layout.ts — Shared layout constants for FlowSwim and SwimNode.
 *
 * These constants control node dimensions and padding for the swim-lane
 * rendering. They are shared between FlowSwim.svelte (for SVG height
 * calculation and edge anchor geometry) and SwimNode.svelte (for rect
 * position and text placement). Changing these in one file without the
 * other would silently clip or mis-align nodes.
 */

export const NODE_W = 120;
export const NODE_H = 48;
export const PAD    = 24;
