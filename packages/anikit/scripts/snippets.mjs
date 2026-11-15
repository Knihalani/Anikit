// Code blocks of the Markdown docs, so they can be type-checked and run.
//
// A ```ts block is checked; add `no-check` to the info string to skip it and
// `no-run` to only type-check it, `browser` to run it in Chromium only (not
// happy-dom). An ```html block right before a ts block is
// its DOM fixture: blocks with a fixture also run in happy-dom
// (test/recipes.test.ts).
import { readFileSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import { join, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

export const pkg = join(dirname(fileURLToPath(import.meta.url)), '..');
export const root = join(pkg, '..', '..');
/** Documents whose ts blocks must be self-contained and correct. */
export const CHECKED_DOCS = [join(root, 'docs/recipes.md'), join(pkg, 'AGENTS.md')];
export const OUT_DIR = join(pkg, 'test/.snippets');

/** `[{ file, line, lang, flags, code, html }]` for every fenced block. */
export function extractSnippets(file) {
  const lines = readFileSync(file, 'utf8').split('\n');
  const out = [];
  let html = null;
  let heading = '';
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].startsWith('## ')) heading = lines[i].slice(3).trim();
    const open = /^```(\w+)?\s*(.*)$/.exec(lines[i]);
    if (!open) continue;
    const start = i;
    const body = [];
    for (i++; i < lines.length && !lines[i].startsWith('```'); i++) body.push(lines[i]);
    const lang = open[1] ?? '';
    const flags = new Set(open[2].split(/\s+/).filter(Boolean));
    const code = body.join('\n');
    if (lang === 'html') {
      html = code;
      continue;
    }
    out.push({ file, line: start + 1, heading, lang, flags, code, html: lang === 'ts' ? html : null });
    if (lang === 'ts') html = null;
  }
  return out;
}

/**
 * docs/api.md holds fragments (no imports, placeholder names like `el`). They
 * are checked with a generated prelude: every value export the block uses but
 * doesn't import or declare, plus `declare const` for the placeholders below.
 * Sections whose source is maintained elsewhere are skipped.
 */
export const API_DOC = join(root, 'docs/api.md');
const API_SKIP = new Set(['3D', 'Point clouds and glow', 'Particle emitters', 'Video: compositions']);
const PLACEHOLDERS = {
  el: 'HTMLElement', card: 'HTMLElement', img: 'HTMLElement', logo: 'HTMLElement', ball: 'HTMLElement', label: 'HTMLElement',
  path: 'SVGPathElement', panel: 'HTMLElement', grid: 'HTMLElement', audioEl: 'HTMLAudioElement', els: 'HTMLElement[]',
  obj: '{ value: number; x: number; y: number }', ctx: 'CanvasRenderingContext2D', w: 'number', h: 'number',
  vx: 'number', floorY: 'number', counter: 'HTMLElement', palette: 'string[]', x: 'number', y: 'number',
  otherTimelineOrAnimation: 'import("anikit").Timeline', render: '(v: number) => void', draw: '(data: Float32Array, stride: number) => void',
  drawParticles: '(dt: number) => void', setVolume: '(v: number) => void', rootRef: '{ current: HTMLElement }',
  useLayoutEffect: '(fn: () => void | (() => void), deps: unknown[]) => void', gl: 'WebGLRenderingContext',
  canvas: 'HTMLCanvasElement', d: 'string', next: '() => void', prev: '() => void', myLogger: 'Console', expect: '(v: unknown) => { toEqual(x: unknown): void }', tl: 'import("anikit").Timeline', t: 'number',
  onEnter: 'Cb', onLeave: 'Cb', onEnterBack: 'Cb', onLeaveBack: 'Cb', onToggle: 'Cb', onUpdate: 'Cb', item: 'HTMLElement', list: 'HTMLElement',
};

/** Value and type exports of src/index.ts. */
export function exportNames(kind = 'all') {
  const src = readFileSync(join(pkg, 'src/index.ts'), 'utf8');
  const names = [];
  for (const m of src.matchAll(/export\s+(type\s+)?\{([^}]+)\}/g)) {
    if ((kind === 'value' && m[1]) || (kind === 'type' && !m[1])) continue;
    for (const part of m[2].split(',')) {
      const name = part.trim().split(/\s+as\s+/).pop();
      if (name) names.push(name);
    }
  }
  return names;
}

/** Names a block declares or imports itself. */
function ownNames(code) {
  const own = new Set();
  for (const m of code.matchAll(/\b(?:const|let|var|function|class)\s+([A-Za-z_$][\w$]*)/g)) own.add(m[1]);
  for (const m of code.matchAll(/\b(?:const|let|var)\s*[{[]([^}\]=]+)[}\]]\s*=/g)) {
    for (const part of m[1].split(',')) {
      const id = part.split(':').pop().trim().split(/\s|=/)[0];
      if (id) own.add(id);
    }
  }
  for (const m of code.matchAll(/import\s*\{([^}]+)\}\s*from/g)) for (const p of m[1].split(',')) own.add(p.trim().split(/\s+as\s+/).pop());
  return own;
}

function fragmentPrelude(code) {
  const own = ownNames(code);
  const used = new Set(code.match(/[A-Za-z_$][\w$]*/g));
  const imports = exportNames('value').filter((n) => used.has(n) && !own.has(n));
  const decls = Object.entries(PLACEHOLDERS).filter(([n]) => used.has(n) && !own.has(n) && !imports.includes(n));
  return [
    imports.length ? `import { ${imports.join(', ')} } from 'anikit';` : '',
    ...decls.map(([n, t]) => `declare const ${n}: ${t === 'Cb' ? '(self: import("anikit").ScrollTrigger) => void' : t};`),
  ].filter(Boolean).join('\n');
}

/** Every checked ts block: `{ doc, line, heading, file, code }`. */
export function checkedSnippets() {
  const out = [];
  for (const doc of [...CHECKED_DOCS, API_DOC]) {
    const fragment = doc === API_DOC;
    for (const s of extractSnippets(doc)) {
      if (s.lang !== 'ts' || s.flags.has('no-check') || (fragment && API_SKIP.has(s.heading))) continue;
      const file = join(OUT_DIR, `${basename(doc, '.md').toLowerCase()}-${s.line}.ts`);
      const prelude = fragment ? fragmentPrelude(s.code) : '';
      out.push({ ...s, doc, file, prelude });
    }
  }
  return out;
}

/** Write each checked ts block to `test/.snippets/<doc>-<line>.ts`; returns their paths. */
export function writeSnippets() {
  rmSync(OUT_DIR, { recursive: true, force: true });
  mkdirSync(OUT_DIR, { recursive: true });
  const files = [];
  for (const s of checkedSnippets()) {
    // `export {}` keeps every snippet a module (own scope, top-level await).
    // The prelude shares the first line so error line numbers stay doc-relative (+1).
    writeFileSync(s.file, `${s.prelude.replace(/\n/g, ' ')} // ${basename(s.doc)}:${s.line}\n${s.code}\nexport {};\n`);
    files.push(s.file);
  }
  return files;
}
