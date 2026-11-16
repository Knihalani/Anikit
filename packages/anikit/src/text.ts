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
  /** One span per character (grapheme). */
  chars: HTMLElement[];
  /** One span per word. */
  words: HTMLElement[];
  /** One wrapper per rendered line (when `type` includes `'lines'`). */
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
  const parts = els.map((el) => splitOne(el, wantWords, wantChars, tpl));
  const tops = wantLines ? parts.map((p) => p.words.map((w) => w.offsetTop)) : null;
  for (let k = 0; k < parts.length; k++) {
    const { words, chars } = parts[k];
    const lines: HTMLElement[] = [];
    if (tops && words.length) group(words, tops[k], options, lines);
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

/** Group words into line wrappers by their measured vertical position. */
function group(words: HTMLElement[], tops: number[], options: SplitOptions, lines: HTMLElement[]): void {
  const groups: HTMLElement[][] = [];
  let lastTop = NaN;
  words.forEach((w, i) => {
    if (groups.length === 0 || Math.abs(tops[i] - lastTop) > 1) groups.push([]);
    groups[groups.length - 1].push(w);
    lastTop = tops[i];
  });
  for (const g of groups) {
    const line = span(options.lineClass);
    line.style.display = 'block';
    g[0].parentNode!.insertBefore(line, g[0]);
    for (let i = 0; i < g.length; i++) {
      const w = g[i];
      const next = w.nextSibling;
      line.appendChild(w);
      if (i < g.length - 1 && next && next.nodeType === 3) line.appendChild(next);
    }
    lines.push(line);
  }
}

/** Split one element into words and chars (writes only). */
function splitOne(el: HTMLElement, wantWords: boolean, wantChars: boolean, tpl: { word: HTMLElement; char: HTMLElement }): Split {
  const original = el.innerHTML;
  const hadLabel = el.getAttribute('aria-label');
  el.setAttribute('aria-label', el.textContent ?? '');
  const words: HTMLElement[] = [];
  const chars: HTMLElement[] = [];

  const walk = (node: Node) => {
    for (const child of Array.from(node.childNodes)) {
      if (child.nodeType === 3) {
        const text = child.textContent ?? '';
        const frag = document.createDocumentFragment();
        for (const part of text.split(/(\s+)/)) {
          if (!part) continue;
          if (/^\s+$/.test(part)) {
            frag.appendChild(document.createTextNode(part));
            continue;
          }
          const container: Node = wantWords ? tpl.word.cloneNode(false) : frag;
          if (wantWords) {
            words.push(container as HTMLElement);
            frag.appendChild(container);
          }
          if (wantChars) {
            for (const g of graphemes(part)) {
              const c = tpl.char.cloneNode(false) as HTMLElement;
              c.textContent = g;
              chars.push(c);
              container.appendChild(c);
            }
          } else container.appendChild(document.createTextNode(part));
        }
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
