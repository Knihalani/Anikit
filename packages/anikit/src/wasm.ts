/**
 * Low-level bridge to the WebAssembly core.
 *
 * The core exports plain C-ABI functions. Bulk data is exchanged through
 * linear memory: we write inputs into a scratch region and read outputs
 * (animated values, updated handles, events) straight from memory views —
 * no copies, no glue code, no per-value calls.
 */
import { WASM_BASE64 } from './generated/wasm-bytes';

export interface Exports {
  memory: WebAssembly.Memory;
  ak_f64_buf(n: number): number;
  ak_u8_buf(n: number): number;
  ak_str_ptr(): number;
  ak_easing_parse(len: number): number;
  ak_easing_table(n: number): number;
  ak_easing_bezier(x1: number, y1: number, x2: number, y2: number): number;
  ak_easing_spring(k: number, c: number, m: number, v: number): number;
  ak_easing_spring_db(duration: number, bounce: number, v: number): number;
  ak_ease(id: number, t: number): number;
  ak_easing_duration(id: number): number;
  ak_easing_css(id: number, tolerance: number): number;
  ak_easing_sample(id: number, n: number): number;
  ak_anim_create(len: number): number;
  ak_group_create(
    delay: number, duration: number, iterations: number, direction: number,
    endDelay: number, easing: number, flags: number, auto: number,
  ): number;
  ak_callback_create(): number;
  ak_group_add(g: number, c: number, start: number): number;
  ak_group_remove(c: number): number;
  ak_spring_create(ch: number, k: number, c: number, m: number): number;
  ak_decay_create(
    ch: number, power: number, tc: number, k: number, c: number, m: number,
    hasTarget: number, hasMin: number, hasMax: number,
  ): number;
  ak_sim_target(h: number, ch: number): number;
  ak_sim_position(h: number, ch: number, hasVel: number): number;
  ak_sim_params(h: number, k: number, c: number, m: number): number;
  ak_sim_rest(h: number, d: number, s: number): number;
  ak_sim_velocity_ptr(h: number): number;
  ak_play(h: number): number;
  ak_pause(h: number): number;
  ak_seek(h: number, t: number): number;
  ak_set_progress(h: number, p: number): number;
  ak_set_rate(h: number, r: number): number;
  ak_reverse(h: number): number;
  ak_restart(h: number): number;
  ak_release(h: number): number;
  ak_set_start(h: number, s: number): number;
  ak_set_timing(
    h: number, delay: number, duration: number, iterations: number, direction: number,
    endDelay: number, easing: number,
  ): number;
  ak_track_enabled(h: number, track: number, enabled: number): number;
  ak_clear(): void;
  ak_values_ptr(h: number): number;
  ak_values_len(h: number): number;
  ak_alive(h: number): number;
  ak_time(h: number): number;
  ak_duration(h: number): number;
  ak_progress(h: number): number;
  ak_state(h: number): number;
  ak_iteration(h: number): number;
  ak_rate(h: number): number;
  ak_tick(dt: number): number;
  ak_updated_ptr(): number;
  ak_updated_len(): number;
  ak_events_ptr(): number;
  ak_events_len(): number;
  ak_clear_frame(): void;
  ak_set_lag_smoothing(threshold: number, adjusted: number): void;
  ak_set_global_rate(r: number): void;
  ak_active_count(): number;
  ak_node_count(): number;
  ak_color_parse(len: number): number;
  ak_color_mix(t: number, space: number): number;
  ak_batch_create(len: number): number;
  ak_batch_ptr(h: number): number;
  ak_noise2(seed: number, x: number, y: number): number;
  ak_noise3(seed: number, x: number, y: number, z: number): number;
  ak_fbm2(seed: number, x: number, y: number, octaves: number, lacunarity: number, gain: number): number;
  ak_fbm3(seed: number, x: number, y: number, z: number, octaves: number, lacunarity: number, gain: number): number;
  ak_noise3_bulk(seed: number, n: number): number;
  ak_path_trim(len: number, start: number, end: number, decimals: number): number;
  ak_path_through(n: number, curviness: number, closed: number, decimals: number): number;
  ak_path_simplify(len: number, tolerance: number, smooth: number, decimals: number): number;
  ak_path_area(len: number): number;
  ak_path_flatten(len: number, tolerance: number): number;
  ak_path_lut_closest(id: number, x: number, y: number): number;
  ak_path_lut_points(id: number, n: number): number;
  ak_morph_many(count: number): number;
  ak_physics_create(ch: number, friction: number, restitution: number, life: number, hasMin: number, hasMax: number): number;
  ak_sim_acceleration(h: number, ch: number): number;
  ak_sample(h: number, p: number): number;
  ak_world_create(gx: number, gy: number, damping: number, substeps: number, iterations: number, restitution: number, wallRestitution: number, friction: number): number;
  ak_world_config(id: number, gx: number, gy: number, damping: number, substeps: number, iterations: number, restitution: number, wallRestitution: number, friction: number): void;
  ak_world_free(id: number): void;
  ak_world_bounds(id: number, x0: number, y0: number, x1: number, y1: number): void;
  ak_world_add(id: number, n: number): number;
  ak_world_box(id: number, x0: number, y0: number, x1: number, y1: number): number;
  ak_world_clear_boxes(id: number): void;
  ak_world_constraint(id: number, a: number, b: number, length: number, stiffness: number, tear: number): number;
  ak_world_constraints(id: number): number;
  ak_world_attractors(id: number, n: number): void;
  ak_world_step(id: number, dt: number): void;
  ak_world_ptr(id: number): number;
  ak_world_len(id: number): number;
  ak_world_set_position(id: number, i: number, x: number, y: number, keepVelocity: number): void;
  ak_world_set_velocity(id: number, i: number, vx: number, vy: number): void;
  ak_world_velocity(id: number, i: number): number;
  ak_world_set_pinned(id: number, i: number, pinned: number): void;
  ak_world_set_active(id: number, i: number, on: number): void;
  ak_world_set_radius(id: number, i: number, r: number): void;
  ak_world_impulse(id: number, x: number, y: number, radius: number, strength: number): void;
  ak_world_pick(id: number, x: number, y: number): number;
  ak_world_energy(id: number): number;
  ak_world_pair_tests(id: number): number;
  ak_transform_parse(len: number): number;
  ak_matrix_mix(t: number): number;
  ak_camera(ex: number, ey: number, ez: number, tx: number, ty: number, tz: number, fov: number, aspect: number, near: number, far: number): number;
  ak_rotation(x: number, y: number, z: number): number;
  ak_projector_create(): number;
  ak_projector_free(id: number): void;
  ak_projector_points(id: number, len: number): number;
  ak_project(id: number, source: number, handle: number, stride: number, ox: number, oy: number, oz: number, n: number, w: number, h: number, sort: number): number;
  ak_projector_order(id: number): number;
  ak_cloud_create(n: number, shapes: number, seed: number): number;
  ak_cloud_free(id: number): void;
  ak_cloud_shape(id: number, k: number): number;
  ak_cloud_snap(id: number, k: number): void;
  ak_cloud_params(id: number): void;
  ak_cloud_step(id: number, dt: number, from: number, to: number, mix: number): number;
  ak_cloud_ptr(id: number): number;
  ak_cloud_explode(id: number, x: number, y: number, z: number, strength: number): void;
  ak_cloud_speed(id: number): number;
  ak_splat_create(w: number, h: number): number;
  ak_splat_free(id: number): void;
  ak_splat_resize(id: number, w: number, h: number): void;
  ak_splat_colors(id: number, n: number): number;
  ak_splat_fade(id: number, k: number): void;
  ak_splat_draw(id: number, pid: number, n: number): void;
  ak_splat_resolve(id: number, exposure: number, bg: number, keep: number): number;
  ak_splat_mode(id: number, ink: number): void;
  ak_video_create(w: number, h: number): number;
  ak_video_free(id: number): void;
  ak_video_input(id: number): number;
  ak_video_add(id: number): number;
  ak_video_resolve(id: number): void;
  ak_video_i420(id: number, order: number): number;
  ak_video_post(id: number, seed: number): void;
  ak_mesh_builtin(kind: number): number;
  ak_mesh_custom(nverts: number, ntris: number): number;
  ak_mesh_ptr(id: number, which: number): number;
  ak_mesh_free(id: number): void;
  ak_raster_create(w: number, h: number): number;
  ak_raster_free(id: number): void;
  ak_raster_resize(id: number, w: number, h: number): void;
  ak_raster_clear(id: number, rgb: number, alpha: number): void;
  ak_raster_instances(id: number, n: number): number;
  ak_raster_draw(id: number, mesh: number, n: number, cullBack: number, nf: number): number;
  ak_raster_ptr(id: number): number;
  ak_emitter_create(capacity: number, seed: number): number;
  ak_emitter_free(id: number): void;
  ak_emitter_params(id: number): void;
  ak_emitter_burst(id: number, n: number): void;
  ak_emitter_step(id: number, dt: number): number;
  ak_emitter_shade(id: number): void;
  ak_emitter_len(id: number): number;
  ak_emitter_ptr(id: number): number;
  ak_emitter_colors(id: number): number;
  ak_emitter_sizes(id: number): number;
  ak_emitter_clear(id: number): void;
  ak_audio_input(n: number): number;
  ak_audio_analyze(rate: number): number;
  ak_audio_hop(): number;
  ak_audio_result(kind: number): number;
  ak_audio_result_len(kind: number): number;
  ak_gif_new(w: number, h: number, loop: number): number;
  ak_gif_frame_ptr(id: number): number;
  ak_gif_add(id: number, delayCs: number): void;
  ak_gif_finish(id: number): number;
  ak_gif_ptr(id: number): number;
  ak_gif_free(id: number): void;
  ak_path_lut(len: number, samples: number): number;
  ak_path_lut_free(id: number): void;
  ak_path_lut_length(id: number): number;
  ak_path_lut_sample(id: number, t: number): number;
  ak_path_length(len: number): number;
  ak_path_bounds(len: number): number;
  ak_path_normalize(len: number, decimals: number): number;
  ak_morph(lenA: number, lenB: number): number;
  ak_stagger(
    count: number, each: number, total: number, start: number, from: number, fromValue: number,
    cols: number, rows: number, axis: number, ease: number,
  ): number;
}

