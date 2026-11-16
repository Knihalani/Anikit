import { describe, it, expect, beforeAll } from 'vitest';
import { createElement as h, act, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { useAnimate, useAnikit, useRecipe, useSpring, useMotionValue, useTransform, Presence } from '../src/react';
import { animate } from '../src/index';
import type { MotionValue } from '../src/index';
import { setup, advance } from './setup';

beforeAll(setup);
(globalThis as any).IS_REACT_ACT_ENVIRONMENT = true;

function mount(node: any) {
  const div = document.createElement('div');
  document.body.appendChild(div);
  const root = createRoot(div);
  act(() => root.render(node));
  return { div, root, unmount: () => { act(() => root.unmount()); div.remove(); } };
}

describe('anikit/react', () => {
  it('useAnikit scopes selectors and reverts on unmount', () => {
    function C() {
      const scope = useAnikit(() => { animate('.b', { opacity: [0, 0.5] }, { duration: 0.2 }); });
      return h('div', { ref: scope }, h('div', { className: 'b' }));
    }
    const outside = document.createElement('div');
    outside.className = 'b';
    document.body.appendChild(outside);
    const m = mount(h(C));
    advance(0.5);
    const b = m.div.querySelector('.b') as HTMLElement;
    expect(b.style.opacity).toBe('0.5');
    expect(outside.style.opacity).toBe('');
    m.unmount();
    expect(b.style.opacity).toBe('');
    outside.remove();
  });

  it('useAnimate animates on demand', () => {
    let run: any, ref: any;
    function C() { [ref, run] = useAnimate(); return h('div', { ref }); }
    const m = mount(h(C));
    run(ref.current, { x: [0, 50] }, { duration: 0.2 });
    advance(0.5);
    expect(ref.current.style.transform).toContain('50px');
    m.unmount();
  });

  it('useSpring follows its target; useTransform maps it', () => {
    let s!: MotionValue, t!: MotionValue, set!: (n: number) => void;
    function C() {
      const [n, setN] = useState(0);
      set = setN;
      s = useSpring(n, { stiffness: 300, damping: 30 });
      t = useTransform(s, [0, 100], [0, 1]);
      useMotionValue(3);
      return null;
    }
    const m = mount(h(C));
    act(() => set(100));
    advance(2);
    expect(s.get()).toBeCloseTo(100, 0);
    expect(t.get()).toBeCloseTo(1, 1);
    m.unmount();
  });

  it('useRecipe applies and reverts a recipe', () => {
    function C() {
      const r = useRef<HTMLDivElement>(null);
      useRecipe('text-scramble', r, { text: 'DONE', duration: 0.3 });
      return h('div', { ref: r }, 'xxxx');
    }
    const m = mount(h(C));
    advance(1);
    expect(m.div.textContent).toBe('DONE');
    m.unmount();
  });

  it('Presence keeps a removed child until its exit finishes', async () => {
    let set!: (b: boolean) => void;
    function C() {
      const [on, setOn] = useState(true);
      set = setOn;
      return h(Presence, { exit: { opacity: 0 }, options: { duration: 0.3 } }, on ? h('p', { key: 'a' }, 'hi') : null);
    }
    const m = mount(h(C));
    expect(m.div.querySelector('p')).not.toBeNull();
    act(() => set(false));
    expect(m.div.querySelector('p')).not.toBeNull();
    advance(0.6);
    await act(async () => { await Promise.resolve(); await Promise.resolve(); });
    expect(m.div.querySelector('p')).toBeNull();
    act(() => set(true));
    expect(m.div.querySelector('p')).not.toBeNull();
    m.unmount();
  });
});
