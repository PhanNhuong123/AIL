import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/svelte';
import TrafficLights from './TrafficLights.svelte';

describe('TrafficLights.svelte', () => {
  it('renders mac variant by default with three platform-color buttons', () => {
    const { container } = render(TrafficLights);
    const wrapper = container.querySelector('[data-testid="traffic-lights-mac"]');
    expect(wrapper).not.toBeNull();
    const buttons = wrapper?.querySelectorAll('button');
    expect(buttons?.length).toBe(3);
    const labels = Array.from(buttons ?? []).map((b) => b.getAttribute('aria-label'));
    expect(labels).toEqual(['Close', 'Minimize', 'Maximize']);
  });

  it('renders generic variant when variant="generic"', () => {
    const { container } = render(TrafficLights, { props: { variant: 'generic' } });
    const wrapper = container.querySelector('[data-testid="traffic-lights-generic"]');
    expect(wrapper).not.toBeNull();
    const buttons = wrapper?.querySelectorAll('button');
    expect(buttons?.length).toBe(3);
    const labels = Array.from(buttons ?? []).map((b) => b.getAttribute('aria-label'));
    expect(labels).toEqual(['Close', 'Minimize', 'Maximize']);
  });

  it('mac variant buttons have inline background style (platform hex colors)', () => {
    const { container } = render(TrafficLights);
    const buttons = container.querySelectorAll('[data-testid="traffic-lights-mac"] button');
    expect(buttons.length).toBe(3);
    for (const btn of Array.from(buttons)) {
      expect(btn.getAttribute('style')).toContain('background:');
    }
  });

  it('generic variant buttons have no inline background style', () => {
    const { container } = render(TrafficLights, { props: { variant: 'generic' } });
    const buttons = container.querySelectorAll('[data-testid="traffic-lights-generic"] button');
    expect(buttons.length).toBe(3);
    for (const btn of Array.from(buttons)) {
      const style = btn.getAttribute('style') ?? '';
      expect(style).not.toContain('background:');
    }
  });
});
