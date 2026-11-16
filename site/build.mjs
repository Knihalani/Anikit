// Build the landing page for static hosting (Cloudflare Pages):
//   node site/build.mjs   →  site-dist/
// Copies the site, vendors the lean anikit bundle (which streams anikit.wasm)
// and rewrites import paths.
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const out = join(root, 'site-dist');
const pages = ['index.html', 'style.css', 'main.js', 'demos.js'];
rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, 'vendor'), { recursive: true });
mkdirSync(join(out, 'fonts'), { recursive: true });
for (const f of pages) cpSync(join(here, f), join(out, f));
for (const f of ['InterDisplay-Black.woff2', 'Inter-Regular.woff2', 'JetBrainsMono-Regular.woff2', 'LICENSE-Inter.txt', 'LICENSE-JetBrainsMono.txt']) cpSync(join(here, 'fonts', f), join(out, 'fonts', f));
for (const f of ['anikit.lean.js', 'anikit.wasm']) cpSync(join(root, 'packages/anikit/dist', f), join(out, 'vendor', f));
for (const f of pages) {
  const p = join(out, f);
  writeFileSync(p, readFileSync(p, 'utf8').replaceAll('../packages/anikit/dist/', './vendor/'));
}
// Inline the stylesheet: one request fewer before first paint.
{
  const p = join(out, 'index.html');
  const css = readFileSync(join(out, 'style.css'), 'utf8');
  writeFileSync(p, readFileSync(p, 'utf8').replace('<link rel="stylesheet" href="style.css" />', () => `<style>${css}</style>`));
}
writeFileSync(join(out, '_headers'), `/fonts/*\n  Cache-Control: public, max-age=31536000, immutable\n/vendor/*\n  Cache-Control: public, max-age=3600\n/vendor/anikit.wasm\n  Content-Type: application/wasm\n`);
console.log('site →', out);
