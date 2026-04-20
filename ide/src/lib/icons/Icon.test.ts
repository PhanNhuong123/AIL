import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/svelte';
import Icon from './Icon.svelte';

describe('Icon.svelte', () => {
  it('renders an svg element with correct data-testid', () => {
    const { container } = render(Icon, { props: { name: 'check' } });
    const svg = container.querySelector('[data-testid="icon-check"]');
    expect(svg).not.toBeNull();
    expect(svg?.tagName.toLowerCase()).toBe('svg');
  });

  it('applies custom size prop to width and height', () => {
    const { container } = render(Icon, { props: { name: 'warn', size: 20 } });
    const svg = container.querySelector('svg');
    expect(svg?.getAttribute('width')).toBe('20');
    expect(svg?.getAttribute('height')).toBe('20');
  });

  it('uses currentColor stroke (no fill)', () => {
    const { container } = render(Icon, { props: { name: 'project' } });
    const svg = container.querySelector('svg');
    expect(svg?.getAttribute('stroke')).toBe('currentColor');
    expect(svg?.getAttribute('fill')).toBe('none');
  });
});
