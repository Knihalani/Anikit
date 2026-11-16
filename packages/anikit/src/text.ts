/**
 * `splitText()` — split an element's text into lines / words / characters
 * (each wrapped in an inline-block span) for per-glyph animation, keeping
 * nested markup and accessibility intact.
 *
 * ```ts
 * const { chars } = splitText('h1');
 * animate(chars, { y: ['1em', 0], opacity: [0, 1] }, { delay: stagger(0.02) });
 *
 * // Lines that slide up from behind a mask, re-split (and re-run) when the width changes:
 * splitText('.lede', {
 *   type: 'lines', mask: 'lines', autoSplit: true,
 *   onSplit: ({ lines }) => animate(lines, { yPercent: [100, 0] }, { delay: stagger(0.08), ease: 'outCubic' }),
 * });
 * ```
 */
import { Targets, toTargets } from './props';
import { checkKeys, checkTargets, warn, didYouMean } from './debug';
import type { OptionKeys } from './debug';
import { track } from './context';
import type { Playable } from './playable';

export interface SplitOptions {
  /** Comma separated: `'chars'`, `'words'`, `'lines'`. Default `'words,chars'`. Lines are measured from the layout at split time. */
  type?: string;
  /** Class added to every character span. */
  charClass?: string;
  /** Class added to every word span. */
  wordClass?: string;
  /** Class added to every line wrapper (with `type` including `'lines'`). */
  lineClass?: string;
  /**
   * Wrap every line / word / char in a clipping element (`overflow: clip`),
   * so units can slide in from behind it. The wrappers are in `masks`.
   */
  mask?: 'lines' | 'words' | 'chars';
  /** Class for mask wrappers. */
  maskClass?: string;
  /**
   * Split again when the element's width changes (and once web fonts have
   * loaded), so lines stay true to the layout. Use with `onSplit`.
   */
  autoSplit?: boolean;
  /**
   * Called after every split (including re-splits). Return the animation that
   * uses the pieces: on a re-split it's reverted, and the new one is moved to
   * the same time.
   */
  onSplit?: (result: SplitResult) => Playable | void;
}

export interface SplitResult {
  /**
   * One span per character (grapheme). In scripts whose letters join (Arabic,
   * Syriac, N'Ko) a joined letter's text carries a zero-width joiner on the
   * joined side, so each piece keeps its connected shape.
   */
  chars: HTMLElement[];
  /** One span per word. */
  words: HTMLElement[];
  /**
   * One wrapper per rendered line (when `type` includes `'lines'`), each a
   * child of the split element; inline markup spanning lines (`<b>`, `<a>`)
   * is continued in the next line by a copy without its `id`.
   */
  lines: HTMLElement[];
  /** Clipping wrappers (with `mask`). */
  masks: HTMLElement[];
  /** Split again now (keeps the same arrays, refilled). */
  split(): void;
  /** Restore the original markup (and stop auto-splitting). */
  revert(): void;
  /** Stop auto-splitting, keeping the split markup. */
  kill(): void;
}

function span(cls: string | undefined, text?: string): HTMLSpanElement {
  const s = document.createElement('span');
  s.style.display = 'inline-block';
  if (cls) s.className = cls;
  if (text !== undefined) s.textContent = text;
  return s;
}

/**
 * A prepared `<span class style="display:inline-block" aria-hidden>` to clone:
 * one `cloneNode` per piece instead of creating the element, parsing its
 * style and setting two attributes.
 */
function pieceTemplate(cls: string | undefined): HTMLSpanElement {
  const s = span(cls);
  s.setAttribute('aria-hidden', 'true');
  return s;
}

let segmenter: any;
/** Grapheme clusters of a word; printable ASCII is one per character. */
function graphemes(s: string): string[] {
  if (ASCII.test(s)) return s.split('');
  if (segmenter === undefined) {
    segmenter = typeof Intl !== 'undefined' && (Intl as any).Segmenter ? new (Intl as any).Segmenter(undefined, { granularity: 'grapheme' }) : null;
  }
  return segmenter ? Array.from(segmenter.segment(s), (x: any) => x.segment) : Array.from(s);
}
const ASCII = /^[\x21-\x7e]*$/;

