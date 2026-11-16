/**
 * `anikit/svelte`: Svelte actions (no Svelte import needed). Everything an
 * action creates is reverted when the element is destroyed.
 *
 * ```svelte
 * <header use:recipe={'hero-reveal'}>…</header>
 * <section use:recipe={{ name: 'scroll-story', options: { children: '.card' } }}>…</section>
 * <div use:anikit={(el) => { animate(el, { x: [0, 100] }); }}>…</div>
 * ```
 */
import { context } from './context';
import { recipe as runRecipe, RecipeName, RecipeOptions } from './recipe';
import type { Context } from './context';

/** What a Svelte action returns. */
export interface ActionReturn<P> {
  /** Called when the parameter changes. */
  update?(param: P): void;
  /** Called when the element is removed. */
  destroy?(): void;
}

/** A recipe name, or `{ name, options }`. */
export type RecipeParam = RecipeName | { name: RecipeName; options?: RecipeOptions };

/** `use:recipe={'hero-reveal'}`: apply a recipe to the element; re-applied when the parameter changes. */
export function recipe(node: Element, param: RecipeParam): ActionReturn<RecipeParam> {
  const run = (p: RecipeParam) => (typeof p === 'string' ? runRecipe(p, node) : runRecipe(p.name, node, p.options));
  let h = run(param);
  return {
    update(p) { h.revert(); h = run(p); },
    destroy() { h.revert(); },
  };
}

/** `use:anikit={(el) => …}`: run setup in a `context()` scoped to the element; reverted on destroy. */
export function anikit(node: Element, setup: (el: Element, ctx: Context) => void | (() => void)): ActionReturn<(el: Element, ctx: Context) => void | (() => void)> {
  let ctx = context((c) => setup(node, c), node);
  return {
    update(fn) { ctx.revert(); ctx = context((c) => fn(node, c), node); },
    destroy() { ctx.revert(); },
  };
}
