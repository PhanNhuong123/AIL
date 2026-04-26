import { describe, it, expect, afterEach } from 'vitest';
import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { patchEffects, clearPatchEffects } from '$lib/patch-effects';
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