/** Right-to-left letters (Hebrew, Arabic, Syriac, Thaana, N'Ko…, presentation forms). */
const RTL = /[\u0590-\u08ff\ufb1d-\ufdff\ufe70-\ufefc]/;
const LETTER = /\p{L}/u;
const ZWJ = '\u200d';

/** Strong direction of a word: its first letter's ('n': no letters, e.g. numbers). */
function wordDir(w: string): 'l' | 'r' | 'n' {
  for (const ch of w) if (LETTER.test(ch)) return RTL.test(ch) ? 'r' : 'l';
  return 'n';
}

/**
 * Arabic-script joining type of a grapheme's first code point: 'D' joins on
 * both sides, 'R' only to the letter before it, '' not at all. Split into
 * separate inline-blocks, joined letters would fall back to their isolated
 * shapes; a zero-width joiner on each side of a joined pair keeps the
 * contextual shape.
 */
function joining(g: string): 'D' | 'R' | '' {
  const c = g.charCodeAt(0);
  if (c === 0x640 || c === 0x7fa) return 'D'; // tatweel, N'Ko lajanyalan
  if (c >= 0x7ca && c <= 0x7ea) return 'D'; // N'Ko letters
  if (c >= 0x710 && c <= 0x74f) {
    // Syriac
    if (c === 0x710 || (c >= 0x715 && c <= 0x719) || c === 0x71e || c === 0x728 || c === 0x72a || c === 0x72c || c === 0x72f || c === 0x74d) return 'R';
    return (c >= 0x712 && c <= 0x72f) || c >= 0x74e ? 'D' : '';
  }
  if (!((c >= 0x620 && c <= 0x6ff) || (c >= 0x750 && c <= 0x77f) || (c >= 0x8a0 && c <= 0x8c7))) return '';
  // Hamza, marks, digits and punctuation don't join.
  if (c === 0x621 || (c >= 0x64b && c <= 0x66d) || c === 0x674 || c === 0x6d4 || (c >= 0x6d6 && c <= 0x6ed) || (c >= 0x6f0 && c <= 0x6f9) || c === 0x6fd || c === 0x6fe) return '';
  if (
    (c >= 0x622 && c <= 0x625) || c === 0x627 || c === 0x629 || (c >= 0x62f && c <= 0x632) || c === 0x648 ||
    (c >= 0x671 && c <= 0x673) || (c >= 0x675 && c <= 0x677) || (c >= 0x688 && c <= 0x699) || c === 0x6c0 ||
    (c >= 0x6c3 && c <= 0x6cb) || c === 0x6cd || c === 0x6cf || c === 0x6d2 || c === 0x6d3 || c === 0x6d5 ||
    c === 0x6ee || c === 0x6ef || (c >= 0x759 && c <= 0x75b) || c === 0x76b || c === 0x76c || c === 0x771 ||
    c === 0x773 || c === 0x774 || c === 0x778 || c === 0x779 || (c >= 0x8aa && c <= 0x8ac) || c === 0x8ae ||
    c === 0x8b1 || c === 0x8b2 || c === 0x8b9
  ) return 'R';
  return 'D';
}

/** The text of each char piece: zero-width joiners where neighbouring letters join. */
function joined(gs: string[]): string[] {
  let any = false;
  for (const g of gs) if (RTL.test(g)) any = true;
  if (!any) return gs;
  const out = gs.slice();
  for (let i = 1; i < gs.length; i++) {
    if (joining(gs[i - 1]) === 'D' && joining(gs[i]) !== '') {
      out[i - 1] += ZWJ;
      out[i] = ZWJ + out[i];
    }
  }
  return out;
}

type Pieces = { chars: HTMLElement[]; words: HTMLElement[]; lines: HTMLElement[]; masks: HTMLElement[] };

/** One element after the DOM writes of the split, before lines are measured. */
interface Split {
  words: HTMLElement[];
  chars: HTMLElement[];
  restore: () => void;
}

