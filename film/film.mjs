// ANIKIT — the film. A composition module for `anikit-render`:
//
//   node soundtrack.mjs soundtrack.wav
//   anikit-render film.mjs --out anikit-film.mp4 --audio soundtrack.wav --workers 3
//
// Every moving thing is anikit: particle clouds and emitters simulated and lit
// in WebAssembly, rigid-body physics, 3D projection and depth sorting, tweens
// and timelines on the camera, easing curves, kinetic type — and cuts placed
// on beats that anikit detected in the soundtrack.
import { synth } from './soundtrack.mjs';

export default async function film({ anikit: A, node }) {
  const { animate, pointCloud, glow, scene3d, textPoints, world, ease, noise2, delayedCall, emitter, typeFx, analyzeAudio, composition } = A;
  const W = 1920, H = 1080;
  const FONT = '/usr/share/fonts';
  node.registerFont(`${FONT}/opentype/inter/InterDisplay-Black.otf`, 'Display');
  node.registerFont(`${FONT}/opentype/inter/InterDisplay-BlackItalic.otf`, 'DisplayItalic');
  node.registerFont(`${FONT}/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf`, 'Mono');

  const C = { paper: '#ece6da', ink: '#0c0c0c', red: '#ff3b14', blue: '#1f2bff', lime: '#d2ff3c', black: '#050505' };
  const TAU = Math.PI * 2;
  const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
  const prog = (t, a, b) => clamp((t - a) / (b - a));
  const io = (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);

  // The soundtrack, analysed by anikit: beats drive cuts, strikes and pulses.
  const music = synth(60);
  const beat = analyzeAudio(music.mono, music.rate);
  const beatsIn = (a, b) => beat.beats.filter((t) => t >= a - 1e-3 && t < b - 1e-3);

  // Particles → light/ink, rendered by anikit's glow() into a Skia canvas.
  const target = node.glowTarget(W, H);
  const g = glow(target, { resolution: 0.85, background: C.black, mode: 'light', trail: 0.5, budget: 0 });
  g.resize(W, H);
  const cam = scene3d({ fov: 40, camera: { z: 1000 }, width: W, height: H, near: 4, far: 30000 });
  const camReset = (z = 1000) => { Object.assign(cam.camera, { x: 0, y: 0, z }); Object.assign(cam.target, { x: 0, y: 0, z: 0 }); Object.assign(cam.rotation, { x: 0, y: 0, z: 0 }); };

  // ---- Shapes (160,000 points each) -------------------------------------------
  const N = 160_000;
  let seed = 11;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const gauss = () => (rnd() + rnd() + rnd() - 1.5) * 1.15;
  const make = (fn) => { const a = new Float32Array(N * 3); for (let i = 0; i < N; i++) fn(i, a, i * 3); return a; };
  const fib = (i, n) => { const y = 1 - ((i + 0.5) / n) * 2, r = Math.sqrt(1 - y * y), t = i * 2.399963; return [Math.cos(t) * r, y, Math.sin(t) * r]; };
  const text = (s, width, dy = 0, depth = 12) => { const t = textPoints(s, N, { font: 'Display', weight: 900, width, depth }); for (let i = 1; i < t.length; i += 3) t[i] += dy; return t; };
  const shapes = {
    origin: make((i, a, o) => { const c = fib(i, N), r = 1.5 * Math.cbrt(rnd()); a[o] = c[0] * r; a[o + 1] = c[1] * r; a[o + 2] = c[2] * r; }),
    blast: make((i, a, o) => { const c = fib(i, N), r = 170 * Math.cbrt(0.2 + 0.8 * rnd()); a[o] = c[0] * r; a[o + 1] = c[1] * r; a[o + 2] = c[2] * r; }),
    debris: (() => { const K = 500, cs = Array.from({ length: K }, (_, k) => { const c = fib(k, K), r = 150 * Math.cbrt(rnd()); return [c[0] * r, c[1] * r, c[2] * r]; });
      return make((i, a, o) => { const c = cs[i % K]; a[o] = c[0] + gauss() * 6; a[o + 1] = c[1] + gauss() * 6; a[o + 2] = c[2] + gauss() * 6; }); })(),
    title: text('anikit', 1150, 40, 180),
    compiled: text('COMPILED.', 1180, 10),
    knot: make((i, a, o) => {
      const t = (i / N) * TAU, p = 2, q = 5, R = 300, rr = 130, r = R + rr * Math.cos(q * t), tube = 34 * Math.sqrt(rnd()), ang = rnd() * TAU;
      a[o] = r * Math.cos(p * t) + tube * Math.cos(ang); a[o + 1] = r * Math.sin(p * t) + tube * Math.sin(ang); a[o + 2] = rr * 1.8 * Math.sin(q * t) + tube * Math.cos(ang * 1.7);
    }),
    wasm: text('WASM.', 1150, 0),
    galaxy: make((i, a, o) => {
      if (i % 9 === 0) { const c = fib(i, N), r = 90 * Math.pow(rnd(), 1.5); a[o] = c[0] * r; a[o + 1] = c[1] * r * 0.6; a[o + 2] = c[2] * r; return; }
      const arm = i % 4, d = Math.pow(rnd(), 0.7), r = 40 + d * 1250, ang = (arm / 4) * TAU + d * 5.2 + gauss() * 0.22 * (1.1 - d);
      a[o] = Math.cos(ang) * r + gauss() * 22; a[o + 1] = gauss() * (26 * (1 - d) + 5); a[o + 2] = Math.sin(ang) * r + gauss() * 22;
    }),
    wordmark: text('anikit', 1050, 110, 30),
  };
  const names = Object.keys(shapes);
  const S = Object.fromEntries(names.map((n, i) => [n, i]));
  const cloud = pointCloud(N, { autoStep: false, shapes: names.map((n) => shapes[n]), stiffness: 34, damping: 7.5, spread: 0.7, noise: 1.6, swirl: 60, frequency: 0.012 });
  cloud.snap(S.origin);
  g.colors(N, () => { const r = rnd(); const k = r < 0.05 ? 1.7 : 0.5 + rnd() * 0.5; return [k, k, k]; });
  /** Start a particle scene on a clean light buffer (no trails from the last one). */
  const clearGlow = () => { const k = g.options.trail; g.options.trail = 0; g.render(cam, cloud, { exposure: 0 }); g.options.trail = k; };
  const flow = (to, dur, e = 'inOutCubic') => { cloud.from = cloud.mix >= 0.5 ? cloud.to : cloud.from; cloud.to = S[to]; cloud.mix = 0; animate(cloud, { mix: 1 }, { duration: dur, ease: e }); };
  const TC = { c: 1 };

  // Sparks: an anikit emitter, projected in WebAssembly and drawn as light or ink.
  const sparks = emitter({ capacity: 12000, rate: 0, autoStep: false, life: 1.1, lifeVariation: 0.4, speed: 1500, speedVariation: 0.6, spread: Math.PI, gravity: [0, -500, 0], drag: 1.6, turbulence: 900, frequency: 0.008 });

  // ---- Drawing helpers ------------------------------------------------------------
  let ctx;
  const label = (s, x, y, color, align = 'left', size = 15) => { ctx.font = `400 ${size}px Mono`; ctx.fillStyle = color; ctx.textAlign = align; ctx.textBaseline = 'alphabetic'; ctx.fillText(s.toUpperCase(), x, y); };
  const big = (s, x, y, px, color, align = 'left') => { ctx.font = `900 ${px}px Display`; ctx.textAlign = align; ctx.textBaseline = 'alphabetic'; ctx.fillStyle = color; ctx.fillText(s, x, y); };
  const fill = (c) => { ctx.fillStyle = c; ctx.fillRect(0, 0, W, H); };
  const particles = (o = {}) => {
    const ink = g.options.mode === 'ink';
    g.render(cam, cloud, { size: o.size ?? (ink ? 2.1 : 1.2), intensity: o.intensity ?? (ink ? 1.6 : 0.36), fog: o.fog ?? [500, 4200], exposure: o.exposure ?? 1, tint: o.tint, maxRadius: o.maxRadius ?? 5 });
    ctx.drawImage(target.canvas, 0, 0, W, H);
  };
  const drawSparks = (light, base = 2.2) => {
    if (!sparks.count) return;
    sparks.shade();
    const p = cam.project(sparks, { sort: false }), col = sparks.colors, sz = sparks.sizes;
    ctx.save();
    if (light) ctx.globalCompositeOperation = 'lighter';
    for (let i = 0; i < p.count; i++) {
      const sc = p.data[i * 4 + 3];
      if (sc <= 0) continue;
      const r = base * sz[i] * sc;
      if (r < 0.2 || r > 40) continue;
      ctx.fillStyle = `rgb(${(col[i * 3] * 255) | 0},${(col[i * 3 + 1] * 255) | 0},${(col[i * 3 + 2] * 255) | 0})`;
      ctx.fillRect(p.data[i * 4] - r, p.data[i * 4 + 1] - r, r * 2, r * 2);
    }
    ctx.restore();
  };
  const fx = (s, x, y, size, color, start, t, o = {}) => typeFx(ctx, s, x, y, { size, color, start, t, font: o.italic ? 'DisplayItalic' : 'Display', ...o });

  // ---- The film -----------------------------------------------------------------
  const film = composition({ width: W, height: H, fps: 60, duration: 60 });
  const movements = [];
  const movement = (name, start, end, bg, fg, hooks) => { movements.push({ name, start, end, bg, fg }); film.scene(name, start, end, { ...hooks, draw: (f) => { ctx = f.ctx; hooks.draw(f); chrome(f, name); } }); };

  // 01 · Frozen — 160,000 particles hang mid-explosion; the camera walks out of them.
  movement('Frozen', 0, 4, C.black, C.paper, {
    enter() {
      g.options.mode = 'light'; g.options.background = C.black; g.options.trail = 0.35;
      clearGlow();
      camReset(130); cam.rotation.x = 14;
      cloud.snap(S.debris); cloud.explode(0, 0, 0, 3400);
      for (let k = 0; k < 28; k++) cloud.step(1 / 60);
      TC.c = 0; cloud.from = cloud.to = S.debris; cloud.mix = 0;
      animate(cam.rotation, { y: [-50, 110], x: [24, 8] }, { duration: 3.4, ease: 'inOutCubic' });
      animate(cam.camera, { z: [130, 1250] }, { duration: 3.4, ease: 'inOutCubic' });
      delayedCall(3.0, () => animate(TC, { c: 1 }, { duration: 0.3, ease: 'inQuad' }));
      delayedCall(3.25, () => flow('origin', 0.7, 'inExpo'));
      delayedCall(3.85, () => { sparks.set({ colors: ['#ffffff', '#ffd9a0', '#ff6a2a', '#200800'], origin: [0, 0, 0], gravity: [0, -400, 0] }); sparks.burst(9000); });
    },
    step: (f) => { cloud.step((1 / 60) * TC.c); sparks.step(1 / 60); },
    draw(f) {
      particles({ tint: [1, 0.68, 0.42], intensity: 0.62, size: 1.6, maxRadius: 9, fog: [200, 3800] });
      drawSparks(true, 2.6);
      big(`${TC.c.toFixed(2)}×`, W - 80, H - 100, 170, C.paper, 'right');
      label(TC.c < 0.01 ? '160,000 particles · time stopped · camera still moving' : 'time resumes', W - 86, H - 62, C.paper, 'right');
    },
  });

  // 02 · Title — the extruded wordmark condenses out of an ink spray.
  movement('Title', 4, 8, C.paper, C.ink, {
    enter() {
      g.options.mode = 'ink'; g.options.background = C.paper; g.options.trail = 0.5;
      camReset(1000);
      cloud.snap(S.origin); cloud.explode(0, 0, 0, 2200);
      clearGlow();
      cloud.from = S.origin; cloud.to = S.title; cloud.mix = 0;
      animate(cloud, { mix: 1 }, { duration: 1.5, ease: 'outCubic' });
      animate(cam.rotation, { y: [-34, 22], x: [18, 4] }, { duration: 4, ease: 'inOutCubic' });
      animate(cam.camera, { z: [1250, 980] }, { duration: 4, ease: 'outCubic' });
      sparks.clear(); sparks.set({ colors: ['#0c0c0c', '#0c0c0c', '#ff3b14', '#ece6da'], gravity: [0, -900, 0], speed: 1300 }); sparks.burst(6000);
    },
    step: () => { cloud.step(1 / 60); sparks.step(1 / 60); },
    draw(f) {
      particles({ intensity: 1.9 });
      drawSparks(false, 2.2);
      fx('Motion,', 80, H - 190, 118, C.ink, 5.2, f.t);
      fx('compiled.', 80, H - 76, 118, C.red, 5.35, f.t, { italic: true });
      label('a web animation engine · rust → webassembly', W - 86, H - 62, C.ink, 'right');
    },
  });

  // 03 · Sand — a word of 9,000 rigid bodies collapses, then time runs backwards.
  let sand = null;
  movement('Sand', 8, 18, C.lime, C.ink, {
    enter() {
      const off = node.createCanvas(W, H), o = off.getContext('2d');
      o.font = '900 400px Display'; o.textAlign = 'center'; o.textBaseline = 'middle'; o.fillStyle = '#fff';
      o.fillText('MOTION', W / 2, H / 2 - 40);
      const img = o.getImageData(0, 0, W, H).data, pts = [], sp = 5.2;
      for (let y = 0; y < H; y += sp) for (let x = 0; x < W; x += sp) if (img[(Math.round(y) * W + Math.round(x)) * 4 + 3] > 140) pts.push({ x: x + (rnd() - 0.5) * 0.6, y: y + (rnd() - 0.5) * 0.6, r: 2.45, mass: 1 });
      const w = world({ bounds: [0, -4000, W, H - 96], gravity: [0, 0], substeps: 3, iterations: 2, restitution: 0.08, friction: 0.35, damping: 0.03 });
      w.addMany(pts);
      sand = { w, n: pts.length, hist: [], mode: 'hold', k: 0 };
      delayedCall(1.5, () => { sand.mode = 'fall'; w.set({ gravity: [0, 1700] }); for (let i = 0; i < sand.n; i++) w.setVelocity(i, (rnd() - 0.5) * 120, -rnd() * 160); });
      delayedCall(5.4, () => { sand.mode = 'rewind'; w.pause(); sand.k = sand.hist.length - 1; animate(sand, { k: 0 }, { duration: 3.4, ease: 'inOutCubic', delay: 0.2, onComplete: () => (sand.mode = 'done') }); });
    },
    step() {
      const s = sand;
      if (s.mode === 'fall' || (s.mode === 'hold' && !s.hist.length)) {
        const d = s.w.data, snap = new Float32Array(s.n * 2);
        for (let i = 0; i < s.n; i++) { snap[i * 2] = d[i * 3]; snap[i * 2 + 1] = d[i * 3 + 1]; }
        s.hist.push(snap);
      }
    },
    draw(f) {
      const s = sand, frame = s.mode === 'rewind' || s.mode === 'done' ? s.hist[Math.round(s.k)] : s.hist[s.hist.length - 1];
      fill(C.lime);
      ctx.fillStyle = C.ink;
      for (let i = 0; i < s.n; i++) ctx.fillRect(frame[i * 2] - 2.25, frame[i * 2 + 1] - 2.25, 4.5, 4.5);
      ctx.fillRect(0, H - 96 + 2.5, W, 2);
      label(`${s.n.toLocaleString('en-US')} rigid bodies · one webassembly physics world`, 86, H - 48, C.ink);
      const rw = s.mode === 'rewind';
      big(rw ? '◀◀ −1×' : s.mode === 'done' ? '■' : '1×', W - 80, 300, rw ? 150 : 110, rw ? C.red : C.ink, 'right');
      if (s.mode === 'done') fx('Deterministic. Rewindable.', 86, 210, 70, C.ink, s.doneAt ?? (s.doneAt = f.t), f.t);
    },
  });

  // 04 · Through — type into a torus knot; the camera dives through it on the drop.
  movement('Through', 18, 28, C.red, C.ink, {
    enter() {
      g.options.mode = 'ink'; g.options.background = C.red; g.options.trail = 0.5;
      clearGlow();
      camReset(1000);
      cloud.snap(S.blast); cloud.explode(0, 0, 0, 2600);
      cloud.from = S.blast; cloud.to = S.compiled; cloud.mix = 0;
      animate(cloud, { mix: 1 }, { duration: 1.3, ease: 'outCubic' });
      delayedCall(2.4, () => flow('knot', 1.3));
      animate(cam.rotation, { y: [0, 360], x: [0, 30, 0] }, { duration: 3.4, ease: 'inOutCubic', delay: 2.6 });
      delayedCall(6.0, () => { animate(cam.camera, { z: [1000, -900] }, { duration: 2.0, ease: 'inOutQuart' }); animate(cam.target, { z: [0, -1900] }, { duration: 2.0, ease: 'inOutQuart' }); });
      delayedCall(8.0, () => { camReset(1000); cloud.snap(S.knot); cloud.explode(0, 0, 0, 900); flow('wasm', 1.1, 'outCubic'); });
      // Every beat of the drop kicks the particles outward a little.
      for (const b of beatsIn(18, 28)) delayedCall(b - 18, () => cloud.explode(0, 0, 0, 160));
    },
    step: () => cloud.step(1 / 60),
    draw(f) {
      particles({ intensity: 1.8, maxRadius: 8 });
      label('160,000 particles · ink on vermilion · pulsing on every detected beat', 86, H - 48, C.ink);
      const lt = f.local;
      label(lt < 2.4 ? 'type → points' : lt < 6 ? 'points → torus knot (2,5) · orbit' : lt < 8 ? 'camera dives through the knot' : 'knot → type', W - 86, H - 48, C.ink, 'right');
    },
  });

  // 05 · Springs — a sheet of 3,080 springs struck on the beat.
  let sheet = null;
  movement('Springs', 28, 34, C.blue, C.paper, {
    enter() {
      const cols = 70, rows = 44, pts = [];
      for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) pts.push({ x: (c / (cols - 1) - 0.5) * 2200, z: (r / (rows - 1) - 0.5) * 1500, h: 0 });
      const spots = [[0, 0], [-600, 300], [520, -260], [-300, -380], [700, 360], [200, 120], [-520, -60], [380, 300], [0, -300], [-200, 380], [640, -120], [0, 0]];
      const hits = beatsIn(28.2, 34).map((at, k) => ({ x: spots[k % spots.length][0], z: spots[k % spots.length][1], at: at - 28, amp: k % 4 === 0 ? 380 : 230 }));
      sheet = { cols, rows, pts, xyz: new Float32Array(cols * rows * 3), hits: hits.map((h) => ({ ...h, d: pts.map((p) => Math.hypot(p.x - h.x, p.z - h.z)) })) };
      camReset(950); cam.rotation.x = 36;
      animate(cam.rotation, { y: [-18, 24] }, { duration: 6, ease: 'inOutSine' });
    },
    step() {},
    draw(f) {
      fill(C.blue);
      const { cols, rows, pts, xyz } = sheet;
      for (let i = 0; i < pts.length; i++) {
        let h = 0;
        for (const hit of sheet.hits) {
          const tau = f.local - hit.at - hit.d[i] / 1500;
          if (tau <= 0 || tau > 1.8) continue;
          h += -hit.amp * Math.exp(-hit.d[i] / 650) * ease('outCubic', Math.min(1, tau / 0.16)) * (1 - ease('spring(bounce: 0.62, duration: 1.5)', tau / 1.8));
        }
        pts[i].h = h;
        xyz[i * 3] = pts[i].x; xyz[i * 3 + 1] = -h; xyz[i * 3 + 2] = pts[i].z;
      }
      const p = cam.project(xyz, { sort: false }), X = (i) => p.data[i * 4], Y = (i) => p.data[i * 4 + 1];
      ctx.lineWidth = 1.5; ctx.strokeStyle = 'rgba(236,230,218,0.55)';
      ctx.beginPath();
      for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) { const i = r * cols + c; if (c === 0) ctx.moveTo(X(i), Y(i)); else ctx.lineTo(X(i), Y(i)); }
      for (let c = 0; c < cols; c++) for (let r = 0; r < rows; r++) { const i = r * cols + c; if (r === 0) ctx.moveTo(X(i), Y(i)); else ctx.lineTo(X(i), Y(i)); }
      ctx.stroke();
      for (let i = 0; i < pts.length; i++) {
        const sc = p.data[i * 4 + 3];
        if (sc <= 0) continue;
        const lift = clamp(-pts[i].h / 140);
        ctx.fillStyle = lift > 0.2 ? C.lime : C.paper;
        const r = (1.8 + lift * 3.2) * sc;
        ctx.fillRect(X(i) - r, Y(i) - r, r * 2, r * 2);
      }
      fx('Springs keep their velocity.', 86, 190, 72, C.paper, 28.3, f.t, { stagger: 0.022 });
      label(`${pts.length.toLocaleString('en-US')} springs · struck on every beat anikit found in the music (${beat.bpm.toFixed(1)} bpm)`, 86, H - 48, C.paper);
    },
  });

  // 06 · Ease — curves plotted live as the pens move.
  const sets = [
    ['outExpo', 'spring(bounce: 0.55)', 'outBounce', 'elastic.out(1, 0.3)', 'steps(8)', 'rough(16, 1.4, 7, outQuad)'],
    ['inOutBack', 'custom(M0,0 C0.2,1.4 0.5,-0.4 1,1)', 'anticipate', 'slowmo(0.6, 0.8)', 'blend(inQuad, outBounce, 0.5)', 'mirror(inOutSine)'],
  ];
  movement('Ease', 34, 40, C.paper, C.ink, {
    enter() {},
    draw(f) {
      fill(C.paper);
      big('Ease', 74, 330, 330, C.ink);
      label('css · penner · gsap · springs · svg curves · functions', 86, 384, C.ink);
      const lt = f.local, set = sets[lt < 3 ? 0 : 1], lt2 = lt < 3 ? lt : lt - 3;
      const x0 = 860, x1 = W - 90, top = 110, lane = 155;
      for (let k = 0; k < set.length; k++) {
        const yb = top + (k + 1) * lane, amp = 95, pr = clamp((lt2 - 0.15 - k * 0.08) / 1.7);
        ctx.strokeStyle = 'rgba(12,12,12,0.16)'; ctx.lineWidth = 1;
        ctx.beginPath(); ctx.moveTo(x0, yb); ctx.lineTo(x1, yb); ctx.moveTo(x0, yb - amp); ctx.lineTo(x1, yb - amp); ctx.stroke();
        ctx.strokeStyle = C.red; ctx.lineWidth = 4.5; ctx.lineJoin = 'round';
        ctx.beginPath();
        const steps = Math.floor(pr * 180);
        for (let j = 0; j <= steps; j++) { const u = j / 180, x = x0 + (x1 - x0) * u, y = yb - ease(set[k], u) * amp; if (j === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y); }
        ctx.stroke();
        ctx.fillStyle = C.ink;
        ctx.beginPath(); ctx.arc(x0 + (x1 - x0) * pr, yb - ease(set[k], pr) * amp, 10, 0, TAU); ctx.fill();
        label(set[k], x0 - 26, yb - amp / 2 + 6, C.ink, 'right', 16);
      }
    },
  });

  // 07 · Tunnel — 1,400 cards, projected and depth-sorted in WebAssembly; red flashes on the beat.
  let tunnel = null;
  movement('Tunnel', 40, 46, C.lime, C.ink, {
    enter() {
      const n = 1400, xyz = new Float32Array(n * 3), meta = [];
      for (let i = 0; i < n; i++) { const a = i * 0.29, r = 380 + (i % 4) * 70; xyz[i * 3] = Math.cos(a) * r; xyz[i * 3 + 1] = Math.sin(a) * r; xyz[i * 3 + 2] = -i * 7.2; meta.push({ a, red: i % 9 === 0 }); }
      tunnel = { n, xyz, meta };
      camReset(1400);
      animate(cam.camera, { z: [1400, -8600] }, { duration: 6, ease: 'inOutCubic', delay: 0.05 });
      animate(cam.target, { z: [400, -9600] }, { duration: 6, ease: 'inOutCubic', delay: 0.05 });
      animate(cam.rotation, { z: [0, 150] }, { duration: 6, ease: 'inOutCubic', delay: 0.05 });
    },
    draw(f) {
      fill(C.lime);
      const p = cam.project(tunnel.xyz), roll = (cam.rotation.z * Math.PI) / 180;
      const { phase } = beat.beatAt(f.t), flash = Math.exp(-phase * 6);
      for (let k = 0; k < p.count; k++) {
        const i = p.order[k], sc = p.data[i * 4 + 3];
        if (sc <= 0) continue;
        const d = p.data[i * 4 + 2];
        if (d < 30) continue;
        const w = 64 * sc, h = 90 * sc;
        if (w < 0.6 || w > 2600) continue;
        ctx.save();
        ctx.globalAlpha = clamp(1.15 - d / 6500);
        ctx.translate(p.data[i * 4], p.data[i * 4 + 1]);
        ctx.rotate(tunnel.meta[i].a + roll);
        ctx.fillStyle = tunnel.meta[i].red || (i % 5 === 0 && flash > 0.5) ? C.red : C.ink;
        ctx.fillRect(-w / 2, -h / 2, w, h);
        ctx.restore();
      }
      big('1,400', 80, H - 118, 190, C.ink);
      label('cards · projected + radix depth-sorted in webassembly · flashing on the beat', 86, H - 60, C.ink);
    },
  });

  // 08 · Galaxy — 160,000 stars breathing with the music; the camera falls into the core.
  movement('Galaxy', 46, 54, C.black, C.paper, {
    enter() {
      g.options.mode = 'light'; g.options.background = C.black; g.options.trail = 0.55;
      clearGlow();
      camReset(2600); Object.assign(cam.camera, { y: 1900, z: 2600 });
      cloud.snap(S.origin); cloud.explode(0, 0, 0, 1500);
      cloud.from = S.origin; cloud.to = S.galaxy; cloud.mix = 0;
      animate(cloud, { mix: 1 }, { duration: 1.6, ease: 'outCubic' });
      animate(cam.camera, { y: [1900, 60], z: [2600, 260] }, { duration: 7.4, ease: 'inOutCubic', delay: 0.2 });
      animate(cam.rotation, { y: [0, 100] }, { duration: 8, ease: 'inOutSine' });
      for (const b of beatsIn(46.5, 53)) delayedCall(b - 46, () => cloud.explode(0, 0, 0, 220));
    },
    step: () => cloud.step(1 / 60),
    draw(f) {
      particles({ tint: [0.82, 0.88, 1], intensity: 0.34, maxRadius: 7, exposure: 0.85 + 0.5 * beat.at(f.t) });
      if (f.local > 7.2) { ctx.fillStyle = `rgba(236,230,218,${io(prog(f.local, 7.2, 8))})`; ctx.fillRect(0, 0, W, H); }
      fx('One engine.', 86, 190, 82, C.paper, 46.8, f.t);
      label(`160,000 stars · exposure follows the soundtrack's loudness`, 86, H - 48, C.paper);
    },
  });

  // 09 · Signature
  movement('anikit', 54, 60, C.paper, C.ink, {
    enter() {
      g.options.mode = 'ink'; g.options.background = C.paper; g.options.trail = 0.5;
      camReset(1000);
      cloud.snap(S.origin); cloud.explode(0, 0, 0, 1900);
      clearGlow();
      cloud.from = S.origin; cloud.to = S.wordmark; cloud.mix = 0;
      animate(cloud, { mix: 1 }, { duration: 1.7, ease: 'outCubic' });
      animate(cam.rotation, { y: [-14, 0] }, { duration: 3, ease: 'outCubic' });
      sparks.clear(); sparks.set({ colors: ['#ff3b14', '#ff3b14', '#0c0c0c', '#ece6da'], origin: [0, 110, 0], gravity: [0, -700, 0], speed: 1600 }); sparks.burst(7000);
    },
    step: () => { cloud.step(1 / 60); sparks.step(1 / 60); },
    draw(f) {
      particles({ intensity: 1.9 });
      drawSparks(false, 2.4);
      fx('Motion,', 80, H - 200, 128, C.ink, 55.0, f.t);
      fx('compiled.', 80, H - 80, 128, C.red, 55.2, f.t, { italic: true });
      if (f.local > 2) {
        ctx.globalAlpha = clamp((f.local - 2) / 0.6);
        label('npm install anikit', W - 86, H - 152, C.ink, 'right', 22);
        label('pip install anikit', W - 86, H - 118, C.ink, 'right', 22);
        label('rust · webassembly · typescript · python', W - 86, H - 80, C.ink, 'right');
        ctx.globalAlpha = 1;
      }
    },
  });

  // Wipes between colour fields, and a slate.
  const WIPE = 0.34;
  function chrome(f, name) {
    const i = movements.findIndex((m) => m.name === name), m = movements[i], next = movements[i + 1];
    if (next && f.t > next.start - WIPE) {
      const u = io(prog(f.t, next.start - WIPE, next.start));
      ctx.fillStyle = next.bg; ctx.fillRect(0, 0, W * u, H);
      ctx.fillStyle = next.bg === C.red || m.bg === C.red ? C.ink : C.red; ctx.fillRect(W * u - 16, 0, 16, H);
    }
    if (i > 0 && f.t < m.start + 0.2) { const u = io(prog(f.t, m.start, m.start + 0.2)); ctx.fillStyle = m.bg; ctx.fillRect(W * u, 0, W * (1 - u), H); }
    big('anikit', 80, 84, 34, m.fg);
    label(`${String(i + 1).padStart(2, '0')} / ${String(movements.length).padStart(2, '0')} — ${name}`, W - 86, 76, m.fg, 'right');
    label(`00:00:${String(Math.floor(f.t)).padStart(2, '0')}:${String(f.frame % 60).padStart(2, '0')}`, W - 86, 102, m.fg, 'right', 13);
  }

  return film;
}
