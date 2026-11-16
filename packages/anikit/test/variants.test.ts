import { describe, it, expect, beforeAll } from 'vitest';
import { variants, customBounce, customWiggle, testing, onWarning } from '../src/index';
import { setup, advance } from './setup';

beforeAll(setup);
const tick = () => Promise.resolve();

const mount = (html: string) => {
  const root = document.createElement('div');
  root.innerHTML = html;
  document.body.appendChild(root);
  return root;
};

describe('variants()', () => {
  it('applies the initial state, animates by name and orchestrates children', async () => {
    const root = mount('<ul class="m"><li>a</li><li>b</li><li>c</li></ul>');
    const items = variants('.m li', { closed: { opacity: 0 }, open: { opacity: 1, transition: { duration: 0.2 } } }, { initial: 'closed' });
    const menu = variants('.m', {
      closed: { opacity: 0 },
      open: { opacity: 1, transition: { duration: 0.2, when: 'beforeChildren', staggerChildren: 0.1 } },
    }, { initial: 'closed', children: items });
    const lis = Array.from(root.querySelectorAll('li')) as HTMLElement[];
    expect(lis[0].style.opacity).toBe('0');
    const tl = menu.animate('open');
    expect(tl.duration).toBeCloseTo(0.6, 2); // parent 0.2, then kids at 0.2, 0.3, 0.4 (0.2 each)
    await tick();
    advance(0.25);
    expect(parseFloat((root.querySelector('.m') as HTMLElement).style.opacity)).toBeCloseTo(1, 2);
    expect(parseFloat(lis[2].style.opacity || '0')).toBeLessThan(0.1);
    advance(1);
    for (const li of lis) expect(parseFloat(li.style.opacity)).toBeCloseTo(1, 2);
    expect(menu.current).toBe('open');
    expect(items.current).toBe('open');
    menu.revert();
    items.revert();
    expect(lis[0].getAttribute('style')).toBeNull();
    root.remove();
  });

  it('afterChildren runs the parent last; function variants get the index', async () => {
    const root = mount('<div class="p"><i></i><i></i></div>');
    const kids = variants('.p i', { on: (_el, i) => ({ x: (i + 1) * 10, transition: { duration: 0.2 } }) });
    const p = variants('.p', { on: { opacity: 0.5, transition: { duration: 0.2, when: 'afterChildren' } } }, { children: kids });
    const tl = p.animate('on');
    expect(tl.duration).toBeCloseTo(0.4, 2);
    await tick();
    advance(1);
    const is = root.querySelectorAll('i');
    expect((is[1] as HTMLElement).style.transform).toContain('20px');
    root.remove();
  });

  it('warns on unknown states and transition options', () => {
    const w = testing.captureWarnings();
    const root = mount('<b class="v"></b>');
    const v = variants('.v', { shown: { opacity: 1, transition: { durration: 1 } as any } });
    v.animate('shwon');
    expect(w.join('\n')).toMatch(/durration.*Did you mean "duration"/);
    expect(w.join('\n')).toMatch(/unknown state "shwon".*Did you mean "shown"/);
    onWarning(null);
    root.remove();
  });
});

describe('customBounce / customWiggle', () => {
  it('bounce lands at 1, rebounds and settles; squash pulses at impacts', () => {
    const { ease, squash } = customBounce({ strength: 0.5 });
    expect(ease(0)).toBe(0);
    expect(ease(1)).toBe(1);
    const total = 1 + 2 * (0.5 + 0.25 + 0.125 + 0.0625 + 0.03125 + 0.015625 + 0.0078125);
    expect(ease(1 / total)).toBeCloseTo(1, 5);       // first impact
    expect(ease(1.5 / total)).toBeCloseTo(0.75, 5);  // top of first rebound: 1 - 0.5²
    expect(squash(1 / total)).toBeCloseTo(1, 5);
    expect(squash(1.5 / total)).toBe(0);
    expect(customBounce({ endAtStart: true }).ease(0)).toBe(1);
  });

  it('wiggle oscillates and returns to 0 for each type', () => {
    for (const type of ['easeOut', 'easeInOut', 'anticipate', 'uniform', 'random'] as const) {
      const e = customWiggle(4, { type });
      expect(e(0)).toBe(0);
      expect(e(1)).toBe(0);
      const xs = Array.from({ length: 99 }, (_, i) => e((i + 1) / 100));
      expect(Math.max(...xs)).toBeGreaterThan(0.05);
      expect(Math.min(...xs)).toBeLessThan(-0.05);
    }
  });
});
