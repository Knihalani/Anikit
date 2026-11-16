// Bioluminescence: jellyfish and plankton on one full-screen 2D canvas, drawn
// additively over the water. Bells pulse on Anikit timelines; their tentacles
// are ropes in an Anikit physics world(), pinned to the rim and dragged
// through the water as the bells swim and the camera descends.
import { world, timeline, noise2 } from '../../../packages/anikit/dist/anikit.lean.js';

const TAU = Math.PI * 2;
const smooth = (a, b, x) => { const t = Math.min(1, Math.max(0, (x - a) / (b - a))); return t * t * (3 - 2 * t); };

function sprite(r, stops) {
  const c = document.createElement('canvas');
  c.width = c.height = r * 2;
  const g = c.getContext('2d');
  const gr = g.createRadialGradient(r, r, 0, r, r, r);
  for (const [o, col] of stops) gr.addColorStop(o, col);
  g.fillStyle = gr;
  g.fillRect(0, 0, r * 2, r * 2);
  return c;
}

export function life(canvas, { reduced, small }) {
  const ctx = canvas.getContext('2d');
  const dpr = Math.min(devicePixelRatio || 1, small ? 1.5 : 1.25);
  let W = 0, H = 0;
  const resize = () => { W = innerWidth; H = innerHeight; canvas.width = Math.round(W * dpr); canvas.height = Math.round(H * dpr); };
  resize();
  addEventListener('resize', resize);

  const dot = sprite(16, [[0, 'rgba(220,255,250,1)'], [0.25, 'rgba(110,255,220,0.55)'], [1, 'rgba(40,180,200,0)']]);
  const halo = sprite(64, [[0, 'rgba(150,230,255,0.55)'], [0.4, 'rgba(90,170,255,0.16)'], [1, 'rgba(60,100,255,0)']]);

  // --- Jellyfish -----------------------------------------------------------
  const w = world({ bounds: false, gravity: [0, 26], substeps: 3, iterations: 4, damping: 0.06, restitution: 0, friction: 0 });
  const palette = [[140, 230, 255], [185, 150, 255], [120, 255, 215], [255, 160, 230]];
  const nJ = small ? 3 : 4;
  const jellies = [];
  for (let k = 0; k < nJ; k++) {
    const R = (small ? 30 : 44) + ((k * 11) % 17) + (k === 1 ? 18 : 0);
    const j = {
      x: W * (0.16 + 0.7 * ((k * 0.618) % 1)), y: H * (0.25 + 0.6 * ((k * 0.43 + 0.2) % 1)) + H * 0.6,
      R, ang: (k % 2 ? 1 : -1) * 0.25, v: 0, c: 0, cPrev: 0, seed: k * 13.7, col: palette[k % palette.length], tent: [],
    };
    const nT = small ? 7 : 10, seg = 14;
    for (let i = 0; i < nT; i++) {
      // Around the rim, seen from the side: denser towards the edges.
      const u = Math.cos(((i + 0.5) / nT) * Math.PI + Math.sin(i * 3.1 + k) * 0.12);
      const len = R * (2.6 + 3.0 * Math.abs(Math.sin(i * 7.1 + k * 2.3)));
      const x0 = j.x + u * R * 0.8, y0 = j.y + R * 0.1;
      j.tent.push({ u, ids: w.rope([[x0, y0], [x0 + u * 6, y0 + len]], { segments: seg, radius: 0.4, stiffness: 0.9, pinStart: true }) });
    }
    // Pulse: contract quickly, relax slowly, forever. Thrust comes from the contraction.
    const period = 2.2 + k * 0.37;
    j.tl = timeline({ repeat: Infinity, delay: k * 0.7 })
      .add(j, { c: [0, 1] }, { duration: period * 0.28, ease: 'outCubic' })
      .add(j, { c: [1, 0] }, { duration: period * 0.72, ease: 'inOutSine' });
    if (reduced) j.tl.pause();
    jellies.push(j);
  }

  const shift = (j, dy) => {
    j.y += dy;
    for (const t of j.tent) for (const id of t.ids) { w.setPosition(id, w.x(id), w.y(id) + dy); }
  };

  function drawJelly(j, a) {
    const [r, g, b] = j.col;
    const cs = j.c;
    const bw = j.R * (1 - 0.2 * cs), bh = j.R * (0.78 + 0.16 * cs);
    const lum = a * (0.55 + 0.45 * cs);
    // Tentacles first.
    ctx.lineCap = 'round';
    for (const t of j.tent) {
      const ids = t.ids;
      ctx.beginPath();
      ctx.moveTo(w.x(ids[0]), w.y(ids[0]));
      for (let i = 1; i < ids.length - 1; i++) {
        const mx = (w.x(ids[i]) + w.x(ids[i + 1])) / 2, my = (w.y(ids[i]) + w.y(ids[i + 1])) / 2;
        ctx.quadraticCurveTo(w.x(ids[i]), w.y(ids[i]), mx, my);
      }
      const g2 = ctx.createLinearGradient(w.x(ids[0]), w.y(ids[0]), w.x(ids[ids.length - 1]), w.y(ids[ids.length - 1]));
      g2.addColorStop(0, `rgba(${r},${g},${b},${0.55 * lum})`);
      g2.addColorStop(1, `rgba(${r},${g},${b},0)`);
      ctx.strokeStyle = g2;
      ctx.lineWidth = 1.1;
      ctx.stroke();
      // A few light organs travelling down the tentacle.
      for (let i = 2; i < ids.length; i += 3) {
        const s = 3 + 3 * (1 - i / ids.length);
        ctx.globalAlpha = lum * (0.5 + 0.5 * Math.sin(j.phase * 3 - i * 0.7)) * (1 - i / ids.length);
        ctx.drawImage(dot, w.x(ids[i]) - s, w.y(ids[i]) - s, s * 2, s * 2);
      }
      ctx.globalAlpha = 1;
    }
    // Bell.
    ctx.save();
    ctx.translate(j.x, j.y);
    ctx.rotate(j.ang);
    ctx.globalAlpha = lum * 0.9;
    ctx.drawImage(halo, -j.R * 2.6, -j.R * 2.9, j.R * 5.2, j.R * 5.2);
    ctx.globalAlpha = 1;
    ctx.beginPath();
    ctx.moveTo(-bw, 0);
    ctx.bezierCurveTo(-bw, -bh * 1.35, bw, -bh * 1.35, bw, 0);
    const sc = 9;
    for (let i = sc; i >= 0; i--) { const x = -bw + (2 * bw * i) / sc; ctx.lineTo(x, (i % 2 ? 3.5 : 0) * (1 - cs * 0.5)); }
    ctx.closePath();
    const fill = ctx.createRadialGradient(0, -bh * 0.5, 0, 0, -bh * 0.4, bw * 1.1);
    fill.addColorStop(0, `rgba(${Math.min(255, r + 80)},${Math.min(255, g + 40)},255,${0.42 * lum})`);
    fill.addColorStop(0.6, `rgba(${r},${g},${b},${0.14 * lum})`);
    fill.addColorStop(1, `rgba(${r},${g},${b},${0.03 * lum})`);
    ctx.fillStyle = fill;
    ctx.fill();
    ctx.strokeStyle = `rgba(${Math.min(255, r + 60)},${Math.min(255, g + 30)},255,${0.7 * lum})`;
    ctx.lineWidth = 1.2;
    ctx.stroke();
    // Radial canals and the gonads: four soft loops.
    ctx.strokeStyle = `rgba(255,255,255,${0.22 * lum})`;
    ctx.lineWidth = 1;
    for (let i = -1.5; i <= 1.5; i += 1) {
      ctx.beginPath(); ctx.moveTo(0, -bh * 0.95); ctx.quadraticCurveTo(i * bw * 0.45, -bh * 0.6, i * bw * 0.55, 0); ctx.stroke();
    }
    ctx.globalAlpha = lum * (0.6 + 0.4 * cs);
    ctx.drawImage(dot, -bw * 0.35, -bh * 0.85, bw * 0.7, bw * 0.7);
    ctx.globalAlpha = 1;
    ctx.restore();
  }

  // --- Plankton --------------------------------------------------------------
  const N = small ? 900 : 2000;
  const px = new Float32Array(N), py = new Float32Array(N), pe = new Float32Array(N), pvx = new Float32Array(N), pvy = new Float32Array(N), ps = new Float32Array(N);
  for (let i = 0; i < N; i++) { px[i] = Math.random() * W; py[i] = Math.random() * H; ps[i] = 0.6 + Math.random() * 0.8; }
  const ptr = { x: -1e4, y: -1e4, px: -1e4, py: -1e4, last: -1e9 };
  addEventListener('pointermove', (e) => { ptr.x = e.clientX; ptr.y = e.clientY; ptr.last = performance.now(); }, { passive: true });
  const ghost = { x: W / 2, y: H / 2, px: W / 2, py: H / 2, t: 0 };

  function excite(x0, y0, x1, y1, radius, push) {
    const dx = x1 - x0, dy = y1 - y0, L2 = dx * dx + dy * dy || 1, r2 = radius * radius;
    const minX = Math.min(x0, x1) - radius, maxX = Math.max(x0, x1) + radius, minY = Math.min(y0, y1) - radius, maxY = Math.max(y0, y1) + radius;
    for (let i = 0; i < N; i++) {
      const x = px[i], y = py[i];
      if (x < minX || x > maxX || y < minY || y > maxY) continue;
      let t = ((x - x0) * dx + (y - y0) * dy) / L2; t = t < 0 ? 0 : t > 1 ? 1 : t;
      const ex = x - (x0 + dx * t), ey = y - (y0 + dy * t), d2 = ex * ex + ey * ey;
      if (d2 > r2) continue;
      const k = 1 - Math.sqrt(d2) / radius;
      if (k > pe[i]) pe[i] = k;
      pvx[i] += dx * push * k + ex * k * 0.6; pvy[i] += dy * push * k + ey * k * 0.6;
    }
  }

  let lastScroll = scrollY, time = 0;
  function frame(dtMs, depth) {
    const dt = Math.min(dtMs, 50) / 1000;
    time += dt;
    const bio = smooth(0.22, 0.55, depth), wake = smooth(0.1, 0.4, depth);
    const ds = scrollY - lastScroll; lastScroll = scrollY;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    if (bio <= 0.001 && wake <= 0.001) { for (const j of jellies) if (Math.abs(ds) > 0) shift(j, -ds * 0.6); return; }
    ctx.globalCompositeOperation = 'lighter';

    // A slow current sways the tentacles.
    if (!reduced) w.set({ gravity: [Math.sin(time * 0.5) * 22 + noise2(time * 0.2, 9) * 18, 22], substeps: 3, iterations: 4, damping: 0.06, restitution: 0, friction: 0 });
    // Jellies: steer slowly, swim on contraction, parallax with the descent.
    for (const j of jellies) {
      j.phase = time + j.seed;
      const dc = Math.max(0, j.c - j.cPrev); j.cPrev = j.c;
      j.v += dc * 160; j.v *= Math.exp(-dt * 1.3);
      const target = 0.35 * noise2(j.seed, time * 0.07);
      j.ang += (target - j.ang) * Math.min(1, dt * 0.6);
      const nx = Math.sin(j.ang), ny = -Math.cos(j.ang);
      j.x += nx * j.v * dt; 
      shift(j, ny * j.v * dt - ds * 0.6 + 6 * dt);
      if (j.x < -j.R * 3) j.x += W + j.R * 6; else if (j.x > W + j.R * 3) j.x -= W + j.R * 6;
      const span = H + j.R * 12;
      if (j.y < -j.R * 6) shift(j, span); else if (j.y > H + j.R * 6 + 40) shift(j, -span);
      // Pin the tentacle roots to the rim of the bell.
      const ca = Math.cos(j.ang), sa = Math.sin(j.ang), bw = j.R * (1 - 0.2 * j.c);
      for (const t of j.tent) { const lx = t.u * bw * 0.85, ly = 2; w.setPosition(t.ids[0], j.x + lx * ca - ly * sa, j.y + lx * sa + ly * ca); }
    }
    if (bio > 0.001) for (const j of jellies) drawJelly(j, bio);

    // Plankton: excited by the pointer, or by an unseen swimmer when it's idle.
    if (wake > 0.001) {
      const idle = performance.now() - ptr.last > 2500;
      if (!idle && ptr.px > -1e3) excite(ptr.px, ptr.py, ptr.x, ptr.y, small ? 60 : 80, 0.02);
      ptr.px = ptr.x; ptr.py = ptr.y;
      if (idle && !reduced) {
        ghost.t += dt;
        ghost.x = W * (0.5 + 0.38 * Math.sin(ghost.t * 0.23) + 0.08 * noise2(ghost.t * 0.3, 1));
        ghost.y = H * (0.5 + 0.32 * Math.sin(ghost.t * 0.37 + 1) + 0.08 * noise2(2, ghost.t * 0.3));
        excite(ghost.px, ghost.py, ghost.x, ghost.y, 60, 0.015);
      }
      ghost.px = ghost.x; ghost.py = ghost.y;
      const decay = Math.exp(-dt * 0.9), damp = Math.exp(-dt * 2.2);
      for (let i = 0; i < N; i++) {
        let y = py[i] - ds * 0.8 * ps[i] - dt * 4 * ps[i];
        let x = px[i] + pvx[i] * dt + Math.sin(time * 0.3 + i) * dt * 2;
        y += pvy[i] * dt;
        pvx[i] *= damp; pvy[i] *= damp;
        if (y < -10) { y += H + 20; pe[i] = 0; } else if (y > H + 10) { y -= H + 20; pe[i] = 0; }
        if (x < -10) x += W + 20; else if (x > W + 10) x -= W + 20;
        px[i] = x; py[i] = y;
        pe[i] *= decay;
        const e = pe[i];
        const a = (0.02 * bio * ps[i] + e * 0.95) * wake;
        if (a < 0.02) continue;
        const s = 1.6 + e * 4 * ps[i];
        ctx.globalAlpha = Math.min(1, a);
        ctx.drawImage(dot, x - s, y - s, s * 2, s * 2);
      }
      ctx.globalAlpha = 1;
    }
    ctx.globalCompositeOperation = 'source-over';
  }

  return { frame };
}
