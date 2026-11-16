// With ANIKIT_TEST_NATIVE=1, every test runs on the native core (native/anikit.node,
// built by `node scripts/build-native.mjs`) instead of WebAssembly: `npm run test:native`.
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';

if (process.env.ANIKIT_TEST_NATIVE === '1') {
  const file = join(dirname(process.cwd() + '/x'), 'native/anikit.node'); // vitest runs from packages/anikit
  if (!existsSync(file)) throw new Error(`ANIKIT_TEST_NATIVE=1: ${file} is missing (node scripts/build-native.mjs)`);
  const m = { exports: {} as Record<string, unknown> };
  process.dlopen(m as unknown as { exports: object }, file);
  (globalThis as { __ANIKIT_NATIVE__?: unknown }).__ANIKIT_NATIVE__ = m.exports;
}
