import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'node:url';
export default defineConfig({
  resolve: { alias: { anikit: fileURLToPath(new URL('./src/index.ts', import.meta.url)) } },
  test: { environment: 'happy-dom', include: ['test/**/*.test.ts'], globalSetup: ['test/stale-check.ts'] },
});
