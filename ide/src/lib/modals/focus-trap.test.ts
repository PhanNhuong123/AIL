import { describe, it, expect, beforeEach } from 'vitest';
import { focusTrap } from './focus-trap';

describe('focusTrap action', () => {
  let dialog: HTMLDivElement;
  let outside: HTMLButtonElement;

  beforeEach(() => {
    document.body.innerHTML = '';
    outside = document.createElement('button');
    outside.id = 'outside';
    document.body.appendChild(outside);

    dialog = document.createElement('div');
    dialog.tabIndex = -1;
    dialog.innerHTML = `
      <button id="a">A</button>
      <input id="b" />
      <button id="c">C</button>
    `;
    document.body.appendChild(dialog);
  });

  function dispatch(key: string, shiftKey = false) {
    dialog.dispatchEvent(
      new KeyboardEvent('keydown', { key, shiftKey, bubbles: true, cancelable: true }),
    );
  }

  it('pulls initial focus to the first focusable on mount', async () => {
    focusTrap(dialog);
    await Promise.resolve(); // flush queueMicrotask
    expect(document.activeElement?.id).toBe('a');
  });

  it('Tab from the last focusable wraps to the first', async () => {
    focusTrap(dialog);
    await Promise.resolve();
    (dialog.querySelector('#c') as HTMLElement).focus();
    expect(document.activeElement?.id).toBe('c');
    dispatch('Tab');
    expect(document.activeElement?.id).toBe('a');
  });

  it('Shift+Tab from the first focusable wraps to the last', async () => {
    focusTrap(dialog);
    await Promise.resolve();
    (dialog.querySelector('#a') as HTMLElement).focus();
    expect(document.activeElement?.id).toBe('a');
    dispatch('Tab', true);
    expect(document.activeElement?.id).toBe('c');
  });

  it('Tab in the middle of the cycle does not interfere with the browser default', async () => {
    focusTrap(dialog);
    await Promise.resolve();
    (dialog.querySelector('#a') as HTMLElement).focus();
    // Forward Tab from #a should let the browser advance naturally — i.e.
    // the action should NOT preventDefault and NOT manually move focus.
    let prevented = false;
    dialog.addEventListener(
      'keydown',
      (e) => {
        if (e.defaultPrevented) prevented = true;
      },
      { once: true },
    );
    dispatch('Tab');
    expect(prevented).toBe(false);
  });

  it('Tab when focus has escaped the dialog pulls it back to the first', async () => {
    focusTrap(dialog);
    await Promise.resolve();
    outside.focus();
    expect(document.activeElement?.id).toBe('outside');
    dispatch('Tab');
    expect(document.activeElement?.id).toBe('a');
  });

  it('non-Tab keys do not move focus', async () => {
    focusTrap(dialog);
    await Promise.resolve();
    (dialog.querySelector('#a') as HTMLElement).focus();
    dispatch('Escape');
    dispatch('Enter');
    dispatch('ArrowDown');
    expect(document.activeElement?.id).toBe('a');
  });

  it('destroy() removes the keydown listener', async () => {
    const handle = focusTrap(dialog);
    await Promise.resolve();
    handle.destroy();
    (dialog.querySelector('#c') as HTMLElement).focus();
    dispatch('Tab');
    // After destroy the trap should not wrap; focus stays on #c (or moves
    // naturally per browser default — tested via "no wrap" not "specific id").
    expect(document.activeElement?.id).toBe('c');
  });

  it('falls back to the dialog itself when no focusables are present', async () => {
    dialog.innerHTML = '';
    focusTrap(dialog);
    await Promise.resolve();
    expect(document.activeElement).toBe(dialog);
    dispatch('Tab');
    expect(document.activeElement).toBe(dialog);
  });
});
