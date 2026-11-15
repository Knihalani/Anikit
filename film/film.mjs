// ANIKIT — a 20-second title film, composed in code and rendered by anikit.
//
//   anikit-render film.mjs --out anikit-film.mp4 --workers 3
//
// One grammar throughout: hard cuts on impacts (flash frame + camera shake),
// whips that carry the outgoing camera's motion, and shape match cuts where
// the particle cloud flows straight into the next form. Every pixel is
// anikit — particles simulated and lit in WebAssembly, rigid bodies, 3D
// projection and depth sorting, tweens on every camera, slammed type,
// emitters, and the transitions themselves — with motion blur.
export default async function film({ anikit: A, node }) {
  const { animate, pointCloud, glow, scene3d, textPoints, world, ease, delayedCall, emitter, typeFx, composition } = A;
  const W = 1920, H = 1080;
  const FONT = '/usr/share/fonts';
  node.registerFont(`${FONT}/opentype/inter/InterDisplay-Black.otf`, 'Display');
  node.registerFont(`${FONT}/opentype/inter/InterDisplay-BlackItalic.otf`, 'DisplayItalic');
  node.registerFont(`${FONT}/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf`, 'Mono');

  const C = { paper: '#ece6da', ink: '#0c0c0c', red: '#ff3b14', blue: '#1f2bff', lime: '#d2ff3c', black: '#050505' };
  const TAU = Math.PI * 2;
  const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));

  // ---- Cut list (seconds): every cut lands on a 130 BPM half-beat grid -------------
  const T = { open: 0, knot: 2.31, sand: 4.38, spring: 6.69, ease: 8.54, depth: 10.15, galaxy: 12.46, sparks: 14.77, sign: 16.62, end: 19.8 };

  // ---- Impacts (shake) and flash frames ---------------------------------------------
  const impacts = [], flashes = [];
  const hit = (t, amp = 26, flashColor = '#fff', k = 34) => { impacts.push({ t, amp }); if (flashColor) flashes.push({ t, color: flashColor, k }); };
  const shake = (t) => {
    let x = 0, y = 0;
    impacts.forEach((s, i) => {
      if (t < s.t) return;
      const a = s.amp * Math.exp(-(t - s.t) * 10);
      x += a * Math.sin(t * 97 + i * 7.1); y += a * Math.cos(t * 83 + i * 3.3);
    });
    return [x, y];
  };
  const frame = (ctx, t, bg, content) => {
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.fillStyle = bg; ctx.fillRect(0, 0, W, H);
    const [sx, sy] = shake(t);
    ctx.save();
    if (sx || sy) { ctx.translate(W / 2 + sx, H / 2 + sy); ctx.scale(1.035, 1.035); ctx.translate(-W / 2, -H / 2); }
    content();
    ctx.restore();
  };
  const flashOver = (ctx, t) => {
    for (const f of flashes) {
      if (t < f.t) continue;
      const a = 0.85 * Math.exp(-(t - f.t) * f.k);
      if (a < 0.01) continue;
      ctx.globalAlpha = a; ctx.fillStyle = f.color; ctx.fillRect(0, 0, W, H); ctx.globalAlpha = 1;
    }
  };
  const slam = (ctx, s, x, y, size, color, start, t, o = {}) => typeFx(ctx, s, x, y, { size, color, start, t, effect: 'slam', font: o.italic ? 'DisplayItalic' : 'Display', align: o.align ?? 'center', ...o });
  const caption = (ctx, s, color, align = 'left') => {
    ctx.font = '400 16px Mono'; ctx.textAlign = align; ctx.textBaseline = 'alphabetic';
    ctx.globalAlpha = 0.8; ctx.fillStyle = color; ctx.fillText(s.toUpperCase(), align === 'left' ? 86 : W - 86, H - 58); ctx.globalAlpha = 1;
  };
  const steppers = {};
  const stepDt = (key, t) => { const d = Math.max(0, Math.min(0.05, t - (steppers[key] ?? t))); steppers[key] = t; return d; };

  // ---- One particle cloud, one glow ------------------------------------------------------
  const target = node.glowTarget(W, H);
  const g = glow(target, { resolution: 0.9, background: C.paper, mode: 'ink', trail: 0.45, budget: 0 });
  g.resize(W, H);
  const N = 160_000;
  let seed = 11;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const gauss = () => (rnd() + rnd() + rnd() - 1.5) * 1.15;
  const make = (fn) => { const a = new Float32Array(N * 3); for (let i = 0; i < N; i++) fn(i, a, i * 3); return a; };
  const fib = (i, n) => { const y = 1 - ((i + 0.5) / n) * 2, r = Math.sqrt(1 - y * y), t = i * 2.399963; return [Math.cos(t) * r, y, Math.sin(t) * r]; };
  const text = (s, width, dy = 0, depth = 12) => { const t = textPoints(s, N, { font: 'Display', weight: 900, width, depth }); for (let i = 1; i < t.length; i += 3) t[i] += dy; return t; };
  const shapes = {
    // A storm of ink around and in front of the camera: the opening frame.
    storm: make((i, a, o) => { const c = fib(i, N), r = 250 + Math.pow(rnd(), 0.6) * 1500; a[o] = c[0] * r * 1.6; a[o + 1] = c[1] * r; a[o + 2] = c[2] * r + 300; }),
    title: text('anikit', 1000, 90, 200),
    knot: make((i, a, o) => {
      const t = (i / N) * TAU, p = 2, q = 5, R = 300, rr = 130, r = R + rr * Math.cos(q * t), tube = 34 * Math.sqrt(rnd()), ang = rnd() * TAU;
      a[o] = r * Math.cos(p * t) + tube * Math.cos(ang); a[o + 1] = r * Math.sin(p * t) + tube * Math.sin(ang) - 40; a[o + 2] = rr * 1.8 * Math.sin(q * t) + tube * Math.cos(ang * 1.7);
    }),
    galaxy: make((i, a, o) => {
      if (i % 9 === 0) { const c = fib(i, N), r = 90 * Math.pow(rnd(), 1.5); a[o] = c[0] * r; a[o + 1] = c[1] * r * 0.6; a[o + 2] = c[2] * r; return; }
      const arm = i % 4, d = Math.pow(rnd(), 0.7), r = 40 + d * 1250, ang = (arm / 4) * TAU + d * 5.2 + gauss() * 0.22 * (1.1 - d);
      a[o] = Math.cos(ang) * r + gauss() * 22; a[o + 1] = gauss() * (26 * (1 - d) + 5); a[o + 2] = Math.sin(ang) * r + gauss() * 22;
    }),
    wordmark: text('anikit', 1100, 150, 30),
  };
  const names = Object.keys(shapes);
  const S = Object.fromEntries(names.map((n, i) => [n, i]));
  const cloud = pointCloud(N, { autoStep: false, shapes: names.map((n) => shapes[n]), stiffness: 34, damping: 7.5, spread: 0.6, noise: 1.6, swirl: 60, frequency: 0.012 });
  cloud.snap(S.storm);
  g.colors(N, () => { const r = rnd(); const k = r < 0.05 ? 1.7 : 0.5 + rnd() * 0.5; return [k, k, k]; });
  const flow = (to, dur, e = 'inOutCubic') => { cloud.from = cloud.mix >= 0.5 ? cloud.to : cloud.from; cloud.to = S[to]; cloud.mix = 0; animate(cloud, { mix: 1 }, { duration: dur, ease: e }); };
  let look = '';
  const particles = (ctx, cam, mode, bg, o = {}) => {
    if (look !== mode + bg) { look = mode + bg; g.options.mode = mode; const k = g.options.trail; g.options.trail = 0; g.render(cam, cloud, { exposure: 0 }); g.options.trail = k; }
    g.options.background = bg;
    const ink = mode === 'ink';
    g.render(cam, cloud, { size: ink ? 2.1 : 1.3, intensity: o.intensity ?? (ink ? 1.8 : 0.5), fog: [300, 4800], tint: o.tint, maxRadius: 8 });
    ctx.drawImage(target.canvas, 0, 0, W, H);
  };
  const cloudStep = ({ t }) => cloud.step(stepDt('cloud', t));

  // Sparks (emitters), drawn as light or ink.
  const drawSparks = (ctx, cam, em, light, base = 2.4) => {
    if (!em.count) return;
    em.shade();
    const p = cam.project(em, { sort: false }), col = em.colors, sz = em.sizes;
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

  const film = composition({ width: W, height: H, fps: 60, duration: T.end, motionBlur: { samples: 3, shutter: 0.5 } });

  // 01 · OPEN (0–2.31): a storm of ink rushes in and slams into a 3D
  //      wordmark while the camera whips around it.
  const camA = scene3d({ fov: 42, camera: { z: 1000 }, width: W, height: H, near: 4, far: 30000 });
  hit(0.35, 34, '#fff', 26);
  film.scene('open', T.open, T.knot, {
    enter() {
      // Frame 0: the 3D wordmark, the camera already whipping around it.
      // At 0.35 s it detonates toward the lens; its springs pull it back.
      cloud.snap(S.title);
      cloud.from = cloud.to = S.title; cloud.mix = 0;
      delayedCall(0.35, () => cloud.explode(0, 90, -500, 3400));
      animate(camA.rotation, { y: [-80, 20], x: [30, 6] }, { duration: 1.6, ease: 'outExpo' });
      animate(camA.camera, { z: [760, 1080] }, { duration: 2.0, ease: 'outExpo' });
    },
    step: cloudStep,
    draw({ ctx, t }) {
      frame(ctx, t, C.paper, () => particles(ctx, camA, 'ink', C.paper, { intensity: 1.9 }));
      slam(ctx, 'Motion,', 90, H - 214, 140, C.ink, 1.05, t, { align: 'left' });
      slam(ctx, 'compiled.', 90, H - 80, 140, C.red, 1.2, t, { align: 'left', italic: true });
      flashOver(ctx, t);
    },
  });

  // 02 · KNOT (2.31–4.38): hard cut to vermilion; the wordmark flows into a
  //      torus knot while the camera spins around it.
  const camB = scene3d({ fov: 42, camera: { z: 1000 }, width: W, height: H, near: 4, far: 30000 });
  hit(T.knot, 22, C.red, 40);
  film.scene('knot', T.knot, T.sand, {
    enter() {
      flow('knot', 0.75, 'outExpo');
      animate(camB.rotation, { y: [0, 400], x: [10, 38, 18] }, { duration: 2.3, ease: 'inQuad' });
      animate(camB.camera, { z: [1150, 820] }, { duration: 2.1, ease: 'inOutSine' });
    },
    step: cloudStep,
    draw({ ctx, t }) {
      frame(ctx, t, C.red, () => particles(ctx, camB, 'ink', C.red, { intensity: 1.85 }));
      slam(ctx, '160,000 PARTICLES.', W / 2, 190, 150, C.paper, T.knot + 0.3, t);
      caption(ctx, 'simulated + lit in webassembly · one spring each', C.paper);
      flashOver(ctx, t);
    },
  });

  // 03 · SAND (4.38–6.69): whip in; a word of 9,000 rigid bodies drops,
  //      then time runs backwards.
  let sand = null;
  hit(T.sand + 0.7, 26, null); hit(T.sand + 1.5, 12, C.red, 40);
  film.scene('sand', T.sand, T.spring, {
    transition: { duration: 0.3, type: 'whip', direction: 'left' },
    enter() {
      const off = node.createCanvas(W, H), o = off.getContext('2d');
      o.font = '900 430px Display'; o.textAlign = 'center'; o.textBaseline = 'middle'; o.fillStyle = '#fff';
      o.fillText('MOTION', W / 2, H / 2 + 20);
      const img = o.getImageData(0, 0, W, H).data, pts = [], sp = 5.2;
      for (let y = 0; y < H; y += sp) for (let x = 0; x < W; x += sp) if (img[(Math.round(y) * W + Math.round(x)) * 4 + 3] > 140) pts.push({ x: x + (rnd() - 0.5) * 0.6, y: y + (rnd() - 0.5) * 0.6, r: 2.45, mass: 1 });
      const w = world({ bounds: [0, -4000, W, H - 40], gravity: [0, 0], substeps: 3, iterations: 2, restitution: 0.08, friction: 0.35, damping: 0.03 });
      w.addMany(pts);
      sand = { w, n: pts.length, hist: [], mode: 'hold', k: 0, last: -1 };
      delayedCall(0.4, () => { sand.mode = 'fall'; w.set({ gravity: [0, 3600] }); for (let i = 0; i < sand.n; i++) w.setVelocity(i, (rnd() - 0.5) * 300, -rnd() * 300); });
      delayedCall(1.5, () => { sand.mode = 'rewind'; w.pause(); sand.k = sand.hist.length - 1; animate(sand, { k: 0 }, { duration: 0.6, ease: 'inOutExpo', onComplete: () => (sand.mode = 'done') }); });
    },
    step({ frame: f }) {
      const s = sand;
      if (f === s.last) return;
      s.last = f;
      if (s.mode === 'fall' || (s.mode === 'hold' && !s.hist.length)) {
        const d = s.w.data, snap = new Float32Array(s.n * 2);
        for (let i = 0; i < s.n; i++) { snap[i * 2] = d[i * 3]; snap[i * 2 + 1] = d[i * 3 + 1]; }
        s.hist.push(snap);
      }
    },
    draw({ ctx, t }) {
      const s = sand, fr = s.mode === 'rewind' || s.mode === 'done' ? s.hist[Math.round(s.k)] : s.hist[s.hist.length - 1];
      frame(ctx, t, C.lime, () => {
        ctx.fillStyle = C.ink;
        for (let i = 0; i < s.n; i++) ctx.fillRect(fr[i * 2] - 2.25, fr[i * 2 + 1] - 2.25, 4.5, 4.5);
      });
      if (s.mode === 'rewind') slam(ctx, '◀◀ REWIND', W / 2, 200, 120, C.red, T.sand + 1.5, t, { duration: 0.2 });
      if (s.mode === 'done') slam(ctx, 'DETERMINISTIC.', W / 2, 200, 150, C.ink, (s.at ??= t), t);
      caption(ctx, `${s.n.toLocaleString('en-US')} rigid bodies · one physics world`, C.ink);
      flashOver(ctx, t);
    },
  });

  // 04 · SPRINGS (6.69–8.54): hard cut; the camera swoops over a sheet of
  //      3,080 springs struck faster and faster.
  const camC = scene3d({ fov: 40, camera: { z: 1300 }, width: W, height: H, near: 4, far: 30000 });
  let sheet = null;
  hit(T.spring, 20, '#fff', 40);
  film.scene('springs', T.spring, T.ease, {
    enter() {
      const cols = 70, rows = 44, pts = [];
      for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) pts.push({ x: (c / (cols - 1) - 0.5) * 2300, z: (r / (rows - 1) - 0.5) * 1600, h: 0 });
      const spots = [[0, 0], [-640, 300], [560, -260], [-300, -420], [720, 380], [180, 120], [-560, -60], [400, 320], [0, -320], [-220, 400]];
      const hits = [];
      for (let k = 0, at = 0.05, gap = 0.3; at < 1.9; k++, at += gap, gap = Math.max(0.1, gap * 0.85)) hits.push({ x: spots[k % spots.length][0], z: spots[k % spots.length][1], at, amp: k === 0 ? 480 : 300 });
      sheet = { cols, rows, pts, xyz: new Float32Array(cols * rows * 3), hits: hits.map((h) => ({ ...h, d: pts.map((p) => Math.hypot(p.x - h.x, p.z - h.z)) })) };
      animate(camC.rotation, { x: [46, 26], y: [-40, 34] }, { duration: 2.3, ease: 'inOutCubic' });
      animate(camC.camera, { z: [1050, 760] }, { duration: 2.3, ease: 'inOutCubic' });
    },
    draw({ ctx, t }) {
      const lt = t - T.spring;
      frame(ctx, t, C.blue, () => {
        const { cols, rows, pts, xyz } = sheet;
        for (let i = 0; i < pts.length; i++) {
          let h = 0;
          for (const s of sheet.hits) {
            const tau = lt - s.at - s.d[i] / 1900;
            if (tau <= 0 || tau > 1.5) continue;
            h -= s.amp * Math.exp(-s.d[i] / 650) * ease('outCubic', Math.min(1, tau / 0.1)) * (1 - ease('spring(bounce: 0.62, duration: 1.3)', tau / 1.5));
          }
          pts[i].h = h;
          xyz[i * 3] = pts[i].x; xyz[i * 3 + 1] = -h; xyz[i * 3 + 2] = pts[i].z;
        }
        const p = camC.project(xyz, { sort: false }), X = (i) => p.data[i * 4], Y = (i) => p.data[i * 4 + 1];
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
          const r = (1.8 + lift * 3.6) * sc;
          ctx.fillRect(X(i) - r, Y(i) - r, r * 2, r * 2);
        }
      });
      slam(ctx, 'SPRINGS.', W / 2, 230, 230, C.paper, T.spring + 0.2, t);
      caption(ctx, '3,080 springs · retargeted mid-flight', C.paper);
      flashOver(ctx, t);
    },
  });

  // 05 · EASE (8.54–10.15): whip (carrying the swoop); curves drawn at once.
  const curves = ['outExpo', 'spring(bounce: 0.55)', 'outBounce', 'elastic.out(1, 0.3)', 'steps(8)', 'custom(M0,0 C0.2,1.4 0.5,-0.4 1,1)'];
  film.scene('ease', T.ease, T.depth, {
    transition: { duration: 0.3, type: 'whip', direction: 'left' },
    draw({ ctx, t }) {
      const lt = t - T.ease;
      frame(ctx, t, C.paper, () => {
        const x0 = 860, x1 = W - 90, top = 60, lane = 160;
        curves.forEach((name, k) => {
          const yb = top + (k + 1) * lane, amp = 110, pr = clamp((lt - 0.12 - k * 0.03) / 0.75);
          ctx.strokeStyle = 'rgba(12,12,12,0.14)'; ctx.lineWidth = 1;
          ctx.beginPath(); ctx.moveTo(x0, yb); ctx.lineTo(x1, yb); ctx.stroke();
          ctx.strokeStyle = C.red; ctx.lineWidth = 6; ctx.lineJoin = 'round';
          ctx.beginPath();
          const steps = Math.floor(pr * 200);
          for (let j = 0; j <= steps; j++) { const u = j / 200, x = x0 + (x1 - x0) * u, y = yb - ease(name, u) * amp; if (j === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y); }
          ctx.stroke();
          ctx.fillStyle = C.ink;
          ctx.beginPath(); ctx.arc(x0 + (x1 - x0) * pr, yb - ease(name, pr) * amp, 12, 0, TAU); ctx.fill();
          ctx.font = '400 16px Mono'; ctx.textAlign = 'right'; ctx.fillText(name.toUpperCase(), x0 - 26, yb - 6);
        });
      });
      slam(ctx, 'EASE.', 80, 640, 250, C.ink, T.ease + 0.15, t, { align: 'left' });
      flashOver(ctx, t);
    },
  });

  // 06 · DEPTH (10.15–12.46): hard cut; a flight through 1,400 cards.
  const camD = scene3d({ fov: 52, camera: { z: 1400 }, width: W, height: H, near: 4, far: 30000 });
  let tunnel = null;
  hit(T.depth, 22, '#fff', 40);
  film.scene('depth', T.depth, T.galaxy, {
    enter() {
      const n = 1400, xyz = new Float32Array(n * 3), meta = [];
      for (let i = 0; i < n; i++) { const a = i * 0.29, r = 360 + (i % 4) * 75; xyz[i * 3] = Math.cos(a) * r; xyz[i * 3 + 1] = Math.sin(a) * r; xyz[i * 3 + 2] = -i * 7.2; meta.push({ a, hot: i % 7 === 0 }); }
      tunnel = { xyz, meta };
      animate(camD.camera, { z: [900, -9300] }, { duration: 2.5, ease: 'inQuad' });
      animate(camD.target, { z: [-100, -10300] }, { duration: 2.5, ease: 'inQuad' });
      animate(camD.rotation, { z: [0, 240] }, { duration: 2.5, ease: 'inQuad' });
    },
    draw({ ctx, t }) {
      frame(ctx, t, C.red, () => {
        const p = camD.project(tunnel.xyz), roll = (camD.rotation.z * Math.PI) / 180;
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
          ctx.fillStyle = tunnel.meta[i].hot ? C.paper : C.ink;
          ctx.fillRect(-w / 2, -h / 2, w, h);
          ctx.restore();
        }
      });
      // Clear space for the type: a solid ink band.
      const a = clamp((t - T.depth - 0.25) / 0.12);
      if (a > 0) { ctx.fillStyle = C.ink; ctx.fillRect(0, H / 2 - 150 * a, W, 300 * a); }
      slam(ctx, 'DEPTH-SORTED.', W / 2, H / 2 + 70, 200, C.red, T.depth + 0.32, t);
      caption(ctx, '1,400 cards · projected + radix-sorted in webassembly, every frame', C.paper);
      flashOver(ctx, t);
    },
  });

  // 07 · GALAXY (12.46–14.77): hard cut to black; the knot flows into a
  //      galaxy as the camera falls into its core.
  const camE = scene3d({ fov: 40, camera: { y: 1900, z: 2600 }, width: W, height: H, near: 4, far: 30000 });
  hit(T.galaxy, 18, '#fff', 30);
  film.scene('galaxy', T.galaxy, T.sparks, {
    enter() {
      flow('galaxy', 0.9, 'outExpo');
      animate(camE.camera, { y: [1100, 260], z: [1500, 700] }, { duration: 2.4, ease: 'inOutCubic' });
      animate(camE.rotation, { y: [0, 110] }, { duration: 2.4, ease: 'inOutSine' });
    },
    step: cloudStep,
    draw({ ctx, t }) {
      frame(ctx, t, C.black, () => particles(ctx, camE, 'light', C.black, { tint: [0.86, 0.9, 1], intensity: 0.55 }));
      slam(ctx, 'ONE ENGINE.', W / 2, 220, 200, C.paper, T.galaxy + 0.4, t);
      caption(ctx, 'rust → webassembly · typescript · python', C.paper);
      flashOver(ctx, t);
    },
  });

  // 08 · SPARKS (14.77–16.62): hard cut; 12,000 sparks burst at the camera.
  const camF = scene3d({ fov: 46, camera: { z: 900 }, width: W, height: H, near: 4, far: 30000 });
  const fire = emitter({ capacity: 22000, rate: 0, autoStep: false, life: 1.7, lifeVariation: 0.5, speed: 1500, speedVariation: 0.7, spread: Math.PI, gravity: [0, -650, 0], drag: 1.4, turbulence: 1200, frequency: 0.006, colors: ['#ffffff', '#ffd9a0', '#ff6a2a', '#300a00'] });
  hit(T.sparks, 34, '#fff', 22);
  hit(T.sparks + 0.95, 26, null);
  film.scene('sparks', T.sparks, T.sign, {
    enter() {
      fire.burst(12000);
      delayedCall(0.95, () => fire.burst(10000));
      animate(camF.rotation, { y: [-30, 70], x: [12, -8] }, { duration: 1.9, ease: 'inOutCubic' });
    },
    step({ t }) { fire.step(stepDt('fire', t)); },
    draw({ ctx, t }) {
      frame(ctx, t, C.black, () => drawSparks(ctx, camF, fire, true, 2.6));
      slam(ctx, 'EMITTERS.', W / 2, H - 120, 190, C.paper, T.sparks + 0.25, t);
      flashOver(ctx, t);
    },
  });

  // 09 · SIGNATURE (16.62–19.8): revealed through an 'a'; the galaxy flows
  //      into the wordmark.
  const camG = scene3d({ fov: 42, camera: { z: 1000 }, width: W, height: H, near: 4, far: 30000 });
  const inkSpray = emitter({ capacity: 10000, rate: 0, autoStep: false, life: 1.2, lifeVariation: 0.4, speed: 1700, speedVariation: 0.6, spread: Math.PI, gravity: [0, -800, 0], drag: 1.6, turbulence: 900, frequency: 0.008, origin: [0, 150, 0], colors: ['#ff3b14', '#ff3b14', '#0c0c0c', '#ece6da'] });
  hit(T.sign + 0.55, 24, null);
  film.scene('signature', T.sign, T.end, {
    transition: { duration: 0.55, type: 'mask', text: 'a', origin: [0.5, 0.56] },
    enter() {
      flow('wordmark', 1.0, 'outExpo');
      animate(camG.rotation, { y: [-24, 0], x: [10, 0] }, { duration: 1.6, ease: 'outExpo' });
      delayedCall(0.55, () => inkSpray.burst(9000));
    },
    step(f) { cloudStep(f); inkSpray.step(stepDt('ink', f.t)); },
    draw({ ctx, t }) {
      frame(ctx, t, C.paper, () => { particles(ctx, camG, 'ink', C.paper, { intensity: 1.95 }); drawSparks(ctx, camG, inkSpray, false, 2.6); });
      slam(ctx, 'Motion,', 90, H - 214, 140, C.ink, T.sign + 1.0, t, { align: 'left' });
      slam(ctx, 'compiled.', 90, H - 80, 140, C.red, T.sign + 1.15, t, { align: 'left', italic: true });
      const a = clamp((t - T.sign - 1.7) / 0.35);
      if (a > 0) {
        ctx.globalAlpha = a; ctx.textAlign = 'right'; ctx.fillStyle = C.ink;
        ctx.font = '400 30px Mono'; ctx.fillText('NPM INSTALL ANIKIT', W - 86, H - 124);
        ctx.font = '400 16px Mono'; ctx.fillText('RUST · WEBASSEMBLY · TYPESCRIPT · PYTHON', W - 86, H - 84);
        ctx.globalAlpha = 1;
      }
      flashOver(ctx, t);
    },
  });

  return film;
}