/** Easing id meaning "linear" (u32::MAX in the core). */
export const LINEAR = 0xffffffff;
export const EASING_ERROR = 0xfffffffe;

export let W: Exports | null = null;
let buffer: ArrayBuffer | null = null;
export let F64: Float64Array = new Float64Array(0);
export let U32: Uint32Array = new Uint32Array(0);
export let U8: Uint8Array = new Uint8Array(0);

/** Re-create memory views if linear memory grew (cheap identity check). */
export function views(): void {
  const b = W!.memory.buffer;
  if (b !== buffer) {
    buffer = b as ArrayBuffer;
    F64 = new Float64Array(b);
    U32 = new Uint32Array(b);
    U8 = new Uint8Array(b);
  }
}

function decodeBase64(s: string): Uint8Array {
  if (typeof atob === 'function') {
    const bin = atob(s);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }
  // Node without atob (very old) — Buffer exists there.
  return new Uint8Array((globalThis as any).Buffer.from(s, 'base64'));
}

const queue: Array<() => void> = [];
let readyResolve!: () => void;
/** Resolves once the WebAssembly core is instantiated. */
export const ready: Promise<void> = new Promise<void>((r) => (readyResolve = r));

function install(instance: WebAssembly.Instance): void {
  W = instance.exports as unknown as Exports;
  views();
  readyResolve();
  // Flush operations requested before the core was ready, in order. One bad
  // call (an unknown easing, say) must not drop everything queued after it,
  // and its error must still surface.
  for (let i = 0; i < queue.length; i++) {
    try {
      queue[i]();
    } catch (e) {
      setTimeout(() => {
        throw e;
      });
    }
  }
  queue.length = 0;
}

