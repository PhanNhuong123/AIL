<script lang="ts">
  import { graph, paletteVisible, selection, path } from '$lib/stores';
  import type { SelectionKind } from '$lib/stores';
  import { expanded, filterTerm, toggleExpanded } from './navigator-state';
  import { filterTree, isVisible, ALL } from './filter';
  import NavigatorRow from './NavigatorRow.svelte';
  import NavigatorSection from './NavigatorSection.svelte';
  import PaletteSection from './PaletteSection.svelte';
  import { createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher();

  $: g = $graph;
  $: visible = filterTree(g, $filterTerm);
  // When filter is active, show all visible items regardless of expand state
  $: filtering = visible !== ALL;

  // id is the full "kind:id" string (e.g. "module:m_wallet")
  function selectNode(kind, id, parentIds) {
    selection.set({ kind, id });
    path.set([...parentIds, id]);
  }

  function isOpen(key) {
    return filtering || $expanded.has(key);
  }

  function forwardCreatedrag(e) {
    dispatch('createdrag', e.detail);
  }
</script>

<aside class="region-navigator">
  <div class="nav-filter">
    <input
      class="nav-filter-input"
      type="text"
      placeholder="Filter…"
      bind:value={$filterTerm}
      aria-label="Filter navigator"
    />
  </div>

  {#if g}
    {#if isVisible(visible, g.project.id)}
      <NavigatorRow
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

    {#each g.modules as mod}
      {#if isVisible(visible, mod.id)}
        <NavigatorRow
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
          {#each mod.functions as fn_}
            {#if isVisible(visible, fn_.id)}
              <NavigatorRow
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
                {#each fn_.steps || [] as step}
                  {#if isVisible(visible, step.id)}
                    <NavigatorRow
                      kind="step"
                      name={step.name}
                      status={step.status}
                      depth={3}
                      hasChildren={false}
                      expanded={false}
                      selected={$selection.id === step.id}
                      onClick={() => selectNode('step', step.id, [g.project.id, mod.id, fn_.id])}
                    />
                  {/if}
                {/each}
              {/if}
            {/if}
          {/each}
        {/if}
      {/if}
    {/each}

    {#if g.types.some((t) => isVisible(visible, t.id))}
      <NavigatorSection title="TYPES">
        {#each g.types as tp}
          {#if isVisible(visible, tp.id)}
            <NavigatorRow
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
      </NavigatorSection>
    {/if}

    {#if g.errors.some((e) => isVisible(visible, e.id))}
      <NavigatorSection title="ERRORS">
        {#each g.errors as err}
          {#if isVisible(visible, err.id)}
            <NavigatorRow
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
      </NavigatorSection>
    {/if}
  {/if}

  {#if $paletteVisible}
    <PaletteSection on:createdrag={forwardCreatedrag} />
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
