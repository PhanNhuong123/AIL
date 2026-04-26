import { describe, it, expect, afterEach, test } from 'vitest';
import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { patchEffects, clearPatchEffects } from '$lib/patch-effects';
import { density, selection, activeLens } from '$lib/stores';
import { chatDraft } from '$lib/chat/chat-state';
import { flowMode, flowSelectedNodeId } from '$lib/stage/flow-state';
import SystemGrid from './SystemGrid.svelte';
import type { GraphJson } from '$lib/types';

describe('SystemGrid — task 16.2 patch-state binding', () => {
  afterEach(() => clearPatchEffects());

  it('binds data-patch-state="added" to module wrappers when patchEffects.addedIds contains the id', async () => {
    const graph: GraphJson = {
      project: { id: 'p', name: 'p', description: '', nodeCount: 2, moduleCount: 2, fnCount: 0, status: 'ok' },
      clusters: [], modules: [
        { id: 'm-A', name: 'A', description: '', cluster: 'c', clusterName: 'c', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] },
        { id: 'm-B', name: 'B', description: '', cluster: 'c', clusterName: 'c', clusterColor: '#fff', status: 'ok', nodeCount: 1, functions: [] },
      ],
      externals: [], relations: [], types: [], errors: [], issues: [], detail: {},
    };
    patchEffects.set({ addedIds: ['m-A'], modifiedIds: [], removedIds: [] });
    const { container } = render(SystemGrid, { props: { graph } });
    await tick();
    const addedWrapper = container.querySelector('[data-patch-state="added"]');
    expect(addedWrapper).not.toBeNull();
  });
});

test('density change does not reset selection/lens/chat/flow stores', () => {
  // Snapshot initial state
  selection.set({ kind: 'module', id: 'module:m_billing' });
  activeLens.set('verify');
  chatDraft.set('hello');
  flowMode.set('Swim');
  flowSelectedNodeId.set('step:foo');

  const before = {
    selection: get(selection),
    activeLens: get(activeLens),
    chatDraft: get(chatDraft),
    flowMode: get(flowMode),
    flowSelectedNodeId: get(flowSelectedNodeId),
  };

  // Mutate density
  density.set('compact');

  // Assert nothing else changed
  expect(get(selection)).toEqual(before.selection);
  expect(get(activeLens)).toBe(before.activeLens);
  expect(get(chatDraft)).toBe(before.chatDraft);
  expect(get(flowMode)).toBe(before.flowMode);
  expect(get(flowSelectedNodeId)).toBe(before.flowSelectedNodeId);
});