let started = false;
function boot(): void {
  if (started) return;
  if (!WASM_BASE64) {
    // Lean build: stream the module that ships next to this file.
    void init(new URL('./anikit.wasm', import.meta.url));
    return;
  }
  started = true;
  const bytes = decodeBase64(WASM_BASE64);
  try {
    // Synchronous compile works in Node, workers, Firefox, Safari and in
    // Chromium when allowed; it lets the very first animate() run immediately.
    const mod = new WebAssembly.Module(bytes as BufferSource);
    install(new WebAssembly.Instance(mod, {}));
  } catch {
    WebAssembly.instantiate(bytes as BufferSource, {}).then((r) => install(r.instance));
  }
}

/**
 * Load the core from a URL instead of the embedded copy (smaller JS payload,
 * streaming compilation). Call before any other API.
 */
export async function init(source?: string | URL | Response | BufferSource): Promise<void> {
  if (W) return;
  if (source === undefined) {
    boot();
    return ready;
  }
  if (started) return ready;
  started = true;
  let result: WebAssembly.WebAssemblyInstantiatedSource;
  if (typeof source === 'string' || source instanceof URL) {
    const res = fetch(source);
    result = WebAssembly.instantiateStreaming
      ? await WebAssembly.instantiateStreaming(res, {})
      : await WebAssembly.instantiate(await (await res).arrayBuffer(), {});
  } else if (typeof Response !== 'undefined' && source instanceof Response) {
    result = await WebAssembly.instantiate(await source.arrayBuffer(), {});
  } else {
    result = await WebAssembly.instantiate(source as BufferSource, {});
  }
  install(result.instance);
}

