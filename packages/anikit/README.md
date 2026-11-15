# anikit

A web animation engine with a Rust/WebAssembly core: tweens, keyframes, timelines, springs, inertia, OKLab colour, shape morphing, motion paths, scroll triggers, text splitting, FLIP, effects, physics worlds, 3D, particles, GIF/video capture, compositor offload and instanced batches.

```bash
npm install anikit
```

```ts
import { animate, timeline, stagger, spring } from 'anikit';

animate('.card', { y: [40, 0], opacity: [0, 1] }, { delay: stagger(0.05), ease: 'spring(bounce: 0.3)' });
```

- Times are in **seconds**. Transforms (`x`, `y`, `scale`, `rotate`) are the fast path for motion.
- Mistakes print `anikit:` warnings that say how to fix them (unknown options and properties with "did you mean", millisecond-looking durations, wrong units, selectors that match nothing). `testing.captureWarnings()` collects them in tests.
- Every option has JSDoc (defaults, units, examples) and easing names autocomplete, so editor hover is usually enough.
- `import 'anikit'` embeds the WASM module, so it works with zero configuration (bundlers, CDNs, Node).
- `import 'anikit/lean'` (ESM) streams `anikit.wasm` from next to the script instead: 53 KB gz of JS plus a cacheable 156 KB gz module (sizes printed by `node scripts/build.mjs`).

Docs:

- [Recipes](../../docs/recipes.md): hero entrance, staggered list, scroll reveal, pinned scroll scrub, page transition, carousel, physics toy, SVG morph, counter, FLIP, React cleanup, testing, recording a GIF, rendering an MP4 in Node, Python.
- [API reference](../../docs/api.md).
- For coding agents: [AGENTS.md](AGENTS.md) (playbook: "want X → use Y", rules, mistakes → fixes) and `llms-full.txt` (playbook + recipes + reference in one file), both shipped in the package.
- Project overview and benchmarks: [README](../../README.md).
