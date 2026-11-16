/**
 * `anikit/vue`: Vue directives (no Vue import needed). Register globally with
 * `app.directive('recipe', vRecipe)` and `app.directive('anikit', vAnikit)`,
 * or locally in `<script setup>` as `vRecipe` / `vAnikit`.
 *
 * ```vue
 * <header v-recipe="'hero-reveal'">…</header>
 * <section v-recipe="{ name: 'scroll-story', options: { children: '.card' } }">…</section>
 * <div v-anikit="(el) => animate(el, { x: [0, 100] })">…</div>
 * ```
 */
import { context, Context } from './context';
import { recipe, RecipeHandle } from './recipe';
import type { RecipeParam } from './svelte';

export type { RecipeParam };

/** The parts of a Vue directive binding these directives read. */
export interface Binding<V> {
  /** The directive's value. */
  value: V;
  /** The previous value (on update). */
  oldValue?: V | null;
}

/** A Vue object directive. */
export interface Directive<V> {
  /** Element inserted. */
  mounted(el: Element, binding: Binding<V>): void;
  /** Component updated. */
  updated(el: Element, binding: Binding<V>): void;
  /** Element removed. */
  unmounted(el: Element): void;
}

const handles = new WeakMap<Element, RecipeHandle>();
const run = (el: Element, p: RecipeParam) => handles.set(el, typeof p === 'string' ? recipe(p, el) : recipe(p.name, el, p.options));
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** `v-recipe="'hero-reveal'"` or `v-recipe="{ name, options }"`: a recipe on the element, reverted on unmount. */
export const vRecipe: Directive<RecipeParam> = {
  mounted: (el, b) => run(el, b.value),
  updated(el, b) {
    if (same(b.value, b.oldValue)) return;
    handles.get(el)?.revert();
    run(el, b.value);
  },
  unmounted(el) {
    handles.get(el)?.revert();
    handles.delete(el);
  },
};

const ctxs = new WeakMap<Element, Context>();
type Setup = (el: Element, ctx: Context) => void | (() => void);

/** `v-anikit="(el) => …"`: run setup in a `context()` scoped to the element after mount; reverted on unmount. */
export const vAnikit: Directive<Setup> = {
  mounted: (el, b) => { ctxs.set(el, context((c) => b.value(el, c), el)); },
  updated(el, b) {
    if (b.value === b.oldValue) return;
    ctxs.get(el)?.revert();
    ctxs.set(el, context((c) => b.value(el, c), el));
  },
  unmounted(el) {
    ctxs.get(el)?.revert();
    ctxs.delete(el);
  },
};
