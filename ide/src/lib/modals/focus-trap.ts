// WCAG 2.1 SC 2.4.3 — focus management for modal dialogs.
//
// `aria-modal="true"` is a semantic hint only; assistive technologies use it
// but the browser does not enforce focus boundaries. Without an active trap,
// pressing Tab from the last focusable inside the modal lands on
// `document.body` and the user can interact with the chrome behind the
// backdrop while the modal still claims `aria-modal`.
//
// This action installs a Tab/Shift+Tab interceptor on the dialog root and
// pulls initial focus into the dialog on mount. It is DOM-only — no Svelte
// store / no Tauri bridge — so it stays compatible with the modals/CLAUDE.md
// rule that modal components must NOT import `$lib/bridge.ts`.

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

function getFocusables(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
    .filter((el) => !el.hasAttribute('disabled') && el.tabIndex !== -1);
}

export function focusTrap(node: HTMLElement) {
  function handleKeydown(e: KeyboardEvent) {
    if (e.key !== 'Tab') return;
    const focusables = getFocusables(node);
    if (focusables.length === 0) {
      // Nothing to cycle; keep focus on the dialog itself.
      e.preventDefault();
      node.focus();
      return;
    }
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    const active = document.activeElement as HTMLElement | null;
    // If focus has somehow escaped the dialog, pull it back to the first
    // focusable on the next Tab — this is the recovery path for the rare
    // case when an outside focus listener stole focus during the modal life.
    if (!active || !node.contains(active)) {
      e.preventDefault();
      first.focus();
      return;
    }
    if (e.shiftKey) {
      if (active === first) {
        e.preventDefault();
        last.focus();
      }
    } else {
      if (active === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  // Initial focus: prefer the first focusable; fall back to the dialog itself
  // (which has `tabindex="-1"` to accept programmatic focus).
  const focusables = getFocusables(node);
  const initial = focusables[0] ?? node;
  // Defer one microtask so callers that mount + immediately programmatically
  // focus another element (rare) still win.
  queueMicrotask(() => initial.focus());

  node.addEventListener('keydown', handleKeydown);

  return {
    destroy() {
      node.removeEventListener('keydown', handleKeydown);
    },
  };
}
