import { describe, it, expect, beforeAll, afterEach } from 'vitest';
import { recipe, recipeNames, testing, animate, onWarning } from '../src/index';
import { setup, advance } from './setup';

beforeAll(setup);
afterEach(() => onWarning(null));

const mount = (html: string) => {
  const root = document.createElement('div');
  root.innerHTML = html;
  document.body.appendChild(root);
  return root;
};

describe('recipe()', () => {
  it('lists its recipes', () => {
    expect(recipeNames()).toEqual(expect.arrayContaining(['hero-reveal', 'scroll-story', 'magnetic-buttons', 'text-scramble', 'parallax-gallery']));
  });

  it('hero-reveal splits the headline, animates it in and reverts the markup', async () => {
    const root = mount('<section class="hero"><h1>Hello big world</h1><p>Sub</p></section>');
    const hero = root.querySelector('.hero')!;
    const h = recipe('hero-reveal', hero);
    expect(h.animations.length).toBe(1);
    expect(root.querySelectorAll('h1 span').length).toBeGreaterThan(2);
    await Promise.resolve(); // timelines start on the next microtask
    advance(3);
    expect(parseFloat(root.querySelector('p')!.style.opacity)).toBeGreaterThan(0.99);
    h.revert();
    expect(root.querySelector('h1')!.innerHTML).toBe('Hello big world');
    root.remove();
  });

  it('text-scramble ends on the requested text', () => {
    const root = mount('<h2 id="t">old</h2>');
    recipe('text-scramble', '#t', { text: 'DECRYPTED', duration: 0.5 });
    advance(1);
    expect(root.querySelector('#t')!.textContent).toBe('DECRYPTED');
    root.remove();
  });

  it('count-up counts to the number in the element when started directly', () => {
    const root = mount('<b id="n">1,200</b>');
    recipe('count-up', '#n', { onScroll: false, duration: 0.5 });
    advance(1);
    expect(root.querySelector('#n')!.textContent).toBe((1200).toLocaleString());
    root.remove();
  });

  it('stagger-in and scroll-story animate every child without scroll when onScroll is false', () => {
    const root = mount('<ul class="l"><li>a</li><li>b</li></ul><main><section>1</section><section>2</section></main>');
    recipe('stagger-in', '.l', { onScroll: false });
    const story = recipe('scroll-story', 'main', { onScroll: false });
    expect(story.animations.length).toBe(2);
    advance(2);
    for (const li of root.querySelectorAll('li')) expect((li as HTMLElement).style.opacity).toBe('1');
    root.remove();
  });

  it('pointer and scroll recipes start and revert cleanly', () => {
    const root = mount('<div class="g"><img><img></div><nav><button>Go</button></nav><svg><path d="M0 0L10 10"/></svg><div class="card"></div>');
    const hs = [recipe('magnetic-buttons', 'nav'), recipe('parallax-gallery', '.g'), recipe('draw-svg', 'svg'), recipe('tilt-cards', '.card'), recipe('scroll-story', '.g')];
    advance(0.5);
    for (const h of hs) h.revert();
    root.remove();
  });

  it('throws on unknown names with a suggestion and warns on unknown options with a doc link', () => {
    expect(() => recipe('hero-reveel' as any, document.body)).toThrow(/Did you mean "hero-reveal"\?.*docs\/api\.md#recipes-recipe/);
    const warnings = testing.captureWarnings();
    const root = mount('<h1 id="x">Hi</h1>');
    recipe('text-scramble', '#x', { duraton: 1 } as any);
    expect(warnings.join('\n')).toMatch(/unknown option "duraton".*Did you mean "duration"\?.*See docs\/api\.md#recipes-recipe/);
    animate({ a: 0 }, { a: 1 }, { durration: 1 } as any);
    expect(warnings.join('\n')).toMatch(/durration.*See docs\/api\.md#animate/);
    root.remove();
  });
});

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { DOC_ANCHORS } from '../src/debug';
describe('doc links in warnings', () => {
  it('point at headings that exist in docs/api.md', () => {
    const md = readFileSync(join(__dirname, '../../../docs/api.md'), 'utf8');
    const slugs = new Set(md.split('\n').filter((l) => /^#{2,3} /.test(l)).map((l) =>
      l.replace(/^#+ /, '').toLowerCase().replace(/[^\p{L}\p{N} _-]/gu, '').replace(/ /g, '-')));
    const missing = Object.entries(DOC_ANCHORS).filter(([, a]) => !slugs.has(a));
    expect(missing).toEqual([]);
  });
});
