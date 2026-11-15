/**
 * `splitText()` — split an element's text into lines / words / characters
 * (each wrapped in an inline-block span) for per-glyph animation, keeping
 * nested markup and accessibility intact.
 *
 * ```ts
 * const { chars } = splitText('h1');
 * animate(chars, { y: ['1em', 0], opacity: [0, 1] }, { delay: stagger(0.02) });
 * ```
 */
import { Targets, toTargets } from './props';
import { checkKeys, checkTargets, warn, didYouMean } from './debug';
import type { OptionKeys } from './debug';

export interface SplitOptions {
  /** Comma separated: `'chars'`, `'words'`, `'lines'`. Default `'words,chars'`. Lines are measured from the layout at split time. */
  type?: string;
  /** Class added to every character span. */
  charClass?: string;
  /** Class added to every word span. */
  wordClass?: string;
  /** Class added to every line wrapper (with `type` including `'lines'`). */
  lineClass?: string;
}

export interface SplitResult {
  /** One span per character (grapheme). */
  chars: HTMLElement[];
  /** One span per word. */
  words: HTMLElement[];
  lines: HTMLElement[];
  /** Restore the original markup. */
  revert(): void;
}

function span(cls: string | undefined, text?: string): HTMLSpanElement {
  const s = document.createElement('span');
  s.style.display = 'inline-block';
  if (cls) s.className = cls;
  if (text !== undefined) s.textContent = text;
  return s;
}

const SPLIT_KEYS: OptionKeys<SplitOptions> = { type: 0, charClass: 0, wordClass: 0, lineClass: 0 };
const SPLIT_TYPES = ['chars', 'words', 'lines'];

/** Split every target; results are concatenated across targets. */
export function splitText(targets: Targets, options: SplitOptions = {}): SplitResult {
  checkKeys('splitText()', options, SPLIT_KEYS);
  const types = new Set((options.type ?? 'words,chars').split(/[\s,]+/).filter(Boolean));
  for (const t of types) if (!SPLIT_TYPES.includes(t)) warn(`splitText(): unknown type "${t}"; use 'chars', 'words' and/or 'lines' (e.g. type: 'lines,words,chars').${didYouMean(t, SPLIT_TYPES)}`);
  const wantChars = types.has('chars');
  const wantWords = types.has('words') || types.has('lines') || !wantChars;
  const wantLines = types.has('lines');
  const result: SplitResult = { chars: [], words: [], lines: [], revert: () => {} };
  const restores: Array<() => void> = [];
  const seg: any =
    typeof Intl !== 'undefined' && (Intl as any).Segmenter ? new (Intl as any).Segmenter(undefined, { granularity: 'grapheme' }) : null;
  const graphemes = (s: string): string[] => (seg ? Array.from(seg.segment(s), (x: any) => x.segment) : Array.from(s));

  const els = toTargets(targets);
  checkTargets('splitText()', targets, els.length);
  for (const t of els) {
    const el = t as HTMLElement;
    const original = el.innerHTML;
    const hadLabel = el.getAttribute('aria-label');
    el.setAttribute('aria-label', el.textContent ?? '');
    const words: HTMLElement[] = [];

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
            const container: Node = wantWords ? span(options.wordClass) : frag;
            if (wantWords) {
              (container as HTMLElement).setAttribute('aria-hidden', 'true');
              words.push(container as HTMLElement);
              frag.appendChild(container);
            }
            if (wantChars) {
              for (const g of graphemes(part)) {
                const c = span(options.charClass, g);
                c.setAttribute('aria-hidden', 'true');
                result.chars.push(c);
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
    result.words.push(...words);

    if (wantLines && words.length) {
      // Group words by their rendered vertical position.
      const groups: HTMLElement[][] = [];
      let lastTop = NaN;
      for (const w of words) {
        const top = w.offsetTop;
        if (groups.length === 0 || Math.abs(top - lastTop) > 1) groups.push([]);
        groups[groups.length - 1].push(w);
        lastTop = top;
      }
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
        result.lines.push(line);
      }
    }
    restores.push(() => {
      el.innerHTML = original;
      if (hadLabel === null) el.removeAttribute('aria-label');
      else el.setAttribute('aria-label', hadLabel);
    });
  }
  result.revert = () => restores.forEach((r) => r());
  return result;
}