/**
 * Split every element. All the writes come first, then one layout read of
 * every word (for lines) across all the elements, then the line and mask
 * wrappers: measuring each element right after splitting it laid the page
 * out again once per element.
 */
function splitAll(els: HTMLElement[], options: SplitOptions, out: Pieces): Array<() => void> {
  const types = new Set((options.type ?? 'words,chars').split(/[\s,]+/));
  // A mask needs the units it wraps.
  if (options.mask) types.add(options.mask);
  const wantChars = types.has('chars');
  const wantLines = types.has('lines');
  const wantWords = types.has('words') || wantLines || !wantChars || options.mask === 'words';
  const tpl = { word: pieceTemplate(options.wordClass), char: pieceTemplate(options.charClass) };
  // Paragraph directions, read before any write (only where right-to-left
  // text or a dir="rtl" ancestor makes mixed directions possible).
  const bases = els.map((el) =>
    RTL.test(el.textContent ?? '') || el.closest('[dir="rtl"]') || el.querySelector('[dir]')
      ? (getComputedStyle(el).direction === 'rtl' ? 'rtl' as const : 'ltr' as const)
      : null);
  const parts = els.map((el, i) => splitOne(el, wantWords, wantChars, tpl, bases[i]));
  const tops = wantLines ? parts.map((p) => p.words.map((w) => w.offsetTop)) : null;
  for (let k = 0; k < parts.length; k++) {
    const { words, chars } = parts[k];
    const lines: HTMLElement[] = [];
    if (tops && words.length) group(els[k], words, tops[k], options, lines);
    if (options.mask) {
      const units = options.mask === 'lines' ? lines : options.mask === 'words' ? words : chars;
      for (const u of units) {
        const m = span(options.maskClass);
        const st = m.style;
        st.display = options.mask === 'lines' ? 'block' : 'inline-block';
        st.overflow = 'clip';
        if (options.mask !== 'lines') st.verticalAlign = 'top';
        u.parentNode!.insertBefore(m, u);
        m.appendChild(u);
        out.masks.push(m);
      }
    }
    for (const w of words) out.words.push(w);
    for (const c of chars) out.chars.push(c);
    for (const l of lines) out.lines.push(l);
  }
  return parts.map((p) => p.restore);
}

/**
 * Group words into line wrappers by their measured vertical position. The
 * element's content is rebuilt line by line: an inline ancestor that spans
 * several lines (`<b>`, `<a>`…) is continued in each line by a shallow clone
 * (without its `id`), so every word keeps its formatting and the line
 * wrappers are direct children of the element.
 */
function group(root: HTMLElement, words: HTMLElement[], tops: number[], options: SplitOptions, lines: HTMLElement[]): void {
  const lineOf = new Map<Node, number>();
  let n = -1;
  let lastTop = NaN;
  for (let i = 0; i < words.length; i++) {
    if (n < 0 || Math.abs(tops[i] - lastTop) > 1) n++;
    lineOf.set(words[i], n);
    lastTop = tops[i];
  }
  const first = lines.length;
  for (let k = 0; k <= n; k++) {
    const line = span(options.lineClass);
    line.style.display = 'block';
    lines.push(line);
  }
  // Elements between the root and the words, then every other node in
  // document order with its path of such elements.
  const holds = new Set<Node>();
  for (const w of words) for (let p = w.parentNode; p && p !== root && !holds.has(p); p = p.parentNode) holds.add(p);
  const leaves: Node[] = [];
  const paths: Node[][] = [];
  const walk = (node: Node, path: Node[]) => {
    for (let c = node.firstChild; c; c = c.nextSibling) {
      if (holds.has(c)) walk(c, path.concat(c));
      else {
        leaves.push(c);
        paths.push(path);
      }
    }
  };
  walk(root, []);
  for (const h of holds) while (h.firstChild) h.removeChild(h.firstChild);
  while (root.firstChild) root.removeChild(root.firstChild);
  for (let k = first; k < lines.length; k++) root.appendChild(lines[k]);
  const used = new Set<Node>();
  let cur = 0;
  let open: Node[] = []; // originals of the ancestors open on the current line
  let placed: Node[] = []; // ...and what stands for them there
  for (let i = 0; i < leaves.length; i++) {
    const leaf = leaves[i];
    const path = paths[i];
    const ln = lineOf.get(leaf);
    if (ln !== undefined && ln !== cur) {
      cur = ln;
      open = [];
      placed = [];
    }
    let d = 0;
    while (d < open.length && d < path.length && open[d] === path[d]) d++;
    open.length = placed.length = d;
    let parent: Node = d ? placed[d - 1] : lines[first + cur];
    for (; d < path.length; d++) {
      const o = path[d];
      let c = o;
      if (used.has(o)) {
        c = o.cloneNode(false);
        (c as Element).removeAttribute('id');
      } else used.add(o);
      parent.appendChild(c);
      open.push(o);
      placed.push(c);
      parent = c;
    }
    parent.appendChild(leaf);
  }
}

