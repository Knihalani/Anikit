# anikit

A web animation engine with a Rust/WebAssembly core: tweens, keyframes, timelines, springs, inertia, OKLab colour, shape morphing, motion paths, scroll‑linked animation, text splitting, FLIP, compositor offload and instanced batches.

```bash
npm install anikit
```

```ts
import { animate, timeline, stagger, spring } from 'anikit';

animate('.card', { y: [40, 0], opacity: [0, 1] }, { delay: stagger(0.05), ease: 'spring(bounce: 0.3)' });
```

- `import 'anikit'` embeds the WASM module, so it works with zero configuration (bundlers, CDNs, Node).
- `import 'anikit/lean'` (ESM) streams `anikit.wasm` from next to the script instead. That's 18 KB gz of JS plus a cacheable 73 KB gz module.

API reference: [docs/api.md](../../docs/api.md). Project overview and benchmarks: [README](../../README.md).
