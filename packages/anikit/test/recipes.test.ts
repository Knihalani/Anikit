// Runs the recipes of docs/recipes.md (and AGENTS.md) that come with an HTML
// fixture: each must load, run a few seconds of animation without throwing,
// and produce no "anikit:" warnings. Type-checking of every block is done by
// `node scripts/check-docs.mjs`.
import { describe, it, expect, beforeAll, afterEach, vi } from 'vitest';
import { join, basename } from 'node:path';
import { testing, engine, onWarning } from '../src/index';
// @ts-expect-error: plain ESM helper without types
import { extractSnippets, writeSnippets, CHECKED_DOCS, OUT_DIR } from '../scripts/snippets.mjs';
// @ts-expect-error: plain ESM helper without types
import { undocumentedExports } from '../scripts/check-docs.mjs';
import { setup } from './setup';

beforeAll(setup);
afterEach(() => {
  engine.reducedMotion = 'never';
  onWarning(null);
});

writeSnippets();
const runnable = (CHECKED_DOCS as string[]).flatMap((doc) =>
  (extractSnippets(doc) as Array<{ line: number; lang: string; flags: Set<string>; html: string | null }>)
    .filter((s) => s.lang === 'ts' && s.html && !s.flags.has('no-run') && !s.flags.has('no-check'))
    .map((s) => ({ doc: basename(doc), line: s.line, html: s.html!, file: join(OUT_DIR, `${basename(doc, '.md').toLowerCase()}-${s.line}.ts`) })),
);

describe('docs', () => {
  it('mention every export of src/index.ts in docs/api.md', () => {
    expect(undocumentedExports()).toEqual([]);
  });
});

describe('recipes run', () => {
  it('finds runnable recipes', () => {
    expect(runnable.length).toBeGreaterThan(10);
  });
  for (const r of runnable) {
    it(`${r.doc}:${r.line}`, async () => {
      testing.reset();
      document.body.innerHTML = r.html;
      const warnings = testing.captureWarnings();
      // Recipes use console.assert for their checks: make failures fail the test.
      vi.spyOn(console, 'assert').mockImplementation((ok?: boolean, ...msg: unknown[]) => {
        if (!ok) throw new Error('console.assert failed: ' + msg.join(' '));
      });
      await import(/* @vite-ignore */ r.file);
      testing.advance(0.1);
      testing.finishAll(5);
      expect(warnings).toEqual([]);
    });
  }
});
