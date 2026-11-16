// Build the landing page for static hosting (Cloudflare Pages):
//   node site/build.mjs   →  site-dist/
// Copies the site, vendors the anikit bundle and rewrites import paths.
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const out = join(root, 'site-dist');
rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, 'vendor'), { recursive: true });
for (const f of ['index.html', 'style.css', 'main.js', 'cosmos.js', 'gravity.js', 'flight.js']) cpSync(join(here, f), join(out, f));
cpSync(join(here, 'fonts'), join(out, 'fonts'), { recursive: true });
cpSync(join(root, 'packages/anikit/dist/anikit.js'), join(out, 'vendor/anikit.js'));
for (const f of ['main.js', 'cosmos.js', 'gravity.js', 'flight.js']) {
  const p = join(out, f);
  writeFileSync(p, readFileSync(p, 'utf8').replaceAll('../packages/anikit/dist/anikit.js', './vendor/anikit.js'));
}
writeFileSync(join(out, '_headers'), `/fonts/*\n  Cache-Control: public, max-age=31536000, immutable\n/vendor/*\n  Cache-Control: public, max-age=3600\n`);
console.log('site →', out);
