import { describe, it, expect } from 'vitest';
import { mkdtempSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
// @ts-expect-error: plain ESM CLI without types
import { scaffold, templateFiles } from '../bin/anikit.mjs';

describe('npx anikit new', () => {
  it('writes each template', () => {
    for (const t of ['vanilla', 'vite', 'react']) {
      const dir = join(mkdtempSync(join(tmpdir(), 'anikit-')), 'app');
      const files: string[] = scaffold(t, dir);
      for (const f of files) expect(existsSync(join(dir, f))).toBe(true);
      const main = t === 'react' ? 'src/App.jsx' : t === 'vite' ? 'src/main.js' : 'main.js';
      expect(readFileSync(join(dir, main), 'utf8')).toMatch(/recipe|useRecipe/);
      if (t !== 'vanilla') expect(JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8')).dependencies.anikit).toBeTruthy();
      rmSync(dir, { recursive: true });
    }
  });
  it('rejects unknown templates and non-empty directories', () => {
    expect(templateFiles('nope')).toBeNull();
    expect(() => scaffold('nope', join(tmpdir(), 'x'))).toThrow(/unknown template/);
    const dir = mkdtempSync(join(tmpdir(), 'anikit-'));
    scaffold('vite', dir);
    expect(() => scaffold('vite', dir)).toThrow(/not empty/);
    rmSync(dir, { recursive: true });
  });
});
