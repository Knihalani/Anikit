import { describe, it, expect, beforeAll } from 'vitest';
import { ready, composition, FrameBuffer, emitter, analyzeAudio, muxWebM, animate, delayedCall } from '../src/index';

beforeAll(async () => { await ready; });

/** A fake 2D canvas whose "pixels" encode a value we draw. */
function fakeCanvas(w: number, h: number) {
  let v = 0;
  const ctx: any = {
    fillStyle: '', fillRect() {}, drawImage() {},
    set value(x: number) { v = x; },
    getImageData: () => ({ data: new Uint8ClampedArray(w * h * 4).fill(v) }),
  };
  return { width: w, height: h, getContext: () => ctx, ctx };
}

describe('composition', () => {
  it('renders scenes deterministically with cues and timelines on schedule', async () => {
    const make = () => {
      const log: string[] = [];
      const st = { x: 0 };
      const c = composition({ width: 4, height: 2, fps: 10, duration: 2 });
      c.scene('a', 0, 1, { enter: () => { log.push('enter a'); animate(st, { x: 100 }, { duration: 1, ease: 'linear' }); delayedCall(0.5, () => log.push('half')); }, draw: (f) => (f.ctx as any).value = Math.round(st.x) });
      c.scene('b', 1, 2, { enter: () => log.push('enter b'), draw: (f) => (f.ctx as any).value = 200 });
      c.cue(1.5, () => log.push('cue'));
      return { c, log };
    };
    const run = async () => {
      const { c, log } = make();
      const cv = fakeCanvas(4, 2);
      const vals: number[] = [];
      await c.render({ canvas: cv as any, onFrame: (fr) => { vals.push(fr.rgba()[0]); } });
      return { vals, log };
    };
    const a = await run(), b = await run();
    expect(a.vals.length).toBe(20);
    expect(a.vals).toEqual(b.vals);
    expect(a.vals[5]).toBe(60); // x after 6 ticks of 0.1 s
    expect(a.vals[15]).toBe(200);
    expect(a.log).toEqual(['enter a', 'half', 'enter b', 'cue']);
  });

  it('averages motion-blur sub-frames and converts to I420', async () => {
    const c = composition({ width: 4, height: 2, fps: 10, duration: 0.1, motionBlur: { samples: 2, shutter: 1 } });
    let k = 0;
    c.scene('s', 0, 1, { draw: (f) => ((f.ctx as any).value = k++ % 2 ? 200 : 100) });
    const cv = fakeCanvas(4, 2);
    let px = 0, yuv = 0;
    await c.render({ canvas: cv as any, onFrame: (fr) => { px = fr.rgba()[0]; yuv = fr.i420().length; } });
    expect(px).toBe(150);
    expect(yuv).toBe(4 * 2 * 1.5);
  });
});

describe('transitions', () => {
  it('runs both scenes during an overlap and composites them', async () => {
    const log: string[] = [];
    const draws: Record<string, number> = { a: 0, b: 0 };
    const mk = () => {
      const ctx: any = { fillStyle: '', globalAlpha: 1, globalCompositeOperation: '', fillRect() {}, clearRect() {}, setTransform() {}, save() {}, restore() {},
        drawImage: (c: any) => log.push('img:' + c.tag), getImageData: () => ({ data: new Uint8ClampedArray(4 * 2 * 4) }) };
      return ctx;
    };
    let n = 0;
    const createCanvas = () => { const c: any = { width: 4, height: 2, tag: 'buf' + n++ }; const x = mk(); c.getContext = () => x; return c; };
    const c = composition({ width: 4, height: 2, fps: 10, duration: 2 });
    c.scene('a', 0, 1, { enter: () => log.push('enter a'), step: () => {}, draw: () => draws.a++ });
    c.scene('b', 1, 2, { enter: () => log.push('enter b'), transition: { duration: 0.5, type: 'push' }, draw: () => draws.b++ });
    expect(c.scenesAt(1.2).map((s) => s.name)).toEqual(['a', 'b']);
    expect(c.scenesAt(1.6).map((s) => s.name)).toEqual(['b']);
    const main: any = { width: 4, height: 2, getContext: () => mainCtx };
    const mainCtx = mk();
    await c.render({ canvas: main, createCanvas, onFrame: () => {} });
    expect(log.filter((l) => l.startsWith('enter'))).toEqual(['enter a', 'enter b']);
    expect(draws.a).toBe(15); // 10 frames alone + 5 during the transition
    expect(draws.b).toBe(10);
    expect(log.filter((l) => l === 'img:buf0').length).toBeGreaterThan(0);
  });
});

describe('FrameBuffer', () => {
  it('converts white to BT.709 limited-range Y', () => {
    const fb = new FrameBuffer(2, 2);
    fb.write(new Uint8Array(16).fill(255));
    const y = fb.i420();
    expect(Array.from(y.slice(0, 4))).toEqual([235, 235, 235, 235]);
    fb.free();
  });
});

describe('emitter', () => {
  it('emits, ages and shades particles in WebAssembly', () => {
    const e = emitter({ rate: 600, life: 0.5, lifeVariation: 0, autoStep: false, colors: ['#ff0000', '#0000ff'] });
    for (let i = 0; i < 30; i++) e.step(1 / 60);
    expect(e.count).toBeGreaterThan(250);
    expect(e.positions.length).toBe(e.count * 3);
    e.shade();
    expect(e.colors[0]).toBeGreaterThan(0);
    e.burst(5000);
    expect(e.count).toBeLessThanOrEqual(e.capacity);
    e.destroy();
  });
});

describe('analyzeAudio', () => {
  it('finds 120 BPM in a click track', () => {
    const rate = 22050, n = rate * 8, s = new Float32Array(n);
    for (let t = 0.25; t < 8; t += 0.5) for (let i = 0; i < 300; i++) s[Math.floor(t * rate) + i] = Math.sin(i * 1.7) * (1 - i / 300);
    const a = analyzeAudio(s, rate);
    expect(Math.abs(a.bpm - 120)).toBeLessThan(4);
    expect(a.beats.length).toBeGreaterThan(12);
    expect(a.at(0.26)).toBeGreaterThan(a.at(0.4));
    expect(a.beatAt(a.beats[3] + 0.01).index).toBe(3);
  });
});

describe('muxWebM', () => {
  it('writes an EBML/WebM container', () => {
    const bytes = muxWebM([{ data: new Uint8Array([1, 2, 3]), timestamp: 0, key: true }, { data: new Uint8Array([4]), timestamp: 33333, key: false }], { width: 16, height: 16, codec: 'V_VP9', duration: 0.066 });
    expect(Array.from(bytes.slice(0, 4))).toEqual([0x1a, 0x45, 0xdf, 0xa3]);
    expect(new TextDecoder().decode(bytes).includes('webm')).toBe(true);
    expect(new TextDecoder().decode(bytes).includes('V_VP9')).toBe(true);
  });
});
