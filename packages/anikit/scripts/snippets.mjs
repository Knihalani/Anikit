// Code blocks of the Markdown docs, so they can be type-checked and run.
//
// A ```ts block is checked; add `no-check` to the info string to skip it and
// `no-run` to only type-check it. An ```html block right before a ts block is
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

/** Write each checked ts block to `test/.snippets/<doc>-<line>.ts`; returns their paths. */
export function writeSnippets() {
  rmSync(OUT_DIR, { recursive: true, force: true });
  mkdirSync(OUT_DIR, { recursive: true });
  const files = [];
  for (const doc of CHECKED_DOCS) {
    for (const s of extractSnippets(doc)) {
      if (s.lang !== 'ts' || s.flags.has('no-check')) continue;
      const name = `${basename(doc, '.md').toLowerCase()}-${s.line}.ts`;
      // `export {}` keeps every snippet a module (own scope, top-level await).
      writeFileSync(join(OUT_DIR, name), `// ${basename(doc)}:${s.line}\n${s.code}\nexport {};\n`);
      files.push(join(OUT_DIR, name));
    }
  }
  return files;
}
