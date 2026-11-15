/**
 * Contexts: collect everything created inside a function (animations,
 * timelines, scroll triggers, draggables, listeners…) so it can all be
 * reverted or killed together — ideal for component cleanup in React, Vue,
 * Svelte, etc. Also scopes selector lookups to a root element.
 *
 * ```ts
 * useEffect(() => {
 *   const ctx = context(() => {
 *     animate('.box', { x: 100 });           // '.box' only inside `root`
 *     scrollTrigger({ trigger: '.panel', animation: timeline().add(...) });
 *   }, root);
 *   return () => ctx.revert();               // restores original styles
 * }, []);
 * ```
 */
import { setSelectorRoot } from './props';

export interface Revertible {
  revert(): void;
  kill?(): void;
}

let current: Context | null = null;

/** Register an object with the active context (if any). */
export function track<T extends Revertible>(x: T): T {
  if (current) current.items.push(x);
  return x;
}

export class Context {
  items: Revertible[] = [];
  private cleanups: Array<() => void> = [];
  constructor(fn?: (ctx: Context) => void | (() => void), public scope: ParentNode | null = null) {
    if (fn) this.add(fn);
  }
  /** Run `fn` inside this context (selectors scoped, creations tracked). */
  add<R>(fn: (ctx: Context) => R): R {
    const prev = current;
    const prevRoot = setSelectorRoot(this.scope);
    current = this;
    try {
      const r = fn(this);
      if (typeof r === 'function') this.cleanups.push(r as unknown as () => void);
      return r;
    } finally {
      current = prev;
      setSelectorRoot(prevRoot);
    }
  }
  /** Revert everything (newest first): restores the styles captured at creation. */
  revert(): void {
    for (const c of this.cleanups.splice(0).reverse()) c();
    for (const x of this.items.splice(0).reverse()) x.revert();
  }
  /** Kill everything, leaving values where they are. */
  kill(): void {
    for (const c of this.cleanups.splice(0).reverse()) c();
    for (const x of this.items.splice(0).reverse()) (x.kill ?? x.revert).call(x);
  }
}

/** Create a context; see {@link Context}. */
export function context(fn?: (ctx: Context) => void | (() => void), scope?: ParentNode | string | null): Context {
  const root = typeof scope === 'string' && typeof document !== 'undefined' ? document.querySelector(scope) : (scope as ParentNode | null);
  return new Context(fn, root ?? null);
}

type Conditions = Record<string, boolean>;

/**
 * Responsive animations: run setup functions while media queries match and
 * revert them automatically when they stop matching.
 *
 * ```ts
 * const mm = matchMedia();
 * mm.add('(min-width: 800px)', () => { animate('.nav', { x: [-100, 0] }); });
 * mm.add({ wide: '(min-width: 800px)', reduce: '(prefers-reduced-motion: reduce)' }, ({ wide, reduce }) => { … });
 * ```
 */
export function matchMedia(scope?: ParentNode | null) {
  const entries: Array<{ off: () => void; ctx: Context | null }> = [];
  const api = {
    add(query: string | Record<string, string>, setup: (conditions: Conditions, ctx: Context) => void | (() => void)) {
      if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return api;
      const queries = typeof query === 'string' ? { matches: query } : query;
      const mqls = Object.entries(queries).map(([k, q]) => [k, window.matchMedia(q)] as const);
      const entry = { ctx: null as Context | null, off: () => {} };
      const run = () => {
        entry.ctx?.revert();
        entry.ctx = null;
        const cond: Conditions = {};
        for (const [k, m] of mqls) cond[k] = m.matches;
        const active = typeof query === 'string' ? cond.matches : Object.values(cond).some(Boolean);
        if (active) entry.ctx = new Context((c) => setup(cond, c), scope ?? null);
      };
      for (const [, m] of mqls) m.addEventListener('change', run);
      entry.off = () => {
        for (const [, m] of mqls) m.removeEventListener('change', run);
        entry.ctx?.revert();
      };
      entries.push(entry);
      run();
      return api;
    },
    /** Revert all and stop listening. */
    revert() {
      for (const e of entries.splice(0)) e.off();
    },
  };
  track(api);
  return api;
}
