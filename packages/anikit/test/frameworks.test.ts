import { describe, it, expect, beforeAll } from 'vitest';
import { recipe as recipeAction, anikit } from '../src/svelte';
import { vRecipe, vAnikit } from '../src/vue';
import { animate } from '../src/index';
import { setup, advance } from './setup';

beforeAll(setup);
const el = (html: string) => {
  const d = document.createElement('div');
  d.innerHTML = html;
  document.body.appendChild(d);
  return d;
};

describe('anikit/svelte actions', () => {
  it('use:recipe applies, updates and reverts', () => {
    const d = el('<h2>old</h2>');
    const h2 = d.firstElementChild!;
    const a = recipeAction(h2, { name: 'text-scramble', options: { text: 'NEW', duration: 0.2 } });
    advance(0.5);
    expect(h2.textContent).toBe('NEW');
    a.update!({ name: 'text-scramble', options: { text: 'AGAIN', duration: 0.2 } });
    advance(0.5);
    expect(h2.textContent).toBe('AGAIN');
    a.destroy!();
    d.remove();
  });
  it('use:anikit scopes and reverts', () => {
    const d = el('<b class="q"></b>');
    const a = anikit(d, () => { animate('.q', { opacity: [0, 0.4] }, { duration: 0.1 }); });
    advance(0.3);
    const q = d.querySelector('.q') as HTMLElement;
    expect(q.style.opacity).toBe('0.4');
    a.destroy!();
    expect(q.style.opacity).toBe('');
    d.remove();
  });
});

describe('anikit/vue directives', () => {
  it('v-recipe and v-anikit mount and unmount', () => {
    const d = el('<h2>x</h2><i class="q"></i>');
    const h2 = d.firstElementChild!;
    vRecipe.mounted(h2, { value: { name: 'text-scramble', options: { text: 'VUE', duration: 0.2 } } });
    vAnikit.mounted(d, { value: () => { animate('.q', { opacity: [0, 0.3] }, { duration: 0.1 }); } });
    advance(0.5);
    expect(h2.textContent).toBe('VUE');
    const q = d.querySelector('.q') as HTMLElement;
    expect(q.style.opacity).toBe('0.3');
    vAnikit.unmounted(d);
    vRecipe.unmounted(h2);
    expect(q.style.opacity).toBe('');
    d.remove();
  });
});
