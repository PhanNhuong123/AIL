<script lang="ts">
  import { graph, selection, path } from '$lib/stores';
  import type { SelectionKind } from '$lib/stores';
  import { expanded, filterTerm, toggleExpanded } from './outline-state';
  import { filterTree, isVisible, ALL } from './filter';
  import { zoomLevel, stageLevelForKind } from './toolbar-state';
  import { patchEffects } from '$lib/patch-effects';
  import { sheafConflicts } from '$lib/sheaf/sheaf-state';
  import OutlineRow from './OutlineRow.svelte';
  import OutlineSection from './OutlineSection.svelte';

  $: g = $graph;
  $: visible = filterTree(g, $filterTerm);
  // When filter is active, show all visible items regardless of expand state
  $: filtering = visible !== ALL;

  $: addedIds    = $patchEffects.addedIds;
  $: modifiedIds = $patchEffects.modifiedIds;
  $: removedIds  = $patchEffects.removedIds;

  function patchStateFor(id) {
    if (addedIds.includes(id)) return 'added';
    if (modifiedIds.includes(id)) return 'modified';
    if (removedIds.includes(id)) return 'removed';
    return undefined;
  }

  // Outline row clicks update selection, path, and zoomLevel (via stageLevelForKind),
  // intentionally outside the toolbar back/forward history stack (invariant 15.4-B).
  function selectNode(kind, id, parentIds) {
    selection.set({ kind: kind as SelectionKind, id });
    path.set([...parentIds, id]);
    zoomLevel.set(stageLevelForKind(kind as SelectionKind));
  }

  function isOpen(key) {
    return filtering || $expanded.has(key);
  }

  $: conflictIds = new Set(
    $sheafConflicts.flatMap((c) => [c.nodeA, c.nodeB])
  );

  function conflictFor(id) {
    return conflictIds.has(id) ? true : undefined;
  }
</script>

<aside class="region-outline" data-testid="region-outline">
  <div class="nav-filter">
    <input
      class="nav-filter-input"
      type="text"
      placeholder="Filter…"
      bind:value={$filterTerm}
      aria-label="Filter outline"
    />
  </div>

  {#if g}
    <OutlineSection title="PROJECT">
      {#if isVisible(visible, g.project.id)}
        <OutlineRow
          kind="project"
          name={g.project.name}
          status={g.project.status}
          depth={0}
          hasChildren={g.modules.length > 0}
          expanded={isOpen(g.project.id)}
          selected={$selection.id === g.project.id}
          onClick={() => selectNode('project', g.project.id, [])}
          onToggle={() => toggleExpanded(g.project.id)}
        />
      {/if}

      {#each g.modules as mod (mod.id)}
        {#if isVisible(visible, mod.id)}
          <div data-patch-state={patchStateFor(mod.id)}>
            <OutlineRow
              kind="module"
              name={mod.name}
              status={mod.status}
              depth={1}
              hasChildren={mod.functions.length > 0}
              expanded={isOpen(mod.id)}
              selected={$selection.id === mod.id}
              onClick={() => selectNode('module', mod.id, [g.project.id])}
              onToggle={() => toggleExpanded(mod.id)}
            />

            {#if isOpen(mod.id)}
              {#each mod.functions as fn_ (fn_.id)}
                {#if isVisible(visible, fn_.id)}
                  <div data-patch-state={patchStateFor(fn_.id)}>
                    <OutlineRow
                      kind="function"
                      name={fn_.name}
                      status={fn_.status}
                      depth={2}
                      hasChildren={(fn_.steps || []).length > 0}
                      expanded={isOpen(fn_.id)}
                      selected={$selection.id === fn_.id}
                      onClick={() => selectNode('function', fn_.id, [g.project.id, mod.id])}
                      onToggle={() => toggleExpanded(fn_.id)}
                    />

                    {#if isOpen(fn_.id)}
                      {#each fn_.steps || [] as step (step.id)}
                        {#if isVisible(visible, step.id)}
                          <div data-patch-state={patchStateFor(step.id)} data-conflict={conflictFor(step.id)}>
                            <OutlineRow
                              kind="step"
                              name={step.name}
                              status={step.status}
                              depth={3}
                              hasChildren={false}
                              expanded={false}
                              selected={$selection.id === step.id}
                              onClick={() => selectNode('step', step.id, [g.project.id, mod.id, fn_.id])}
                            />
                          </div>
                        {/if}
                      {/each}
                    {/if}
                  </div>
                {/if}
              {/each}
            {/if}
          </div>
        {/if}
      {/each}
    </OutlineSection>

    {#if g.types.some((t) => isVisible(visible, t.id))}
      <OutlineSection title="TYPES">
        {#each g.types as tp}
          {#if isVisible(visible, tp.id)}
            <OutlineRow
              kind="type"
              name={tp.name}
              status={tp.status}
              depth={1}
              hasChildren={false}
              expanded={false}
              selected={$selection.id === tp.id}
              onClick={() => selectNode('type', tp.id, [g.project.id])}
            />
          {/if}
        {/each}
      </OutlineSection>
    {/if}

    {#if g.errors.some((e) => isVisible(visible, e.id))}
      <OutlineSection title="ERRORS">
        {#each g.errors as err}
          {#if isVisible(visible, err.id)}
            <OutlineRow
              kind="error"
              name={err.name}
              status={err.status}
              depth={1}
              hasChildren={false}
              expanded={false}
              selected={$selection.id === err.id}
              onClick={() => selectNode('error', err.id, [g.project.id])}
            />
          {/if}
        {/each}
      </OutlineSection>
    {/if}
  {/if}
</aside>

<style>
  .nav-filter {
    padding: 8px;
    border-bottom: 1px solid var(--line);
  }

  .nav-filter-input {
    width: 100%;
    box-sizing: border-box;
    background: var(--surface-3);
    border: 1px solid var(--line-2);
    border-radius: var(--radius-sm);
    color: var(--ink);
    font-size: 12px;
    padding: 4px 8px;
    outline: none;
  }

  .nav-filter-input::placeholder {
    color: var(--ink-3);
  }

  .nav-filter-input:focus {
    border-color: var(--accent);
  }
</style>
