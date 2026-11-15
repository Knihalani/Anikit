// Build the landing page for static hosting (Cloudflare Pages):
//   node site/build.mjs [path/to/film.mp4]   →  site-dist/
// Copies the site, vendors the anikit bundle, rewrites import paths, and
// encodes a web copy of the film (< 25 MiB, Pages' per-file limit) + poster.
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const out = join(root, 'site-dist');
rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, 'vendor'), { recursive: true });
for (const f of ['index.html', 'style.css', 'main.js', 'cosmos.js', 'gravity.js']) cpSync(join(here, f), join(out, f));
cpSync(join(here, 'fonts'), join(out, 'fonts'), { recursive: true });
cpSync(join(root, 'packages/anikit/dist/anikit.js'), join(out, 'vendor/anikit.js'));
for (const f of ['main.js', 'cosmos.js', 'gravity.js']) {
  const p = join(out, f);
  writeFileSync(p, readFileSync(p, 'utf8').replaceAll('../packages/anikit/dist/anikit.js', './vendor/anikit.js'));
}
const film = process.argv[2];
if (film && existsSync(film)) {
  mkdirSync(join(out, 'film'), { recursive: true });
  const web = join(out, 'film/anikit-film.mp4');
  execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-i', film, '-c:v', 'libx264', '-preset', 'slow', '-crf', '27', '-maxrate', '2400k', '-bufsize', '4800k',
    '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-b:a', '128k', '-movflags', '+faststart', web]);
  execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-ss', '10.6', '-i', film, '-frames:v', '1', '-q:v', '3', '-vf', 'scale=1280:-2', join(out, 'film/poster.jpg')]);
  console.log('film:', (statSync(web).size / 1048576).toFixed(1), 'MiB');
}
writeFileSync(join(out, '_headers'), `/fonts/*\n  Cache-Control: public, max-age=31536000, immutable\n/film/*\n  Cache-Control: public, max-age=86400\n/vendor/*\n  Cache-Control: public, max-age=3600\n`);
console.log('site →', out);
