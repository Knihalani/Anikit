/**
 * Make CSS list values interpolable when they are written differently:
 *
 * - **Filter function lists** (`filter`, `backdrop-filter`): functions missing
 *   from one keyframe are added with their identity value, in a common order;
 *   `150%` becomes `1.5`, a bare `0` gets its unit.
 * - **Shadow lists** (`box-shadow`, `text-shadow`): every shadow is rewritten
 *   as `[inset] x y [blur [spread]] color` (browsers serialise the colour
 *   first), lengths get explicit units, and shorter lists are padded with
 *   transparent shadows.
 *
 * Runs once when an animation is built; returns `null` when the values aren't
 * lists of either kind (the caller then switches discretely, as before).
 */

/** Identity value of each filter function (what "no filter" means). */
const FILTER_IDENTITY: Record<string, string> = {
  blur: '0px', brightness: '1', contrast: '1', grayscale: '0', 'hue-rotate': '0deg', invert: '0',
  opacity: '1', saturate: '1', sepia: '0', 'drop-shadow': '0px 0px 0px rgba(0,0,0,0)',
};
const PERCENT_FILTERS = new Set(['brightness', 'contrast', 'grayscale', 'invert', 'opacity', 'saturate', 'sepia']);

/** Split on a separator at parenthesis depth 0. */
function splitTop(s: string, sep: RegExp): string[] {
  const out: string[] = [];
  let depth = 0;
  let cur = '';
  for (const ch of s) {
    if (ch === '(') depth++;
    else if (ch === ')') depth--;
    if (depth === 0 && sep.test(ch)) {
      if (cur.trim()) out.push(cur.trim());
      cur = '';
    } else cur += ch;
  }
  if (cur.trim()) out.push(cur.trim());
  return out;
}

const isNone = (s: string) => s === 'none' || s === '';

// ---------------------------------------------------------------------------
// Filter lists
// ---------------------------------------------------------------------------

type Fn = { name: string; arg: string };

function parseFilters(s: string): Fn[] | null {
  if (isNone(s)) return [];
  const out: Fn[] = [];
  for (const part of splitTop(s, /\s/)) {
    const m = /^([a-z-]+)\((.*)\)$/i.exec(part);
    if (!m) return null;
    const name = m[1].toLowerCase();
    if (!(name in FILTER_IDENTITY)) return null;
    out.push({ name, arg: m[2].trim() });
  }
  return out;
}

