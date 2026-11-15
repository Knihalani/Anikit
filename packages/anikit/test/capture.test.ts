import { describe, it, expect, beforeAll } from 'vitest';
import { encodeGif, GifWriter, engine } from '../src/index';
import { setup } from './setup';

beforeAll(setup);

describe('capture', () => {
  it('encodes animated GIFs in the core', () => {
    const w = 8;
    const h = 4;
    const frames = [0, 1, 2].map((k) => {
      const f = new Uint8Array(w * h * 4);
      for (let i = 0; i < w * h; i++) f.set([i === k ? 255 : 20, 40, 60, 255], i * 4);
      return f;
    });
    const gif = encodeGif(frames, w, h, 10);
    expect(String.fromCharCode(...gif.slice(0, 6))).toBe('GIF89a');
    expect(gif[gif.length - 1]).toBe(0x3b);
    expect(gif[6] | (gif[7] << 8)).toBe(8);
    const g = new GifWriter(w, h);
    expect(() => g.add(new Uint8Array(5), 0.1)).toThrow(/expected 8×4×4/);
    g.finish();
  });

  it('exposes a global engine handle for recorders and agents', () => {
    const handle = (globalThis as any).__ANIKIT__;
    expect(handle).toBe(engine);
    expect(typeof handle.tick).toBe('function');
    expect(Array.isArray(handle.inspect())).toBe(true);
  });
});
