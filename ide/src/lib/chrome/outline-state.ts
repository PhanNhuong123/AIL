import { writable, type Writable } from 'svelte/store';

export const expanded: Writable<Set<string>> = writable(new Set(['project:root']));
export const filterTerm: Writable<string> = writable('');

export function toggleExpanded(key: string): void {
  expanded.update((set) => {
    const next = new Set(set);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    return next;
  });
}
