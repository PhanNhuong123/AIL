import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Source-level assertion that the global :focus-visible rule exists in
// tokens.css. Browsers' default outline (~0.5px gray) is invisible against
// dark surfaces; this rule is required for WCAG 2.4.7 compliance and is
// load-bearing for keyboard-only navigation across the IDE shell.

describe('tokens.css :focus-visible global rule (WCAG 2.4.7)', () => {
  const source = readFileSync(
    resolve(__dirname, 'tokens.css'),
    'utf-8',
  );

  it('declares a global :focus-visible rule', () => {
    expect(source).toMatch(/:focus-visible\s*\{/);
  });

  it('uses var(--accent) for the focus outline color', () => {
    const block = source.match(/:focus-visible\s*\{[^}]*\}/);
    expect(block).not.toBeNull();
    expect(block![0]).toMatch(/outline:\s*[^;]*var\(--accent\)/);
  });

  it('declares an outline-offset for breathing room around the focus ring', () => {
    const block = source.match(/:focus-visible\s*\{[^}]*\}/);
    expect(block).not.toBeNull();
    expect(block![0]).toMatch(/outline-offset:/);
  });
});