/** Split one element into words and chars (writes only). */
function splitOne(el: HTMLElement, wantWords: boolean, wantChars: boolean, tpl: { word: HTMLElement; char: HTMLElement }, base: 'ltr' | 'rtl' | null): Split {
  const original = el.innerHTML;
  const hadLabel = el.getAttribute('aria-label');
  el.setAttribute('aria-label', el.textContent ?? '');
  const words: HTMLElement[] = [];
  const chars: HTMLElement[] = [];

  // Inline-blocks are neutral to the bidi algorithm: a run of words (or
  // chars) against the paragraph's direction would come out in the
  // paragraph's order. Such runs go in an isolate of their own direction.
  const bidi = base !== null;
  const walk = (node: Node) => {
    for (const child of Array.from(node.childNodes)) {
      if (child.nodeType === 3) {
        const text = child.textContent ?? '';
        const frag = document.createDocumentFragment();
        // With bidi: the first node and direction of every word / space, in order.
        const firsts: Node[] = [];
        const dirs: Array<'l' | 'r' | 'n'> = [];
        for (const part of text.split(/(\s+)/)) {
          if (!part) continue;
          if (/^\s+$/.test(part)) {
            const t = document.createTextNode(part);
            if (bidi) {
              firsts.push(t);
              dirs.push('n');
            }
            frag.appendChild(t);
            continue;
          }
          const container: Node = wantWords ? tpl.word.cloneNode(false) : frag;
          const at = frag.lastChild;
          if (wantWords) {
            words.push(container as HTMLElement);
            frag.appendChild(container);
          }
          if (wantChars) {
            const gs = graphemes(part);
            const texts = ASCII.test(part) ? gs : joined(gs);
            for (let i = 0; i < gs.length; i++) {
              const c = tpl.char.cloneNode(false) as HTMLElement;
              c.textContent = texts[i];
              chars.push(c);
              container.appendChild(c);
            }
          } else container.appendChild(document.createTextNode(part));
          if (bidi) {
            firsts.push(at ? at.nextSibling! : frag.firstChild!);
            dirs.push(wordDir(part));
          }
        }
        if (bidi) isolate(frag, firsts, dirs, dirOf(child.parentNode as Element, el, base!));
        child.parentNode!.replaceChild(frag, child);
      } else if (child.nodeType === 1 && (child as Element).tagName !== 'BR') {
        walk(child);
      }
    }
  };
  walk(el);

  const restore = () => {
    el.innerHTML = original;
    if (hadLabel === null) el.removeAttribute('aria-label');
    else el.setAttribute('aria-label', hadLabel);
  };
  return { words, chars, restore };
}

/** Direction a text node's parent lays out in: the nearest `dir` inside the element, else the element's. */
function dirOf(parent: Element, root: HTMLElement, base: 'ltr' | 'rtl'): 'ltr' | 'rtl' {
  for (let p: Element | null = parent; p && p !== root; p = p.parentElement) {
    const d = p.getAttribute('dir');
    if (d === 'ltr' || d === 'rtl') return d;
  }
  return base;
}

/**
 * Wrap each run of words against the paragraph direction `base` (with the
 * spaces and direction-less words between them) in
 * `<span dir style="unicode-bidi: isolate">`.
 */
