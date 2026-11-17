# AGENTS.md — working on the Anikit codebase

Anikit is an animation engine: a `no_std` Rust core compiled to WebAssembly (TypeScript package) and to a native Python extension. This file tells coding agents how the repo fits together and how to verify changes. For *using* the library in an app, read `packages/anikit/AGENTS.md`.

## Layout

| Path | What | Verify with |
|---|---|---|
| `crates/anikit-core` | Engine: timing, easing, springs, colour, paths, morph, timelines, batches, physics `world`, 3D `space`, particle `cloud`, `splat` renderer, `raster` (solid 3D), `sdf` (surface nets), `emitter`, `audio`, `video` (blur, I420, grade), `gif` encoder, noise | `cargo test -p anikit-core` · `cargo clippy -p anikit-core -- -D warnings` |
| `crates/anikit-wasm` | C‑ABI WebAssembly shim (`ak_*` exports, no wasm‑bindgen). Inputs go through a scratch buffer; outputs are read from linear memory. | `cargo clippy -p anikit-wasm -- -D warnings` |
| `crates/anikit-node` | The same `ak_*` exports as a native N‑API addon for Node (`node scripts/build-native.mjs` → `packages/anikit/native/anikit.node`, built for this CPU; `anikit/node` loads it when present, `ANIKIT_NATIVE=0` forces wasm). Wrappers are generated from `anikit-wasm`'s signatures; all allocations come from one arena exposed as one `ArrayBuffer`, so JS offsets work unchanged. | `cargo clippy -p anikit-node -- -D warnings` · `npm run test:native` (whole vitest suite on the native core) |
| `crates/anikit-shader` | Shadertoy GLSL/WGSL → naga → Cranelift JIT (4-lane quad SIMD, masked control flow), texture sampling, built-in shader library (`library/*.glsl`); used by `anikit-video` shader layers (`src/eval/shader.rs`). Guide: `docs/video/shader.md` | `cargo test -p anikit-shader` · `cargo run --release -p anikit-shader --example library_bench` |
| `packages/anikit` | TypeScript API + DOM layer (`src/`), vitest + happy‑dom tests (`test/`), build and bench scripts | `npm test` · `npm run typecheck` (tsc + doc examples + JSDoc) · `node scripts/build.mjs` |
| `crates/anikit-py` | PyO3 bindings + high‑level Python API | `. .venv/bin/activate && maturin develop --release && pytest` |
| `examples/`, `site/` | Browser demos and the landing page (import `../packages/anikit/dist/anikit.js`) | build the TS package, then open in a browser (Playwright works headless) |

## How a feature flows

1. Implement and unit‑test it in **core** (`crates/anikit-core/src/*.rs`).
2. Export it from **wasm** (`crates/anikit-wasm/src/lib.rs`):
   - scalar arguments go directly in the call;
   - bulk data goes through `ak_f64_buf` / `ak_u8_buf`;
   - results are written to the f64 buffer, or exposed as pointers into stable buffers.
3. Add the typing in `packages/anikit/src/wasm.ts` (`Exports`), and build the TS API in its own module. Export it from `src/index.ts` and add a vitest test.
   - Document every new export in `docs/api.md` with an example: `npm run typecheck` fails otherwise (it runs `scripts/check-docs.mjs`, which type-checks the examples in `docs/api.md`, `docs/recipes.md` and `packages/anikit/AGENTS.md`, and `scripts/check-jsdoc.mjs`, which requires JSDoc on every member of exported interfaces).
   - New option keys go in the interface **and** its `OptionKeys<T>` table next to it (e.g. `ANIMATE_KEYS`, `TIMELINE_KEYS`); the compiler keeps them in sync, and the table drives the "unknown option" warnings.
   - Validate user input at construction with the helpers in `src/debug.ts` (`checkKeys`, `checkSeconds`, `checkTargets`, `warn`); never in `tick`/`render`/`apply`/`flush`. Test warnings with `testing.captureWarnings()`.
4. Run `node scripts/build-wasm.mjs` before TS tests whenever Rust changed. It regenerates `src/generated/wasm-bytes.ts`, which is gitignored. After doc changes, `node scripts/build-llms.mjs` regenerates `llms.txt`/`llms-full.txt`; `npm run docs:browser` runs the recipes in Chromium.
5. Python: bind it in `crates/anikit-py/src/lib.rs`, then the high‑level API in `python/anikit/`. Keep `_anikit.pyi` in sync.

## Invariants (don't break these)

- **Hot paths:**
  - `Engine::tick`, `Anim::eval_into`, `Part.render`, `apply()` and `flush()` must not allocate;
  - keep the JS binding loop **monomorphic**: every binding is a `Binding` instance with a `mode` switch, never per‑binding closures;
  - plain-object targets write through one generated function per property-set shape (`new Function`, strict mode, built once and shared), with a plain loop as the fallback under a CSP without `'unsafe-eval'`; keep both paths tested;
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
- **Native = wasm, bit for bit:** `simd.rs` has wasm, SSE4.1 and AVX2 (`V8`) backends with the same per-lane semantics (pmin/pmax operand order, saturating float→int, `V4 / f32` is a reciprocal multiply). New lane code must keep them equal; `node scripts/check-films.mjs --base <saved build>` compares the reference film stills (also with `NEW_ENV=ANIKIT_NATIVE=0`).

## Benchmarks

`packages/anikit/scripts/`: `bench.mjs` (Node, vs GSAP on objects), `bench-dom.mjs` (Chromium, DOM vs GSAP/Motion), `bench-physics.mjs` (vs matter.js), `bench-3d.mjs` (projection vs plain JS), `bench-cloud.mjs` (point cloud + glow in Chromium). Native: `cargo run --release -p anikit-core --example bench|physics_bench|cloud_bench`. V8 tiers WebAssembly up lazily, so warm up (100+ iterations) before timing.

## Debugging tips

- `engine.inspect()` returns a JSON snapshot of running animations. `testing.useFakeTime()` and `testing.advance(s)` give deterministic frames.
- Warnings are prefixed `anikit:` and include "did you mean" suggestions. `testing.captureWarnings()` collects them.
- For layout‑dependent features (scroll triggers, pinning, dragging), verify in Chromium via Playwright. happy‑dom has no layout.
