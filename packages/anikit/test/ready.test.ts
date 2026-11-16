// Calls made before the WebAssembly core is ready (anikit/lean, or browsers
// that refuse synchronous compilation): they queue, synchronous helpers explain
// what to do, and one failing queued call doesn't drop the others.
import { it, expect, vi } from 'vitest';

it('queues work before ready and keeps going past a failing call', async () => {
  vi.resetModules();
  const realModule = WebAssembly.Module;
  const realInstantiate = WebAssembly.instantiate;
  const realSetTimeout = globalThis.setTimeout;
  // Force the asynchronous path, and hold it until we say so.
  let release!: () => void;
  const gate = new Promise<void>((r) => (release = r));
  (WebAssembly as any).Module = function () {
    throw new Error('synchronous compilation disabled for this test');
  };
  (WebAssembly as any).instantiate = async (...args: Parameters<typeof WebAssembly.instantiate>) => {
    await gate;
    return realInstantiate(...args);
  };
  const rethrown: Array<() => void> = [];
  try {
    const A = await import('../src/index');
    expect(() => A.ease('outCubic', 0.5)).toThrow(/still loading.*`await ready`/s);

    const a = { x: 0 };
    const b = { y: 0 };
    A.animate(a, { x: 1 }, { ease: 'outCubik' });              // fails once the core is there
    A.animate(b, { y: 1 }, { duration: 0.1, ease: 'linear' });  // must still run
    (globalThis as any).setTimeout = (fn: () => void) => {
      rethrown.push(fn);
      return 0;
    };
    release();
    await A.ready;
    globalThis.setTimeout = realSetTimeout;

    expect(rethrown.length).toBe(1);
    expect(() => rethrown[0]()).toThrow(/unknown easing "outCubik".*Did you mean "outCubic"/);
    A.engine.manual(true);
    A.engine.tick(200);
    expect(b.y).toBe(1);
    expect(A.ease('outCubic', 0.5)).toBe(0.875);
  } finally {
    WebAssembly.Module = realModule;
    WebAssembly.instantiate = realInstantiate;
    globalThis.setTimeout = realSetTimeout;
  }
});

it('decodes the embedded module the same way with every base64 decoder', async () => {
  const { decodeBase64 } = await import('../src/wasm');
  const { WASM_BASE64 } = await import('../src/generated/wasm-bytes');
  const all = new Uint8Array(256 * 3 + 2).map((_, i) => (i * 7 + (i >> 8)) & 255);
  const samples = [WASM_BASE64, '', 'AA==', 'AAE=', 'AAEC', Buffer.from(all).toString('base64'), Buffer.from(all.subarray(1)).toString('base64')];
  const vias = ['node', 'atob', undefined] as const;
  const hasNative = typeof (Uint8Array as any).fromBase64 === 'function';
  for (const s of samples) {
    const ref = Array.from(Buffer.from(s, 'base64'));
    for (const via of vias) expect(Array.from(decodeBase64(s, via))).toEqual(ref);
    if (hasNative) expect(Array.from(decodeBase64(s, 'native'))).toEqual(ref);
  }
  expect(new WebAssembly.Module(decodeBase64(WASM_BASE64) as BufferSource)).toBeInstanceOf(WebAssembly.Module);
});
