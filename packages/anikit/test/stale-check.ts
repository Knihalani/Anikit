// Vitest global setup: warn when the generated wasm is older than the Rust
// sources (a forgotten `node scripts/build-wasm.mjs` means tests run old code).
import { statSync, readdirSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

export default function (): void {
  const here = dirname(fileURLToPath(import.meta.url));
  const gen = join(here, '../src/generated/wasm-bytes.ts');
  const dirs = [join(here, '../../../crates/anikit-core/src'), join(here, '../../../crates/anikit-wasm/src')];
  if (!existsSync(gen)) {
    console.warn('\nanikit tests: src/generated/wasm-bytes.ts is missing — run `node scripts/build-wasm.mjs` first.\n');
    return;
  }
  const built = statSync(gen).mtimeMs;
  const newer = dirs.filter(existsSync).flatMap((d) => readdirSync(d).filter((f) => f.endsWith('.rs')).map((f) => join(d, f))).filter((f) => statSync(f).mtimeMs > built + 1000);
  if (newer.length) console.warn(`\nanikit tests: the wasm is older than ${newer.length} Rust source file(s) (e.g. ${newer[0].replace(/.*\/crates\//, 'crates/')}); run \`node scripts/build-wasm.mjs\` or these tests use old core code.\n`);
}
