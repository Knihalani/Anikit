import { describe, it, expect, beforeAll } from 'vitest';
import { ready, composition, FrameBuffer, emitter, analyzeAudio, muxWebM, animate, delayedCall, mesh, instances, renderer3d, scene3d, testing } from '../src/index';

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

describe('composition pixels()', () => {
  it('delivers handed-over pixels without the canvas, and puts them on it when the canvas is needed', async () => {
    const px = new Uint8ClampedArray(4 * 2 * 4).map((_, i) => (i * 37) & 255);
    const run = async (o: { blur?: number; keepCanvas?: boolean; impact?: boolean }) => {
      const c = composition({ width: 4, height: 2, fps: 10, duration: 0.2, motionBlur: o.blur });
      if (o.impact) c.impact(5, { shake: 0 });
      c.scene('s', 0, 1, { draw: (f) => f.pixels(px) });
      let put = 0, read = 0;
      const ctx: any = { fillRect() {}, drawImage() {}, putImageData: () => { put++; }, getImageData: () => { read++; return { data: px }; } };
      const out: number[][] = [];
      await c.render({ canvas: { width: 4, height: 2, getContext: () => ctx } as any, keepCanvas: o.keepCanvas, onFrame: (fr) => { out.push(Array.from(fr.rgba())); } });
      return { out, put, read };
    };
    const plain = await run({});
    expect(plain.out).toEqual([Array.from(px), Array.from(px)]);
    expect(plain.put + plain.read).toBe(0); // no canvas round trip
    expect((await run({ blur: 2 })).out[1]).toEqual(Array.from(px));
    const kept = await run({ keepCanvas: true });
    expect(kept.out).toEqual(plain.out);
    expect(kept.put).toBeGreaterThan(0);
    expect((await run({ impact: true })).put).toBeGreaterThan(0);
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

  it('lays out odd sizes like ffmpeg (chroma planes round up), so frames stay aligned', () => {
    for (const [w, h] of [[5, 3], [1, 1], [63, 35], [2, 3]]) {
      const fb = new FrameBuffer(w, h);
      fb.write(new Uint8Array(w * h * 4).fill(255));
      const y = fb.i420();
      const c = Math.ceil(w / 2) * Math.ceil(h / 2);
      expect(y.length).toBe(w * h + 2 * c);
      expect(Array.from(y.slice(0, w * h)).every((v) => v === 235)).toBe(true);
      // Every chroma sample is written (neutral grey for white), including the odd edge.
      expect(Array.from(y.slice(w * h)).every((v) => v >= 127 && v <= 129)).toBe(true);
      fb.free();
    }
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

describe('composition extras', () => {
  it('explains mistakes', () => {
    const c = composition({ width: 4, height: 2, duration: 2 });
    expect(() => c.scene('a', 1, 1, { draw() {} })).toThrow(/must be after start/);
    expect(() => c.scene('a', 0, 1, {} as any)).toThrow(/needs a draw/);
    c.scene('a', 0, 1, { draw() {} });
    expect(() => c.scene('b', 0.5, 2, { draw() {} })).toThrow(/overlaps "a".*transition/);
    expect(() => c.scene('b', 0.5, 2, { draw() {}, transition: { duration: 0.5 } })).not.toThrow();
  });

  it('impacts are pure functions of time', () => {
    const c = composition({ width: 4, height: 2, duration: 2 }).impact(1, { shake: 20, flash: '#fff', aberration: 5 });
    expect(c.impactAt(0.5)).toEqual({ x: 0, y: 0, flash: '', flashAlpha: 0, aberration: 0, shock: [0.5, 0.5, 0, 0, 0] });
    const a = c.impactAt(1.01), b = c.impactAt(1.01);
    expect(a).toEqual(b);
    expect(a.flashAlpha).toBeGreaterThan(0.5);
    expect(c.impactAt(1.8).aberration).toBeLessThan(a.aberration);
  });

  it('timeScale ramps engine time and passes dt to steps', async () => {
    const c = composition({ width: 4, height: 2, fps: 10, duration: 1 });
    let clock = 0;
    const o = { x: 0 };
    c.scene('s', 0, 1, { enter: () => { animate(o, { x: 100 }, { duration: 1, ease: 'linear' }); }, timeScale: () => 0.5, step: ({ dt }) => (clock += dt), draw() {} });
    const cv: any = { width: 4, height: 2, getContext: () => ({ fillRect() {}, getImageData: () => ({ data: new Uint8ClampedArray(32) }) }) };
    await c.render({ canvas: cv, onFrame: () => {} });
    expect(clock).toBeCloseTo(0.5, 5);
    expect(o.x).toBeCloseTo(50, 0);
  });

  it('reports where each frame\'s time went, down to renderer3d passes', async () => {
    const c = composition({ width: 32, height: 16, fps: 10, duration: 0.3, post: { vignette: 0.3 } });
    const r = renderer3d({ width: 32, height: 16 });
    const cam = scene3d({ width: 32, height: 16, camera: { z: 300 } });
    const inst = instances(1, () => ({ scale: 80 }));
    c.scene('s', 0, 0.3, { draw: () => { r.render(cam, [{ mesh: mesh.cube(), instances: inst }], { skybox: true }); } });
    const timings: any[] = [];
    const cv: any = { width: 32, height: 16, getContext: () => ({ fillRect() {}, drawImage() {}, getImageData: () => ({ data: new Uint8ClampedArray(32 * 16 * 4) }) }) };
    await c.render({ canvas: cv, onFrame: (f) => { timings.push(f.timing); } });
    expect(timings).toHaveLength(3);
    for (const t of timings) {
      for (const k of ['step', 'draw', 'read', 'post'] as const) expect(t[k]).toBeGreaterThanOrEqual(0);
      expect(Object.keys(t.passes)).toEqual(expect.arrayContaining(['opaque', 'sky']));
    }
    expect(Object.keys(r.stats)).toEqual(expect.arrayContaining(['setup', 'opaque', 'sky', 'output']));
  });

  it('warns when draw() keeps creating meshes or instance buffers', async () => {
    const warnings = testing.captureWarnings();
    const c = composition({ width: 4, height: 2, fps: 10, duration: 0.6 });
    c.scene('leaky', 0, 0.6, { draw: () => { instances(2); } });
    const cv: any = { width: 4, height: 2, getContext: () => ({ fillRect() {}, getImageData: () => ({ data: new Uint8ClampedArray(32) }) }) };
    await c.render({ canvas: cv, onFrame: () => {} });
    expect(warnings.filter((w) => /creates 1 mesh or instance buffers in every draw/.test(w))).toHaveLength(1);
    expect(warnings[0]).toMatch(/scene "leaky"/);
    warnings.length = 0;
    const ok = composition({ width: 4, height: 2, fps: 10, duration: 0.6 });
    const inst = instances(2);
    ok.scene('fine', 0, 0.6, { draw: () => { inst.set(0, { x: 1 }); } });
    await ok.render({ canvas: cv, onFrame: () => {} });
    expect(warnings).toHaveLength(0);
  });

  it('impacts can send out a shockwave that expands and fades', () => {
    const c = composition({ width: 400, height: 300, fps: 30, duration: 3 });
    c.impact(1, { shake: 0, shockwave: 30, origin: [0.25, 0.5] });
    expect(c.impactAt(0.9).shock[4]).toBe(0);
    const a = c.impactAt(1.1).shock, b = c.impactAt(1.3).shock;
    expect(a.slice(0, 2)).toEqual([0.25, 0.5]);
    expect(b[2]).toBeGreaterThan(a[2]); // expands
    expect(Math.abs(b[4])).toBeLessThan(Math.abs(a[4])); // and fades
    expect(c.impactAt(2.9).shock[4]).toBe(0); // gone once it has left the frame
  });

  it('adds lens flares opposite bright lights', async () => {
    const [w, h] = [128, 64];
    const src = new Uint8ClampedArray(w * h * 4);
    for (let i = 3; i < src.length; i += 4) src[i] = 255;
    for (let y = 10; y < 16; y++) for (let x = 20; x < 28; x++) src.fill(255, (y * w + x) * 4, (y * w + x) * 4 + 4);
    const ghost = async (flares: number) => {
      const c = composition({ width: w, height: h, fps: 10, duration: 0.1, post: { flares, bloomThreshold: 0.5, vignette: 0.0001 } });
      c.scene('s', 0, 0.1, { draw() {} });
      let sum = 0;
      const cv: any = { width: w, height: h, getContext: () => ({ fillRect() {}, drawImage() {} }) };
      await c.render({ canvas: cv, readPixels: () => src, onFrame: (f) => {
        const px = f.rgba();
        for (let y = h - 16; y < h - 10; y++) for (let x = w - 28; x < w - 20; x++) sum += px[(y * w + x) * 4] + px[(y * w + x) * 4 + 2];
      } });
      return sum;
    };
    expect(await ghost(0)).toBe(0);
    expect(await ghost(2)).toBeGreaterThan(100);
  });

  it('keeps a scene clock and maps it back to composition time', async () => {
    const c = composition({ width: 4, height: 2, fps: 10, duration: 2, motionBlur: 3 });
    const ramp = (l: number) => (l < 1 ? 1 : 0.25);
    const seen: number[] = [];
    let own = 0;
    c.scene('s', 0, 2, { timeScale: ramp, step: ({ dt }) => (own += dt), draw: ({ clock }) => { seen.push(clock); expect(clock).toBeCloseTo(own, 9); } });
    const cv: any = { width: 4, height: 2, getContext: () => ({ fillRect() {}, drawImage() {}, getImageData: () => ({ data: new Uint8ClampedArray(32) }) }) };
    await c.render({ canvas: cv, onFrame: () => {} });
    expect(seen.length).toBeGreaterThan(0);
    expect(c.clockAt('s', 0.9)).toBeCloseTo(1, 6);
    expect(c.clockAt('s', 1.9)).toBeCloseTo(1.25, 6);
    expect(c.timeAt('s', 1.25)).toBeCloseTo(1.9, 6);
    expect(c.timeAt('s', 0.5)).toBeCloseTo(0.4, 6);
    expect(() => c.timeAt('nope', 1)).toThrow(/no scene named "nope".*scenes: s/);
  });
});

describe('renderer3d', () => {
  it('rasterises lit, depth-tested instanced meshes', () => {
    const r = renderer3d({ width: 64, height: 48 });
    const cam = scene3d({ width: 64, height: 48, camera: { z: 300 } });
    const inst = instances(2, (i) => ({ x: i * 1000, scale: 80, rx: 20, ry: 30, color: '#ff3b14' }));
    const px = r.render(cam, [{ mesh: mesh.cube(), instances: inst }], { background: '#000000' });
    expect(px.length).toBe(64 * 48 * 4);
    const c = (32 * 64 + 32) * 4;
    expect(px[c]).toBeGreaterThan(60); // red face in the middle
    expect(px[c]).toBeGreaterThan(px[c + 1] * 2); // and still red: the material block lines up with the wasm ABI
    expect(px[0]).toBe(0); // background
    expect(r.triangles).toBeGreaterThan(0);
    expect(mesh.torusKnot({ segments: 64, sides: 8 }).triangles).toBe(64 * 8 * 2);
  });

  it('applies chrome, grid lines and pulses per pixel', () => {
    const r = renderer3d({ width: 64, height: 48 });
    const cam = scene3d({ width: 64, height: 48, camera: { z: 300 } });
    const inst = instances(1, () => ({ scale: 120, rx: 20, ry: 30, color: [0.05, 0.05, 0.05] }));
    const sum = (px: Uint8ClampedArray, ch: number) => { let s = 0; for (let i = ch; i < px.length; i += 4) s += px[i]; return s; };
    const plain = sum(r.render(cam, [{ mesh: mesh.cube(), instances: inst }], {}).slice(), 2);
    const lined = sum(r.render(cam, [{ mesh: mesh.cube(), instances: inst, material: { lines: 2, lineColor: [0, 0, 1], grid: [3, 3] } }], {}).slice(), 2);
    expect(lined).toBeGreaterThan(plain * 1.5);
    const chrome = r.render(cam, [{ mesh: mesh.cube(), instances: inst, material: { metal: 1 } }], { sky: [0, 1, 0], horizon: [0, 1, 0], ground: [0, 1, 0] }).slice();
    expect(sum(chrome, 1)).toBeGreaterThan(sum(chrome, 0) * 3); // reflects the green environment
  });
});