/** Run `fn` now if the core is ready, otherwise as soon as it is. */
export function whenReady(fn: () => void): void {
  if (!W) boot();
  if (W) fn();
  else queue.push(fn);
}

/** Get the exports, booting synchronously if needed (throws if unavailable). */
export function core(): Exports {
  if (!W) boot();
  if (!W) {
    throw new Error(
      'anikit: the WebAssembly core is still loading. animate()/timeline() calls queue until it is ready, but functions that return a value right away ' +
        '(ease(), morph(), pathLength(), …) need it loaded: `await ready` (or `await init()` with anikit/lean) first.',
    );
  }
  return W;
}

// ---------------------------------------------------------------------------
// Scratch helpers
// ---------------------------------------------------------------------------

const encoder = typeof TextEncoder !== 'undefined' ? new TextEncoder() : null;
const decoder = typeof TextDecoder !== 'undefined' ? new TextDecoder() : null;

/** Write a string into the u8 scratch buffer at `offset`; returns byte length. */
export function writeStr(s: string, offset = 0): number {
  const w = core();
  const max = s.length * 3 + offset;
  w.ak_u8_buf(max);
  views();
  // Fast path for ASCII (path data, easings, colours are almost always ASCII).
  const base = w.ak_u8_buf(0) + offset;
  let ascii = true;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c > 127) {
      ascii = false;
      break;
    }
    U8[base + i] = c;
  }
  if (ascii) return s.length;
  const r = encoder!.encodeInto(s, U8.subarray(base, base + s.length * 3));
  return r.written!;
}

export function readStr(len: number): string {
  views();
  const p = core().ak_str_ptr();
  if (decoder) return decoder.decode(U8.subarray(p, p + len));
  let s = '';
  for (let i = 0; i < len; i++) s += String.fromCharCode(U8[p + i]);
  return s;
}

/** Reserve `n` f64 slots in scratch memory; returns the F64 index of slot 0. */
export function f64Buf(n: number): number {
  const p = core().ak_f64_buf(n);
  views();
  return p >>> 3;
}

/** Copy `n` f64 values starting at byte pointer `ptr` into a new array. */
export function readF64(ptr: number, n: number): number[] {
  views();
  const at = ptr >>> 3;
  const out = new Array<number>(n);
  for (let i = 0; i < n; i++) out[i] = F64[at + i];
  return out;
}

// Start compiling as soon as the module loads so the core is ready by the
// first frame (synchronously where the platform allows it).
boot();