function isolate(frag: DocumentFragment, firsts: Node[], dirs: Array<'l' | 'r' | 'n'>, base: 'ltr' | 'rtl'): void {
  const opp = base === 'ltr' ? 'r' : 'l';
  for (let i = 0; i < dirs.length; i++) {
    if (dirs[i] !== opp) continue;
    let j = i;
    for (let k = i + 1; k < dirs.length && dirs[k] !== (opp === 'r' ? 'l' : 'r'); k++) if (dirs[k] === opp) j = k;
    const end = j + 1 < firsts.length ? firsts[j + 1] : null;
    const box = document.createElement('span');
    box.dir = opp === 'r' ? 'rtl' : 'ltr';
    box.style.unicodeBidi = 'isolate';
    frag.insertBefore(box, firsts[i]);
    for (let n: Node | null = firsts[i]; n && n !== end; ) {
      const next: Node | null = n.nextSibling;
      box.appendChild(n);
      n = next;
    }
    i = j;
  }
}

const SPLIT_KEYS: OptionKeys<SplitOptions> = {
  type: 0, charClass: 0, wordClass: 0, lineClass: 0, mask: 0, maskClass: 0, autoSplit: 0, onSplit: 0,
};
const SPLIT_TYPES = ['chars', 'words', 'lines'];

/** Split every target; results are concatenated across targets. */
export function splitText(targets: Targets, options: SplitOptions = {}): SplitResult {
  checkKeys('splitText()', options, SPLIT_KEYS);
  for (const t of (options.type ?? 'words,chars').split(/[\s,]+/).filter(Boolean)) {
    if (!SPLIT_TYPES.includes(t)) warn(`splitText(): unknown type "${t}"; use 'chars', 'words' and/or 'lines' (e.g. type: 'lines,words,chars').${didYouMean(t, SPLIT_TYPES)}`);
  }
  const els = toTargets(targets) as HTMLElement[];
  checkTargets('splitText()', targets, els.length);
  let restores: Array<() => void> = [];
  let anim: Playable | null = null;
  let ro: ResizeObserver | null = null;
  let done = false;

  const result: SplitResult = {
    chars: [], words: [], lines: [], masks: [],
    split: () => {
      if (done) return;
      // Hand the animation over: revert the old one, re-run onSplit, same time.
      const time = anim ? anim.time : null;
      anim?.revert();
      anim = null;
      for (const r of restores.splice(0).reverse()) r();
      result.chars.length = result.words.length = result.lines.length = result.masks.length = 0;
      restores = splitAll(els, options, result);
      const a = options.onSplit?.(result);
      if (a) {
        anim = a;
        if (time !== null && time > 0) a.seek(time);
      }
    },
    kill: () => {
      done = true;
      ro?.disconnect();
    },
    revert: () => {
      done = true;
      ro?.disconnect();
      anim?.revert();
      anim = null;
      for (const r of restores.splice(0).reverse()) r();
    },
  };
  restores = splitAll(els, options, result);
  const first = options.onSplit?.(result);
  if (first) anim = first;

  if (options.autoSplit) {
    // Width changes re-flow lines; height changes alone don't.
    const widths = new Map<Element, number>();
    let queued = false;
    const resplit = () => {
      if (queued) return;
      queued = true;
      const run = () => {
        queued = false;
        result.split();
      };
      if (typeof requestAnimationFrame === 'function') requestAnimationFrame(run);
      else setTimeout(run, 0);
    };
    if (typeof ResizeObserver !== 'undefined') {
      ro = new ResizeObserver((entries) => {
        let changed = false;
        for (const e of entries) {
          const w = Math.round(e.contentRect.width);
          const prev = widths.get(e.target);
          widths.set(e.target, w);
          if (prev !== undefined && prev !== w) changed = true;
        }
        if (changed) resplit();
      });
      for (const el of els) ro.observe(el);
    }
    const fonts = typeof document !== 'undefined' ? (document as any).fonts : null;
    if (fonts && fonts.status !== 'loaded' && fonts.ready) fonts.ready.then(() => !done && resplit());
  }
  track(result);
  return result;
}
