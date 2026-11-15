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
