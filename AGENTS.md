# AGENTS.md — working on the Anikit codebase

Anikit is an animation engine: a `no_std` Rust core compiled to WebAssembly (TypeScript package) and to a native Python extension. This file tells coding agents how the repo fits together and how to verify changes. For *using* the library in an app, read `packages/anikit/AGENTS.md`.

## Layout

| Path | What | Verify with |
|---|---|---|
| `crates/anikit-core` | Engine: timing, easing, springs, colour, paths, morph, timelines, batches, physics `world`, 3D `space`, particle `cloud`, `splat` renderer, `raster` (solid 3D), `emitter`, `audio`, `video` (blur, I420, grade), `gif` encoder, noise | `cargo test -p anikit-core` · `cargo clippy -p anikit-core -- -D warnings` |
| `crates/anikit-wasm` | C‑ABI WebAssembly shim (`ak_*` exports, no wasm‑bindgen). Inputs go through a scratch buffer; outputs are read from linear memory. | `cargo clippy -p anikit-wasm -- -D warnings` |
| `packages/anikit` | TypeScript API + DOM layer (`src/`), vitest + happy‑dom tests (`test/`), build and bench scripts | `npm test` · `npx tsc --noEmit` · `node scripts/build.mjs` |
| `crates/anikit-py` | PyO3 bindings + high‑level Python API | `. .venv/bin/activate && maturin develop --release && pytest` |
| `examples/`, `site/` | Browser demos and the landing page (import `../packages/anikit/dist/anikit.js`) | build the TS package, then open in a browser (Playwright works headless) |

## How a feature flows

1. Implement and unit‑test it in **core** (`crates/anikit-core/src/*.rs`).
2. Export it from **wasm** (`crates/anikit-wasm/src/lib.rs`):
   - scalar arguments go directly in the call;
   - bulk data goes through `ak_f64_buf` / `ak_u8_buf`;
   - results are written to the f64 buffer, or exposed as pointers into stable buffers.
3. Add the typing in `packages/anikit/src/wasm.ts` (`Exports`), and build the TS API in its own module. Export it from `src/index.ts` and add a vitest test.
4. Run `node scripts/build-wasm.mjs` before TS tests whenever Rust changed. It regenerates `src/generated/wasm-bytes.ts`, which is gitignored.
5. Python: bind it in `crates/anikit-py/src/lib.rs`, then the high‑level API in `python/anikit/`. Keep `_anikit.pyi` in sync.

## Invariants (don't break these)

- **Hot paths:**
  - `Engine::tick`, `Anim::eval_into`, `Part.render`, `apply()` and `flush()` must not allocate;
  - keep the JS binding loop **monomorphic**: every binding is a `Binding` instance with a `mode` switch, never per‑binding closures;
  - measure before and after with `scripts/bench*.mjs` and `cargo run --release -p anikit-core --example bench`.
- **Re‑entrancy:**
  - `flush()` may be called from inside callbacks; it's re‑entrancy safe, so keep it that way;
  - user callbacks run after DOM writes.
- **Memory:**
  - handles are generational, so stale handles must be harmless;
  - controllers detach to weak references when finished;
  - every new controller must be registered with `track()` (see `context.ts`) and implement `revert()`/`kill()`.
- **Scratch buffer:** don't call user code (functions passed as values, `stagger()` and so on) between reserving the f64 scratch buffer and the `ak_*` call that consumes it. Resolve user inputs first. This was a real bug in `batch()`.
- **Honesty:** benchmark claims in `README.md` must come from the scripts in `packages/anikit/scripts/`.
- **Maths on wasm:** use `crate::math::{sqrt, sqrtf, floor, floorf, ceil, ceilf}`, never `libm::sqrt*`/`floor*`/`ceil*` directly. On wasm32, libm's versions are *software* routines unless nightly features are on; `math.rs` routes them to the hardware instructions. Check with `wasm-dis dist/anikit.wasm | grep -c f32.sqrt` (should be non-zero).
- **Pixel loops:** keep them branch-free over planar `f32` slices so LLVM vectorises them (`f32x4` on wasm), and keep decaying values out of denormal range (subtract a small floor). Denormals made one loop ~100× slower.

## Benchmarks

`packages/anikit/scripts/`: `bench.mjs` (Node, vs GSAP on objects), `bench-dom.mjs` (Chromium, DOM vs GSAP/Motion), `bench-physics.mjs` (vs matter.js), `bench-3d.mjs` (projection vs plain JS), `bench-cloud.mjs` (point cloud + glow in Chromium). Native: `cargo run --release -p anikit-core --example bench|physics_bench|cloud_bench`. V8 tiers WebAssembly up lazily, so warm up (100+ iterations) before timing.

## Debugging tips

- `engine.inspect()` returns a JSON snapshot of running animations. `testing.useFakeTime()` and `testing.advance(s)` give deterministic frames.
- Warnings are prefixed `anikit:` and include "did you mean" suggestions. `testing.captureWarnings()` collects them.
- For layout‑dependent features (scroll triggers, pinning, dragging), verify in Chromium via Playwright. happy‑dom has no layout.
