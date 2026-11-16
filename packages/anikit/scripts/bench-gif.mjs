// GIF encoding (what record() does with its frames) in Node: encodeGif() of
// F frames at W x H. Two scenes: 'ui' (flat colours and a moving card: few
// colours, small changed rectangles) and 'gradient' (a full-frame moving
// gradient with a soft glow: thousands of colours, every pixel changes).
// Median of RUNS, ms per frame (lower is better), plus the output size.
// Usage: node scripts/bench-gif.mjs   (W=480 H=270 F=60 RUNS=5 DIST=path/to/dist)
import { pathToFileURL, fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

const dist = resolve(process.env.DIST || fileURLToPath(new URL('../dist', import.meta.url)));
const { encodeGif, ready } = await import(pathToFileURL(resolve(dist, 'anikit.js')).href);
await ready;
const W = +(process.env.W || 480), H = +(process.env.H || 270), F = +(process.env.F || 60);
const RUNS = +(process.env.RUNS || 5);
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];

function scene(kind) {
  const frames = [];
  for (let f = 0; f < F; f++) {
    const px = new Uint8ClampedArray(W * H * 4);
    const t = f / F;
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        const p = (y * W + x) * 4;
        if (kind === 'ui') {
          const cx = 40 + t * (W - 160);
          const card = x > cx && x < cx + 120 && y > 80 && y < 180;
          const bar = y < 30;
          px[p] = card ? 110 : bar ? 30 : 245;
          px[p + 1] = card ? 168 : bar ? 30 : 245;
          px[p + 2] = card ? 255 : bar ? 40 : 250;
        } else {
          const dx = x - W * (0.3 + 0.4 * t), dy = y - H / 2;
          const glow = 120 * Math.exp(-(dx * dx + dy * dy) / 4000);
          px[p] = (x / W) * 200 + glow;
          px[p + 1] = (y / H) * 160 + 40 * t + glow * 0.6;
          px[p + 2] = 255 - (x / W) * 120 + glow * 0.2;
        }
        px[p + 3] = 255;
      }
    }
    frames.push(px);
  }
  return frames;
}

console.log(`${W}x${H}, ${F} frames; median of ${RUNS} runs, ms per frame (lower is better)\n`);
console.log('scene'.padEnd(10), 'ms/frame'.padStart(9), 'KB'.padStart(8));
for (const kind of ['ui', 'gradient']) {
  const frames = scene(kind);
  encodeGif(frames.slice(0, 5), W, H, 30); // warm-up
  const runs = [];
  let bytes = 0;
  for (let r = 0; r < RUNS; r++) {
    const t = performance.now();
    bytes = encodeGif(frames, W, H, 30).length;
    runs.push((performance.now() - t) / F);
  }
  console.log(kind.padEnd(10), median(runs).toFixed(3).padStart(9), (bytes / 1024).toFixed(1).padStart(8));
}