function normFilterArg(name: string, arg: string): string {
  if (arg === '') return FILTER_IDENTITY[name];
  if (name === 'drop-shadow') {
    // Same shape as the identity: three lengths, then the colour.
    const sh = parseShadow(arg);
    if (!sh || sh.inset || sh.lengths.length > 3) return arg;
    while (sh.lengths.length < 3) sh.lengths.push('0px');
    return sh.lengths.join(' ') + ' ' + (sh.color === 'currentcolor' ? 'currentcolor' : sh.color);
  }
  const m = /^([+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)([a-z%]*)$/i.exec(arg);
  if (!m) return arg;
  const n = parseFloat(m[1]);
  const unit = m[2];
  if (PERCENT_FILTERS.has(name)) return String(unit === '%' ? n / 100 : n);
  if (name === 'blur') return n + (unit || 'px');
  if (name === 'hue-rotate') {
    const deg = unit === 'rad' ? (n * 180) / Math.PI : unit === 'turn' ? n * 360 : unit === 'grad' ? n * 0.9 : n;
    return deg + 'deg';
  }
  return arg;
}

/** Merge function-name sequences, keeping each list's order. Null on conflicts. */
function mergeOrder(lists: string[][]): string[] | null {
  const out: string[] = [];
  for (const l of lists) {
    if (new Set(l).size !== l.length) return null; // repeated functions: leave alone
    let pos = 0;
    for (let i = 0; i < l.length; i++) {
      const at = out.indexOf(l[i]);
      if (at !== -1) {
        if (at < pos) return null; // order conflict
        pos = at + 1;
        continue;
      }
      // New name: just before the next name of this list that is already
      // placed, or at the end.
      let next = -1;
      for (let j = i + 1; j < l.length && next === -1; j++) next = out.indexOf(l[j]);
      const ins = next === -1 ? out.length : next;
      out.splice(ins, 0, l[i]);
      pos = ins + 1;
    }
  }
  return out;
}

function harmonizeFilters(strs: string[]): string[] | null {
  const lists = strs.map(parseFilters);
  if (lists.some((l) => l === null)) return null;
  const fl = lists as Fn[][];
  if (fl.every((l) => l.length === 0)) return null;
  const order = mergeOrder(fl.map((l) => l.map((f) => f.name)));
  if (!order) return null;
  return fl.map((l) =>
    order
      .map((name) => {
        const f = l.find((x) => x.name === name);
        return name + '(' + normFilterArg(name, f ? f.arg : FILTER_IDENTITY[name]) + ')';
      })
      .join(' '),
  );
}

// ---------------------------------------------------------------------------
// Shadow lists
// ---------------------------------------------------------------------------

type Shadow = { inset: boolean; lengths: string[]; color: string };

const LENGTH = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?([a-z%]*)$/i;

function parseShadow(s: string): Shadow | null {
  let inset = false;
  const lengths: string[] = [];
  let color = '';
  let colorAt = 0;
  for (const tok of splitTop(s, /\s/)) {
    if (tok.toLowerCase() === 'inset') {
      inset = true;
      continue;
    }
    const m = LENGTH.exec(tok);
    if (m) {
      if (m[1] && !/^(px|em|rem|vw|vh|vmin|vmax|ch|ex|cm|mm|in|pt|pc|%)$/i.test(m[1])) return null;
      if (color && colorAt > 0) return null; // lengths must be contiguous
      lengths.push(m[1] ? tok : parseFloat(tok) + 'px');
      continue;
    }
    if (color) return null;
    color = tok;
    colorAt = lengths.length;
  }
  if (lengths.length < 2 || lengths.length > 4) return null;
  return { inset, lengths, color: color || 'currentcolor' };
}

function harmonizeShadows(strs: string[]): string[] | null {
  const lists: Shadow[][] = [];
  for (const s of strs) {
    if (isNone(s)) {
      lists.push([]);
      continue;
    }
    const items = splitTop(s, /,/).map(parseShadow);
    if (items.some((x) => x === null)) return null;
    lists.push(items as Shadow[]);
  }
  if (lists.every((l) => l.length === 0)) return null;
  const count = Math.max(...lists.map((l) => l.length));
  // Per position: inset must agree; lengths padded to the longest.
  const inset: boolean[] = [];
  const len: number[] = [];
  for (let i = 0; i < count; i++) {
    let ins: boolean | null = null;
    let n = 2;
    for (const l of lists) {
      const it = l[i];
      if (!it) continue;
      if (ins !== null && ins !== it.inset) return null;
      ins = it.inset;
      n = Math.max(n, it.lengths.length);
    }
    inset.push(!!ins);
    len.push(n);
  }
  return lists.map((l) => {
    const out: string[] = [];
    for (let i = 0; i < count; i++) {
      const it = l[i];
      const lengths = it ? it.lengths.slice() : [];
      while (lengths.length < len[i]) lengths.push('0px');
      const color = it ? (it.color === 'currentcolor' ? 'currentcolor' : it.color) : 'rgba(0,0,0,0)';
      out.push((inset[i] ? 'inset ' : '') + lengths.join(' ') + ' ' + color);
    }
    return out.join(', ');
  });
}

/** Rewrite keyframe strings into one interpolable shape, or `null`. */
export function harmonize(strs: string[]): string[] | null {
  const t = strs.map((s) => s.trim());
  return harmonizeFilters(t) ?? harmonizeShadows(t);
}
