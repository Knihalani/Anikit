// Build the native core for Node (crates/anikit-node) for this machine's CPU
// and copy it to native/anikit.node, where anikit/node picks it up. Renders
// then run the same core natively instead of in WebAssembly (same pixels;
// ANIKIT_NATIVE=0 forces WebAssembly).
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..', '..');
const cpu = process.env.ANIKIT_TARGET_CPU ?? 'native';
execFileSync('cargo', ['build', '-p', 'anikit-node', '--release', '--target-dir', join(root, 'target/native')], {
  cwd: root,
  stdio: 'inherit',
  env: { ...process.env, RUSTFLAGS: `${process.env.RUSTFLAGS ?? ''} -C target-cpu=${cpu}`.trim() },
});
const ext = process.platform === 'darwin' ? 'dylib' : process.platform === 'win32' ? 'dll' : 'so';
const lib = join(root, 'target/native/release', `${process.platform === 'win32' ? '' : 'lib'}anikit_node.${ext}`);
mkdirSync(join(here, '../native'), { recursive: true });
copyFileSync(lib, join(here, '../native/anikit.node'));
console.log(`native: ${join(here, '../native/anikit.node')} (target-cpu=${cpu})`);
