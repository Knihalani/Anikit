// ANIKIT — ONE TAKE. A 24-second film in a single unbroken camera move,
// composed in code and rendered entirely by anikit (no browser, no WebGL).
//
//   anikit-render film.mjs --out anikit-film.mp4 --workers 3
//
// A pool of 3,000 solid, lit cubes (anikit's WebAssembly rasterizer) morphs
// between formations — a voxel wordmark, a sea of springs, rings around a
// glossy torus knot, a tunnel — while 120,000 particles (pointCloud + glow)
// detonate into a galaxy. One camera flies through all of it on a Hermite
// path. Impacts, a slow-motion speed ramp, motion blur and the film grade
// (bloom, aberration, vignette, grain) come from anikit's composition.
export default async function film({ anikit: A, node }) {
  const { pointCloud, glow, scene3d, ease, typeFx, composition, mesh, instances, renderer3d } = A;
  const W = 1920, H = 1080;
  const FONT = '/usr/share/fonts';
  node.registerFont(`${FONT}/opentype/inter/InterDisplay-Black.otf`, 'Display');
  node.registerFont(`${FONT}/opentype/inter/InterDisplay-BlackItalic.otf`, 'DisplayItalic');
  node.registerFont(`${FONT}/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf`, 'Mono');

  const PAPER = [0.93, 0.9, 0.85], RED = [1, 0.23, 0.08], INK = [0.12, 0.12, 0.13], BLUE = [0.12, 0.17, 1];
  const TAU = Math.PI * 2;
  const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
  const mix = (a, b, t) => a + (b - a) * t;
  const mix3 = (a, b, t) => [mix(a[0], b[0], t), mix(a[1], b[1], t), mix(a[2], b[2], t)];
  let seed = 5;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);

  // ---- Voxel wordmark: 'anikit' sampled on a grid, three layers deep ----------------
  const vox = [];
  {
    const cw = 118, chh = 34, cv = node.createCanvas(cw, chh), c = cv.getContext('2d');
    c.font = '900 33px Display'; c.textAlign = 'center'; c.textBaseline = 'middle'; c.fillStyle = '#fff';
    c.fillText('anikit', cw / 2, chh / 2 + 1);
    const img = c.getImageData(0, 0, cw, chh).data, cell = 15;
    for (let y = 0; y < chh; y++) for (let x = 0; x < cw; x++) if (img[(y * cw + x) * 4 + 3] > 120)
      for (let d = 0; d < 3; d++) vox.push([(x - cw / 2) * cell, -(y - chh / 2) * cell + 40, (d - 1) * cell, x / cw]);
  }
  const K = vox.length; // ≈ 3,000 cubes in the pool
  const order = vox.map((v, i) => [Math.hypot(v[0], v[1]), i]).sort((a, b) => a[0] - b[0]);
  const rank = new Float32Array(K); order.forEach(([, i], r) => (rank[i] = r / K));
  const jitter = Float32Array.from({ length: K }, () => rnd());

  // ---- Formations: each returns [x, y, z, rx, ry, rz, sx, sy, sz, r, g, b] --------------
  const strikes = [5.0, 5.55, 6.05, 6.5, 6.85, 7.2, 7.5];
  const spots = [[0, 0], [-700, 300], [650, -250], [-300, -420], [760, 380], [150, 100], [-560, -60]];
  const COLS = 60, ROWS = Math.ceil(K / 60), GAP = 58;
  const F = {
    vortex: (i) => { const a = jitter[i] * TAU * 3, r = 1400 + jitter[(i * 7) % K] * 1800; return [Math.cos(a) * r, (jitter[(i * 13) % K] - 0.5) * 1600, Math.sin(a) * r - 600, i * 37, i * 53, 0, 14, 14, 14, ...PAPER]; },
    word: (i) => { const v = vox[i], c = v[3] > 0.82 ? RED : mix3(PAPER, [1, 0.62, 0.45], clamp((v[3] - 0.55) * 1.6)); return [v[0], v[1], v[2], 0, 0, 0, 13.6, 13.6, 13.6, ...c]; },
    wave: (i, t) => {
      const c = i % COLS, r = Math.floor(i / COLS), x = (c - COLS / 2) * GAP, z = (r - ROWS / 2) * GAP - 200;
      let h = 0;
      strikes.forEach((s, k) => {
        const [sx, sz] = spots[k % spots.length], d = Math.hypot(x - sx, z - sz), tau = t - s - d / 1700;
        if (tau > 0 && tau < 1.6) h += 330 * Math.exp(-d / 700) * ease('outCubic', Math.min(1, tau / 0.12)) * (1 - ease('spring(bounce: 0.62, duration: 1.4)', tau / 1.6));
      });
      const lift = clamp(h / 220);
      return [x, -420 + h, z, 0, 0, 0, 48, 48 + h * 0.15, 48, ...mix3(INK.map((v) => v * 2.2), lift > 0.55 ? RED : PAPER, clamp(lift * 1.4))];
    },
    orbit: (i, t) => {
      const ring = i % 6, a = (i / K) * TAU * 6 + t * (0.5 + ring * 0.12) * (ring % 2 ? 1 : -1), R = 560 + ring * 95, tilt = (ring - 2.5) * 0.32;
      const x = Math.cos(a) * R, y0 = Math.sin(a) * R * 0.2, z0 = Math.sin(a) * R;
      return [x, y0 * Math.cos(tilt) - z0 * Math.sin(tilt), y0 * Math.sin(tilt) + z0 * Math.cos(tilt), t * 90 + i, t * 70, 0, 16, 16, 16, ...(i % 11 === 0 ? RED : PAPER)];
    },
    belt: (i, t) => { const a = (i / K) * TAU + t * 0.25, R = 1500 + (jitter[i] - 0.5) * 300; return [Math.cos(a) * R, (jitter[(i * 3) % K] - 0.5) * 60, Math.sin(a) * R, i, i, 0, 9, 9, 9, ...(i % 9 === 0 ? RED : PAPER)]; },
    tunnel: (i) => {
      const ringN = 14, ring = Math.floor(i / ringN), j = i % ringN, a = (j / ringN) * TAU + ring * 0.16, R = 430;
      return [Math.cos(a) * R, Math.sin(a) * R, 2600 - ring * 78, 0, 0, (a * 180) / Math.PI + 90, 170, 22, 52, ...(ring % 9 === 0 ? RED : j % 7 === 0 ? [1, 0.75, 0.6] : PAPER)];
    },
  };
  // Morph schedule on the film clock: [start, duration, from, to, stagger].
  const morphs = [
    [0.0, 1.0, 'vortex', 'word', 0.35],
    [3.5, 1.3, 'word', 'wave', 0.5],
    [8.3, 1.3, 'wave', 'orbit', 0.45],
    [12.7, 1.1, 'orbit', 'belt', 0.4],
    [16.5, 1.1, 'belt', 'tunnel', 0.5],
    [20.4, 1.2, 'tunnel', 'word', 0.45],
  ];
  const cubes = instances(K);
  const place = (t) => {
    let m = morphs[0];
    for (const x of morphs) if (t >= x[0]) m = x;
    const [s, d, from, to, st] = m;
    for (let i = 0; i < K; i++) {
      const p = ease('inOutCubic', clamp((t - s - rank[i] * st * d) / (d * (1 - st))));
      const a = F[from](i, t), b = p >= 1 ? null : F[to](i, t), q = p >= 1 ? F[to](i, t) : null;
      const v = q ?? a.map((av, k) => mix(av, b[k], p));
      const arc = p > 0 && p < 1 ? Math.sin(p * Math.PI) * 220 * (jitter[i] - 0.3) : 0; // a little lift mid-flight
      cubes.set(i, { x: v[0], y: v[1] + arc, z: v[2], rx: v[3], ry: v[4], rz: v[5], sx: v[6], sy: v[7], sz: v[8], color: [v[9], v[10], v[11]] });
    }
  };

  // ---- The knot (solid) and the particles it becomes ------------------------------------
  const knot = mesh.torusKnot({ p: 2, q: 5, radius: 250, spread: 105, tube: 46, segments: 460, sides: 20 });
  const knotInst = instances(1);
  const cube = mesh.cube();
  const N = 120_000;
  const knotPts = new Float32Array(N * 3), galaxy = new Float32Array(N * 3), core = new Float32Array(N * 3);
  for (let i = 0; i < N; i++) {
    const t = (i / N) * TAU, k = 250 + 105 * Math.cos(5 * t), ang = rnd() * TAU, rr = 46 * Math.sqrt(rnd());
    knotPts.set([k * Math.cos(2 * t) + rr * Math.cos(ang), k * Math.sin(2 * t) + rr * Math.sin(ang), 105 * 1.6 * Math.sin(5 * t) + rr * Math.cos(ang * 1.3)], i * 3);
    const arm = i % 4, d = Math.pow(rnd(), 0.7), r = 60 + d * 1300, a = (arm / 4) * TAU + d * 5 + (rnd() - 0.5) * 0.5 * (1.1 - d);
    galaxy.set([Math.cos(a) * r, (rnd() - 0.5) * (50 * (1 - d) + 8), Math.sin(a) * r], i * 3);
    core.set([(rnd() - 0.5) * 6, (rnd() - 0.5) * 6, (rnd() - 0.5) * 6], i * 3);
  }
  const cloud = pointCloud(N, { autoStep: false, shapes: [knotPts, galaxy, core], stiffness: 30, damping: 6.5, spread: 0.5, noise: 1.4, swirl: 70 });
  cloud.snap(0);
  const gt = node.glowTarget(W, H);
  const g = glow(gt, { resolution: 0.85, background: '#000000', mode: 'light', trail: 0.55, budget: 0 });
  g.resize(W, H);
  g.colors(N, (i) => (i % 7 === 0 ? [1, 0.55, 0.32] : i % 13 === 0 ? [0.55, 0.62, 1] : [1, 0.92, 0.84]));

  // ---- One camera, one unbroken Hermite path --------------------------------------------
  // [clock, eye xyz, target xyz, roll°]
  const keys = [
    [0.0, [-1500, 260, 900], [0, 40, 0], 0],
    [1.1, [-420, 120, 1250], [0, 40, 0], 0],
    [3.4, [480, 60, 1300], [0, 40, 0], 0],
    [5.0, [900, 120, 1350], [0, -300, -200], 0],
    [7.6, [-700, 260, 1100], [0, -380, -400], 0],
    [9.4, [0, 420, 1700], [0, 0, 0], 0],
    [11.0, [900, 300, 700], [0, 0, 0], 0],
    [12.0, [180, 60, 160], [-200, 0, -600], 0],
    [12.9, [0, 300, 1300], [0, 0, 0], 0],
    [14.6, [0, 1500, 2600], [0, 0, 0], 0],
    [16.5, [0, 160, 420], [0, 0, -400], 0],
    [17.4, [0, 0, 3600], [0, 0, 2000], 0],
    [20.2, [0, 0, -800], [0, 0, -2600], 220],
    [21.6, [0, 120, 1500], [0, 40, 0], 360],
    [24.0, [-260, 80, 1150], [0, 40, 0], 360],
  ];
  const hermite = (t) => {
    let i = 0;
    while (i < keys.length - 2 && t > keys[i + 1][0]) i++;
    const k0 = keys[Math.max(0, i - 1)], k1 = keys[i], k2 = keys[i + 1], k3 = keys[Math.min(keys.length - 1, i + 2)];
    const u = clamp((t - k1[0]) / (k2[0] - k1[0]));
    const s = ease('inOutSine', u) * 0.35 + u * 0.65;
    const h00 = 2 * s ** 3 - 3 * s ** 2 + 1, h10 = s ** 3 - 2 * s ** 2 + s, h01 = -2 * s ** 3 + 3 * s ** 2, h11 = s ** 3 - s ** 2;
    const dt = k2[0] - k1[0];
    const tan = (a, b, ta, tb, j, f) => ((b[f][j] - a[f][j]) / Math.max(1e-3, tb - ta)) * dt;
    const at = (f, j) => h00 * k1[f][j] + h10 * tan(k0, k2, k0[0], k2[0], j, f) + h01 * k2[f][j] + h11 * tan(k1, k3, k1[0], k3[0], j, f);
    return { eye: [0, 1, 2].map((j) => at(1, j)), target: [0, 1, 2].map((j) => at(2, j)), roll: mix(k1[3], k2[3], ease('inOutCubic', u)) };
  };
  const cam = scene3d({ fov: 46, width: W, height: H, near: 8, far: 40000 });
  const r3 = renderer3d({ width: W, height: H });

  // ---- The film ----------------------------------------------------------------------------
  const film = composition({
    width: W, height: H, fps: 60, duration: 24.5, motionBlur: { samples: 2, shutter: 0.5 },
    post: { bloom: 0.55, bloomThreshold: 0.72, bloomRadius: 36, vignette: 0.42, grain: 5, aberration: 0.5 },
  });
  // Impacts on composition time (the slow-mo stretches clock time after 17.9).
  film.impact(1.0, { shake: 34, flash: '#ffffff', flashOpacity: 0.55, aberration: 7 });
  for (const s of strikes) film.impact(s, { shake: 10, aberration: 2.5 });
  film.impact(12.75, { shake: 40, flash: '#ffffff', flashOpacity: 0.7, aberration: 9 });
  // The bullet-time ramp lags the film clock by ∫0.88·e^(−x²) = 0.88·0.55·√π ≈ 0.86 s.
  film.impact(21.6 + 0.86, { shake: 30, flash: '#ff3b14', flashOpacity: 0.35, aberration: 6 });

  let clock = 0;
  const S = { wordmark: false };
  film.scene('one take', 0, 24.5, {
    // Bullet time inside the tunnel: the whole engine (and the film clock) drops to 12%.
    timeScale: (t) => 1 - 0.88 * Math.exp(-Math.pow((t - 18.7) / 0.55, 2)),
    step({ dt }) {
      clock += dt;
      cloud.step(dt);
      // Particle choreography (clock-driven).
      if (clock >= 12.72 && cloud.to !== 1 && clock < 16.4) { cloud.explode(0, 0, 0, 2600); cloud.from = 0; cloud.to = 1; cloud.mix = 0; S.flowT = clock; }
      if (cloud.to === 1 && S.flowT !== undefined) cloud.mix = ease('outCubic', clamp((clock - S.flowT) / 1.6));
      if (clock >= 16.4 && cloud.to !== 2) { cloud.from = 1; cloud.to = 2; cloud.mix = 0; S.coreT = clock; }
      if (cloud.to === 2) cloud.mix = ease('inExpo', clamp((clock - S.coreT) / 0.9));
    },
    draw({ ctx, t }) {
      const c = clock;
      // Camera.
      const k = hermite(c);
      Object.assign(cam.camera, { x: k.eye[0], y: k.eye[1], z: k.eye[2] });
      Object.assign(cam.target, { x: k.target[0], y: k.target[1], z: k.target[2] });
      cam.rotation.z = k.roll;
      // Solid geometry.
      place(c);
      const items = [];
      const cubesVisible = !(c > 13.6 && c < 16.6);
      if (cubesVisible) items.push({ mesh: cube, instances: cubes, material: { ambient: 0.16, diffuse: 0.9, specular: 0.55, shininess: 30, rim: 0.45 } });
      const ks = c < 8.5 ? 0 : c < 12.72 ? ease('elastic.out(1, 0.4)', clamp((c - 8.5) / 1.4)) : 0;
      if (ks > 0.01) {
        knotInst.set(0, { rx: c * 22, ry: c * 31, scale: ks, color: RED });
        items.push({ mesh: knot, instances: knotInst, material: { ambient: 0.12, diffuse: 0.75, specular: 1.1, shininess: 70, rim: 0.9 } });
      }
      const px = r3.render(cam, items, { background: '#06070a', fog: [1400, 5200], key: [0.5, 0.9, 0.4], keyColor: [1, 0.95, 0.9], fill: [-0.7, -0.3, 0.5], fillColor: [0.12, 0.16, 0.45], rimColor: [1, 0.4, 0.25] });
      ctx.putImageData(new globalThis.ImageData(px, W, H), 0, 0);
      // Particles as light, added over the solids.
      const pOn = c > 12.6 && c < 17.6;
      if (pOn) {
        g.render(cam, cloud, { size: 1.4, intensity: 0.55, fog: [200, 6000], maxRadius: 9, exposure: clamp((17.4 - c) / 0.6) });
        ctx.save(); ctx.globalCompositeOperation = 'lighter'; ctx.drawImage(gt.canvas, 0, 0, W, H); ctx.restore();
      }
      // Type: paper on the dark world, over a soft scrim where the scene is busy.
      const scrim = (y0, y1) => { const gr = ctx.createLinearGradient(0, y0, 0, y1); gr.addColorStop(0, 'rgba(6,7,10,0.72)'); gr.addColorStop(1, 'rgba(6,7,10,0)'); ctx.fillStyle = gr; ctx.fillRect(0, Math.min(y0, y1), W, Math.abs(y1 - y0)); };
      if ((c > 5.0 && c < 8.3) || (c > 9.6 && c < 12.7) || (c > 13.4 && c < 16.4)) scrim(0, 420);
      const slam = (s, x, y, size, color, start, o = {}) => typeFx(ctx, s, x, y, { size, color, start, t: c, effect: 'slam', font: o.italic ? 'DisplayItalic' : 'Display', align: o.align ?? 'center', ...o });
      const cap = (s, a = 1) => { ctx.globalAlpha = 0.75 * a; ctx.font = '400 16px Mono'; ctx.textAlign = 'left'; ctx.fillStyle = '#ece6da'; ctx.fillText(s.toUpperCase(), 86, H - 58); ctx.globalAlpha = 1; };
      if (c > 1.2 && c < 3.6) { slam('Motion,', 90, H - 200, 120, '#ece6da', 1.3, { align: 'left' }); slam('compiled.', 90, H - 84, 120, '#ff3b14', 1.45, { align: 'left', italic: true }); }
      if (c > 1.2 && c < 3.6) cap(`${K.toLocaleString('en-US')} solid cubes · lit + depth-tested in webassembly`);
      if (c > 5.0 && c < 8.3) { slam('SPRINGS.', W / 2, 200, 190, '#ece6da', 5.1); cap('every cube rides a spring · retargeted on every strike'); }
      if (c > 9.6 && c < 12.7) { slam('SOLID 3D.', W / 2, 190, 170, '#ece6da', 9.7); slam('NO WEBGL.', W / 2, 330, 120, '#ff3b14', 9.9); cap(`${(knot.triangles).toLocaleString('en-US')}-triangle knot · ${K.toLocaleString('en-US')} orbiting instances`); }
      if (c > 13.4 && c < 16.4) { slam('120,000 PARTICLES.', W / 2, 200, 150, '#ece6da', 13.5); cap('simulated + lit in webassembly · one spring each'); }
      if (c > 17.6 && c < 20.3) { slam('BULLET TIME.', W / 2, H / 2 + 60, 170, '#ece6da', 18.3); cap('one clock · engine.speed slows everything together'); }
      if (c > 21.8) {
        slam('Motion,', 90, H - 200, 120, '#ece6da', 22.0, { align: 'left' }); slam('compiled.', 90, H - 84, 120, '#ff3b14', 22.15, { align: 'left', italic: true });
        const a = clamp((c - 22.7) / 0.4);
        if (a > 0) { ctx.globalAlpha = a; ctx.textAlign = 'right'; ctx.fillStyle = '#ece6da'; ctx.font = '400 30px Mono'; ctx.fillText('NPM INSTALL ANIKIT', W - 86, H - 124); ctx.font = '400 16px Mono'; ctx.fillText('RUST · WEBASSEMBLY · TYPESCRIPT · PYTHON', W - 86, H - 84); ctx.globalAlpha = 1; }
      }
    },
  });
  return film;
}
