/**
 * Developer (and coding-agent) experience: actionable warnings with
 * "did you mean" suggestions, a JSON snapshot of everything running, and
 * deterministic testing helpers.
 */

const G = globalThis as any;
let enabled = !(typeof G.process !== 'undefined' && G.process?.env?.NODE_ENV === 'production');
const seen = new Set<string>();
let sink: ((msg: string) => void) | null = null;

/** Turn diagnostics on/off (default: on, except when NODE_ENV=production). */
export function setWarnings(on: boolean): void {
  enabled = on;
}
/** Route warnings somewhere else (e.g. collect them in tests). `null` restores console.warn. */
export function onWarning(fn: ((msg: string) => void) | null): void {
  sink = fn;
}

/** Whether warnings are on (cheap guard so validation costs nothing in production). */
export function warningsOn(): boolean {
  return enabled;
}
/** Forget which messages were already shown (each unique message is shown once). */
export function resetWarnings(): void {
  seen.clear();
}

/** Warn once per unique message. */
export function warn(msg: string): void {
  if (!enabled || seen.has(msg)) return;
  seen.add(msg);
  const full = 'anikit: ' + msg;
  if (sink) sink(full);
  else if (typeof console !== 'undefined') console.warn(full);
}

/** Levenshtein distance (small strings). */
function dist(a: string, b: string): number {
  a = a.toLowerCase();
  b = b.toLowerCase();
  const m = a.length;
  const n = b.length;
  const d = Array.from({ length: n + 1 }, (_, j) => j);
  for (let i = 1; i <= m; i++) {
    let prev = d[0];
    d[0] = i;
    for (let j = 1; j <= n; j++) {
      const tmp = d[j];
      d[j] = Math.min(d[j] + 1, d[j - 1] + 1, prev + (a[i - 1] === b[j - 1] ? 0 : 1));
      prev = tmp;
    }
  }
  return d[n];
}

/** Closest candidate within a sensible edit distance. */
export function suggest(input: string, candidates: Iterable<string>): string | undefined {
  let best: string | undefined;
  let bd = Infinity;
  const limit = Math.max(2, Math.floor(input.length / 3));
  for (const c of candidates) {
    const d = dist(input, c);
    if (d < bd) {
      bd = d;
      best = c;
    }
  }
  return bd <= limit ? best : undefined;
}

export const didYouMean = (input: string, candidates: Iterable<string>): string => {
  const s = suggest(input, candidates);
  return s ? ` Did you mean "${s}"?` : '';
};

/** Every easing name the parser understands (for suggestions). */
export const EASING_NAMES: string[] = (() => {
  const curves = ['Quad', 'Cubic', 'Quart', 'Quint', 'Sine', 'Expo', 'Circ', 'Back', 'Elastic', 'Bounce'];
  const out = ['linear', 'ease', 'ease-in', 'ease-out', 'ease-in-out', 'step-start', 'step-end', 'spring', 'anticipate', 'wiggle', 'shake', 'slowmo', 'rough', 'expoScale', 'gentle', 'wobbly', 'stiff', 'slow', 'molasses', 'snappy', 'bouncy'];
  for (const c of curves) for (const m of ['in', 'out', 'inOut', 'outIn']) out.push(m + c);
  for (let p = 1; p <= 4; p++) for (const m of ['in', 'out', 'inOut']) out.push(`power${p}.${m}`);
  return out;
})();

/** Short human description of a target for messages / inspect(). */
export function describe(t: unknown): string {
  if (typeof Element !== 'undefined' && t instanceof Element) {
    let s = t.tagName.toLowerCase();
    if (t.id) s += '#' + t.id;
    if (typeof t.className === 'string' && t.className.trim()) s += '.' + t.className.trim().split(/\s+/).slice(0, 2).join('.');
    return s;
  }
  if (t && typeof t === 'object') return 'Object{' + Object.keys(t).slice(0, 4).join(',') + '}';
  return String(t);
}

let cssProps: string[] | null = null;
/** Known CSS property names (camelCase) for suggestions. */
export function cssPropertyNames(): string[] {
  if (cssProps) return cssProps;
  cssProps = [];
  if (typeof document !== 'undefined') {
    const st = document.createElement('div').style as any;
    for (const k in st) if (typeof st[k] === 'string' && !/^\d/.test(k)) cssProps.push(k);
  }
  return cssProps;
}

/**
 * Every key of an options interface, checked by the compiler in both
 * directions: `const KEYS: OptionKeys<TimelineOptions> = { delay: 0, … }`
 * fails to compile when a key is missing or unknown, so lists can't drift.
 */
export type OptionKeys<T> = { readonly [K in keyof Required<T>]: 0 };

/**
 * Warn about option keys `api` doesn't know (`checkKeys('timeline()', opts, KEYS)`),
 * with a "did you mean" suggestion. Only runs while warnings are on.
 */
export function checkKeys(api: string, opts: unknown, ...known: Array<Readonly<Record<string, 0>>>): void {
  if (!enabled || !opts || typeof opts !== 'object' || Array.isArray(opts)) return;
  for (const k of Object.keys(opts)) {
    if (known.some((o) => Object.prototype.hasOwnProperty.call(o, k))) continue;
    warn(`${api}: unknown option "${k}"; it is ignored.${didYouMean(k, known.flatMap((o) => Object.keys(o)))}`);
  }
}

/**
 * Times in anikit are seconds. Flag values that look like milliseconds
 * (`duration: 600`), the most common mistake when coming from other libraries.
 */
export function checkSeconds(api: string, name: string, v: unknown, limit = 100): void {
  if (enabled && typeof v === 'number' && v >= limit && v !== Infinity) {
    warn(`${api}: ${name} is ${v} seconds. Times in anikit are in seconds; did you mean ${name}: ${+(v / 1000).toFixed(3)}?`);
  }
}

/** Explain an empty target list: `null` (a querySelector that found nothing), a selector, or an empty list. */
export function checkTargets(api: string, targets: unknown, found: number): void {
  if (!enabled || found > 0) return;
  if (targets === null || targets === undefined) {
    warn(`${api} received ${targets} as its target, so nothing was animated. Did a querySelector() or ref return ${targets}? Pass an element, a selector string, a list, or a plain object.`);
  } else if (typeof targets === 'string') {
    warn(`${api}: selector "${targets}" matched no elements, so nothing was animated.${selectorHint(targets)}`);
  } else {
    warn(`${api} received an empty target list, so nothing was animated.`);
  }
}

/** Why a selector might have matched nothing, and what would have. */
export function selectorHint(sel: string): string {
  if (typeof document === 'undefined') return ' There is no `document` here: selectors only work with a DOM. Pass objects instead.';
  if (/^[A-Za-z][\w-]*$/.test(sel)) {
    for (const alt of ['.' + sel, '#' + sel]) {
      try {
        if (document.querySelector(alt)) return ` Did you mean "${alt}"?`;
      } catch {
        /* not a valid selector either */
      }
    }
  }
  if (document.readyState === 'loading') {
    return ' The document is still loading: run this after DOMContentLoaded (or from a module script, which is deferred).';
  }
  return '';
}

/** `querySelectorAll` with an actionable error for invalid selectors. */
export function queryAll(root: ParentNode, sel: string): Element[] {
  try {
    return Array.from(root.querySelectorAll(sel));
  } catch {
    throw new Error(`anikit: "${sel}" is not a valid CSS selector. Pass an element, a list of elements, or a selector such as ".card" or "#hero".`);
  }
}
