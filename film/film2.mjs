// ANIKIT — CALIBRE. A macro product film of a mechanical watch: the balance wheel breathing in
// slow motion, a glide along the going train (escape wheel, pallet fork, fourth, third and centre
// wheels, every one cut with real teeth and turning at its true ratio from a 4 Hz escapement),
// a rotor sweeping over Côtes de Genève, then the movement flips, seats into its case and the
// dial, hands, bezel and crystal close over it. It ends at 10:10 on black glass.
// Every pixel is rendered by anikit's WebAssembly rasteriser; no WebGL.
//
//   anikit-render film2.mjs --still 1.5 --out still.jpg
//   anikit-render film2.mjs --out film2.mp4 --workers 4 --preroll 0 --fade-out 0.5
export default async function film2({ anikit: A, node }) {
  const { composition, scene3d, renderer3d, mesh, instances, texture } = A;
  const W = 1920, H = 1080, FPS = 60, D = 20;
  const PI = Math.PI, TAU = PI * 2, DEG = PI / 180;
  let seed = 11;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
  const smooth = (a, b, x) => { const t = clamp((x - a) / (b - a)); return t * t * (3 - 2 * t); };
  const ease = (a, b, x) => { const t = clamp((x - a) / (b - a)); return t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2; }; // inOutCubic
  const easeQ = (a, b, x) => { const t = clamp((x - a) / (b - a)); return t < 0.5 ? 8 * t ** 4 : 1 - (-2 * t + 2) ** 4 / 2; };  // inOutQuart
  const lerp = (a, b, k) => a + (b - a) * k;
  const lerp3 = (a, b, k) => [lerp(a[0], b[0], k), lerp(a[1], b[1], k), lerp(a[2], b[2], k)];

  // ---- Affine 3×4 matrices, column-major like the instance buffer -------------------------------
  const M = (r00, r10, r20, r01, r11, r21, r02, r12, r22, x = 0, y = 0, z = 0) => [r00, r10, r20, r01, r11, r21, r02, r12, r22, x, y, z];
  const T = (x, y, z) => M(1, 0, 0, 0, 1, 0, 0, 0, 1, x, y, z);
  const S = (x, y = x, z = x) => M(x, 0, 0, 0, y, 0, 0, 0, z);
  const Rx = (a) => { const c = Math.cos(a), s = Math.sin(a); return M(1, 0, 0, 0, c, s, 0, -s, c); };
  // Ry(-θ) turns a point's polar angle atan2(z, x) by +θ; spin(θ) is that.
  const spin = (th) => { const c = Math.cos(th), s = Math.sin(th); return M(c, 0, s, 0, 1, 0, -s, 0, c); };
  const Rz = (a) => { const c = Math.cos(a), s = Math.sin(a); return M(c, s, 0, -s, c, 0, 0, 0, 1); };
  const mul = (A2, B) => {
    const o = new Array(12);
    for (let c = 0; c < 3; c++) for (let r = 0; r < 3; r++) o[c * 3 + r] = A2[r] * B[c * 3] + A2[3 + r] * B[c * 3 + 1] + A2[6 + r] * B[c * 3 + 2];
    for (let r = 0; r < 3; r++) o[9 + r] = A2[r] * B[9] + A2[3 + r] * B[10] + A2[6 + r] * B[11] + A2[9 + r];
    return o;
  };
  const chain = (...ms) => ms.reduce((a, b) => mul(a, b));
  const put = (inst, i, m, col) => { const d = inst.data, b = i * 15; for (let k = 0; k < 12; k++) d[b + k] = m[k]; d[b + 12] = col[0]; d[b + 13] = col[1]; d[b + 14] = col[2]; };

  // ---- Mesh builder ------------------------------------------------------------------------------
  class MB {
    constructor() { this.P = []; this.N = []; this.U = []; this.I = []; }
    v(x, y, z, nx, ny, nz, u = 0, w = 0) { this.P.push(x, y, z); const l = Math.hypot(nx, ny, nz) || 1; this.N.push(nx / l, ny / l, nz / l); this.U.push(u, w); return this.P.length / 3 - 1; }
    tri(a, b, c, n) { // wound so its geometric normal agrees with n (or the vertex normals)
      const P = this.P;
      const ux = P[b * 3] - P[a * 3], uy = P[b * 3 + 1] - P[a * 3 + 1], uz = P[b * 3 + 2] - P[a * 3 + 2];
      const vx = P[c * 3] - P[a * 3], vy = P[c * 3 + 1] - P[a * 3 + 1], vz = P[c * 3 + 2] - P[a * 3 + 2];
      const cx = uy * vz - uz * vy, cy = uz * vx - ux * vz, cz = ux * vy - uy * vx;
      const N = this.N, nn = n ?? [N[a * 3] + N[b * 3] + N[c * 3], N[a * 3 + 1] + N[b * 3 + 1] + N[c * 3 + 1], N[a * 3 + 2] + N[b * 3 + 2] + N[c * 3 + 2]];
      if (cx * nn[0] + cy * nn[1] + cz * nn[2] * 1 >= 0) this.I.push(a, b, c); else this.I.push(a, c, b);
    }
    quad(a, b, c, d, n) { this.tri(a, b, c, n); this.tri(a, c, d, n); }
    // A flat quad wall between two points of an outline (xz), from y0 to y1, facing away from (ox, oz).
    wall(x0, z0, x1, z1, y0, y1, out = 1) {
      let nx = z1 - z0, nz = -(x1 - x0);
      const mx = (x0 + x1) / 2, mz = (z0 + z1) / 2;
      if ((nx * mx + nz * mz) * out < 0) { nx = -nx; nz = -nz; }
      const a = this.v(x0, y0, z0, nx, 0, nz), b = this.v(x1, y0, z1, nx, 0, nz), c = this.v(x1, y1, z1, nx, 0, nz), d = this.v(x0, y1, z0, nx, 0, nz);
      this.quad(a, b, c, d, [nx, 0, nz]);
    }
    box(cx, cz, ang, len, wid, y0, y1) { // a bar centred at (cx, cz), long along polar angle ang
      const ux = Math.cos(ang), uz = Math.sin(ang), wx = -uz, wz = ux;
      const p = [[-len / 2, -wid / 2], [len / 2, -wid / 2], [len / 2, wid / 2], [-len / 2, wid / 2]].map(([a, b]) => [cx + ux * a + wx * b, cz + uz * a + wz * b]);
      for (const y of [y0, y1]) { const s = y === y1 ? 1 : -1; const q = p.map(([x, z]) => this.v(x, y, z, 0, s, 0, x / 300 + 0.5, z / 300 + 0.5)); this.quad(q[0], q[1], q[2], q[3], [0, s, 0]); }
      for (let k = 0; k < 4; k++) { const [x0, z0] = p[k], [x1, z1] = p[(k + 1) % 4]; let nx = z1 - z0, nz = -(x1 - x0); if (nx * (x0 - cx) + nz * (z0 - cz) < 0) { nx = -nx; nz = -nz; } const a = this.v(x0, y0, z0, nx, 0, nz), b = this.v(x1, y0, z1, nx, 0, nz), c = this.v(x1, y1, z1, nx, 0, nz), d = this.v(x0, y1, z0, nx, 0, nz); this.quad(a, b, c, d, [nx, 0, nz]); }
    }
    build() { return mesh.custom(new Float32Array(this.P), new Float32Array(this.N), new Uint32Array(this.I), new Float32Array(this.U)); }
  }

  // A wheel: a toothed outline extruded from y0 to y1, a rim down to rIn, spokes and a hub.
  // Teeth are centred on polar angles 2πj/N (before the wheel turns).
  const gear = ({ n, rp, add, ded, rIn = 0, spokes = 0, spokeW = 3, hubR = 0, y0 = 0, y1 = 3, kind = 'wheel', per = 12, tw = 0.5 }) => {
    const b = new MB(), rT = rp + add, rR = rp - ded;
    const out = [];
    if (n === 0) for (let i = 0; i < 256; i++) out.push([(i / 256) * TAU, rp]);
    else for (let j = 0; j < n; j++) for (let s = 0; s < per; s++) {
      const u = s / per, a = ((j + u - tw / 2) / n) * TAU;
      let r;
      if (u < tw) {
        const v = u / tw;
        if (kind === 'escape') r = rR + (rT - rR) * (v < 0.8 ? (v / 0.8) ** 0.6 : (1 - v) / 0.2);
        else r = rR + (rT - rR) * Math.min(1, 1.08 * Math.sqrt(Math.sin(PI * v)));
      } else r = rR - 0.12 * (rp - rR) * Math.sin((PI * (u - tw)) / (1 - tw));
      out.push([a, r]);
    }
    const L = out.length;
    const ring = (y, s) => { // top (s = 1) or bottom (s = −1) face
      const o = [], q = [];
      for (const [a, r] of out) {
        const x = Math.cos(a) * r, z = Math.sin(a) * r;
        o.push(b.v(x, y, z, 0, s, 0, x / (2 * rT) + 0.5, z / (2 * rT) + 0.5));
        const ri = rIn; const xi = Math.cos(a) * ri, zi = Math.sin(a) * ri;
        q.push(b.v(xi, y, zi, 0, s, 0, xi / (2 * rT) + 0.5, zi / (2 * rT) + 0.5));
      }
      for (let i = 0; i < L; i++) { const k = (i + 1) % L; if (rIn > 0) b.quad(o[i], o[k], q[k], q[i], [0, s, 0]); else b.tri(o[i], o[k], q[i], [0, s, 0]); }
    };
    ring(y1, 1); ring(y0, -1);
    for (let i = 0; i < L; i++) {
      const [a0, r0] = out[i], [a1, r1] = out[(i + 1) % L];
      b.wall(Math.cos(a0) * r0, Math.sin(a0) * r0, Math.cos(a1) * r1, Math.sin(a1) * r1, y0, y1, 1);
    }
    if (rIn > 0) {
      const K = 128;
      for (let i = 0; i < K; i++) {
        const a0 = (i / K) * TAU, a1 = ((i + 1) / K) * TAU;
        const p = [[a0, y0], [a1, y0], [a1, y1], [a0, y1]].map(([a, y]) => b.v(Math.cos(a) * rIn, y, Math.sin(a) * rIn, -Math.cos(a), 0, -Math.sin(a)));
        b.quad(p[0], p[1], p[2], p[3]);
      }
      for (let k = 0; k < spokes; k++) { const a = (k / spokes) * TAU + 0.3, rm = (hubR + rIn) / 2; b.box(Math.cos(a) * rm, Math.sin(a) * rm, a, rIn - hubR + 1.5, spokeW, y0 + 0.4, y1 - 0.4); }
      if (hubR > 0) disc(b, 0, 0, hubR, y0, y1 + 0.6, 48);
    }
    return b.build();
  };
  // A solid round boss (cylinder) inside a builder.
  const disc = (b, cx, cz, r, y0, y1, K = 32) => {
    for (const [y, s] of [[y1, 1], [y0, -1]]) { const c0 = b.v(cx, y, cz, 0, s, 0); for (let i = 0; i < K; i++) { const a0 = (i / K) * TAU, a1 = ((i + 1) / K) * TAU; b.tri(c0, b.v(cx + Math.cos(a0) * r, y, cz + Math.sin(a0) * r, 0, s, 0), b.v(cx + Math.cos(a1) * r, y, cz + Math.sin(a1) * r, 0, s, 0), [0, s, 0]); } }
    for (let i = 0; i < K; i++) { const a0 = (i / K) * TAU, a1 = ((i + 1) / K) * TAU; const p = [[a0, y0], [a1, y0], [a1, y1], [a0, y1]].map(([a, y]) => b.v(cx + Math.cos(a) * r, y, cz + Math.sin(a) * r, Math.cos(a), 0, Math.sin(a))); b.quad(p[0], p[1], p[2], p[3]); }
  };
  // A polar surface (annular sector) with a normal function: Côtes de Genève, sunray, domes.
  const polar = ({ r0, r1, a0 = 0, a1 = TAU, nr, na, y, normal, flatSeg = false, yAt, uvR = r1, walls = true, y0, face = [0, 1, 0] }) => {
    const b = new MB(), full = Math.abs(a1 - a0 - TAU) < 1e-6;
    const hy = (x, z) => (yAt ? yAt(x, z) : y);
    if (!flatSeg) {
      const idx = [];
      for (let i = 0; i <= nr; i++) for (let j = 0; j <= na; j++) {
        const r = lerp(r0, r1, i / nr), a = lerp(a0, a1, j / na), x = Math.cos(a) * r, z = Math.sin(a) * r;
        const n = normal(x, z, a, r, j); idx.push(b.v(x, hy(x, z), z, n[0], n[1], n[2], x / (2 * uvR) + 0.5, z / (2 * uvR) + 0.5));
      }
      for (let i = 0; i < nr; i++) for (let j = 0; j < na; j++) { const p = i * (na + 1) + j; b.quad(idx[p], idx[p + 1], idx[p + na + 2], idx[p + na + 1], face); }
    } else {
      for (let j = 0; j < na; j++) { // one flat-shaded wedge per angular segment (crisp radial streaks)
        const aa = lerp(a0, a1, j / na), ab = lerp(a0, a1, (j + 1) / na), n = normal(0, 0, (aa + ab) / 2, 0, j);
        const pts = [[r0, aa], [r0, ab], [r1, ab], [r1, aa]].map(([r, a]) => { const x = Math.cos(a) * r, z = Math.sin(a) * r; return b.v(x, hy(x, z), z, n[0], n[1], n[2], x / (2 * uvR) + 0.5, z / (2 * uvR) + 0.5); });
        if (r0 > 0) b.quad(pts[0], pts[1], pts[2], pts[3], [0, 1, 0]); else b.tri(pts[0], pts[2], pts[3], [0, 1, 0]);
      }
    }
    if (walls && y0 !== undefined) {
      const K = Math.max(8, Math.round(na / 2));
      for (const r of r0 > 0 ? [r0, r1] : [r1]) for (let j = 0; j < K; j++) {
        const aa = lerp(a0, a1, j / K), ab = lerp(a0, a1, (j + 1) / K), s = r === r1 ? 1 : -1;
        const p = [[aa, y0], [ab, y0], [ab, null], [aa, null]].map(([a, yy]) => { const x = Math.cos(a) * r, z = Math.sin(a) * r; return b.v(x, yy ?? hy(x, z), z, s * Math.cos(a), 0, s * Math.sin(a)); });
        b.quad(p[0], p[1], p[2], p[3]);
      }
      if (!full) for (const a of [a0, a1]) {
        const s = a === a0 ? -1 : 1, nx = -Math.sin(a) * s, nz = Math.cos(a) * s;
        const p = [[r0, y0], [r1, y0], [r1, null], [r0, null]].map(([r, yy]) => { const x = Math.cos(a) * r, z = Math.sin(a) * r; return b.v(x, yy ?? hy(x, z), z, nx, 0, nz); });
        b.quad(p[0], p[1], p[2], p[3]);
      }
      // underside
      for (let j = 0; j < K; j++) {
        const aa = lerp(a0, a1, j / K), ab = lerp(a0, a1, (j + 1) / K);
        const p = [[r0, aa], [r0, ab], [r1, ab], [r1, aa]].map(([r, a]) => b.v(Math.cos(a) * r, y0, Math.sin(a) * r, 0, -1, 0));
        if (r0 > 0) b.quad(p[0], p[1], p[2], p[3], [0, -1, 0]); else b.tri(p[0], p[2], p[3], [0, -1, 0]);
      }
    }
    return b.build();
  };
  // A lathe: profile [[r, y, corner?], …] swept around Y. Corners split the normals (crisp bevels).
  const lathe = (prof, K = 192) => {
    const b = new MB();
    for (let k = 0; k < prof.length - 1; k++) {
      const [ra, ya] = prof[k], [rb, yb] = prof[k + 1];
      const segN = (p, q) => { const dr = q[0] - p[0], dy = q[1] - p[1]; const l = Math.hypot(dr, dy); return [-dy / l, dr / l]; }; // outward for a profile running top→bottom outside
      const nAB = segN(prof[k], prof[k + 1]);
      const nAt = (i) => {
        if (prof[i][2] || i === 0 || i === prof.length - 1) return nAB;
        const o = i === k ? segN(prof[k - 1], prof[k]) : segN(prof[k + 1], prof[k + 2] ?? prof[k + 1]);
        if (i === k + 1 && !prof[k + 2]) return nAB;
        const s = [nAB[0] + o[0], nAB[1] + o[1]]; const l = Math.hypot(s[0], s[1]); return [s[0] / l, s[1] / l];
      };
      const na = nAt(k), nb = nAt(k + 1);
      for (let i = 0; i < K; i++) {
        const a0 = (i / K) * TAU, a1 = ((i + 1) / K) * TAU;
        const vv = (r, y, n, a) => b.v(Math.cos(a) * r, y, Math.sin(a) * r, Math.cos(a) * n[0], n[1], Math.sin(a) * n[0], i / K, k / prof.length);
        const p0 = vv(ra, ya, na, a0), p1 = vv(ra, ya, na, a1), p2 = vv(rb, yb, nb, a1), p3 = vv(rb, yb, nb, a0);
        const am = (a0 + a1) / 2;
        b.quad(p0, p1, p2, p3, [Math.cos(am) * nAB[0], nAB[1], Math.sin(am) * nAB[0]]);
      }
    }
    return b.build();
  };

  // ---- Textures ----------------------------------------------------------------------------------
  const cv = (w, h) => node.createCanvas(w, h);
  const perlage = (() => {
    const c = cv(1024, 1024), g = c.getContext('2d');
    g.fillStyle = 'rgb(150,150,152)'; g.fillRect(0, 0, 1024, 1024);
    const sp = 26;
    for (let row = -1; row < 1024 / sp + 2; row++) for (let col = -1; col < 1024 / sp + 2; col++) {
      const x = col * sp + (row % 2 ? sp / 2 : 0) + (rnd() - 0.5) * 2, y = row * sp * 0.87 + (rnd() - 0.5) * 2;
      const gr = g.createRadialGradient(x - 5, y - 6, 1, x, y, 19);
      gr.addColorStop(0, 'rgb(236,236,238)'); gr.addColorStop(0.35, 'rgb(196,196,199)'); gr.addColorStop(0.75, 'rgb(150,150,153)'); gr.addColorStop(1, 'rgb(118,118,121)');
      g.fillStyle = gr; g.beginPath(); g.arc(x, y, 18, 0, TAU); g.fill();
    }
    return texture(c.getContext("2d").getImageData(0, 0, c.width, c.height));
  })();
  const DIALPX = 2048;
  const dialTex = (() => {
    const c = cv(DIALPX, DIALPX), g = c.getContext('2d'), R = DIALPX / 2;
    const gr = g.createRadialGradient(R, R, 0, R, R, R);
    gr.addColorStop(0, 'rgb(40,70,140)'); gr.addColorStop(0.55, 'rgb(24,46,104)'); gr.addColorStop(0.9, 'rgb(10,20,52)'); gr.addColorStop(1, 'rgb(6,12,32)');
    g.fillStyle = gr; g.fillRect(0, 0, DIALPX, DIALPX);
    g.lineWidth = 1.2;
    for (let i = 0; i < 2600; i++) { // sunray brushing
      const a = rnd() * TAU, k = rnd();
      g.strokeStyle = k < 0.5 ? `rgba(255,255,255,${0.012 * rnd()})` : `rgba(0,0,0,${0.02 * rnd()})`;
      g.beginPath(); g.moveTo(R + Math.cos(a) * 20, R + Math.sin(a) * 20); g.lineTo(R + Math.cos(a) * R, R + Math.sin(a) * R); g.stroke();
    }
    // Minute track
    g.strokeStyle = 'rgba(232,236,242,0.92)'; g.lineWidth = 3;
    for (const rr of [0.905, 0.955]) { g.beginPath(); g.arc(R, R, R * rr, 0, TAU); g.stroke(); }
    for (let m = 0; m < 60; m++) {
      const a = (m / 60) * TAU, five = m % 5 === 0;
      g.lineWidth = five ? 7 : 3.5;
      g.beginPath(); g.moveTo(R + Math.cos(a) * R * (five ? 0.895 : 0.905), R + Math.sin(a) * R * (five ? 0.895 : 0.905)); g.lineTo(R + Math.cos(a) * R * 0.955, R + Math.sin(a) * R * 0.955); g.stroke();
      for (let q = 1; q < 5; q++) { const b2 = ((m + q / 5) / 60) * TAU; g.lineWidth = 1.6; g.beginPath(); g.moveTo(R + Math.cos(b2) * R * 0.94, R + Math.sin(b2) * R * 0.94); g.lineTo(R + Math.cos(b2) * R * 0.955, R + Math.sin(b2) * R * 0.955); g.stroke(); }
    }
    // A fine railway ring and a small seconds-scale dot ring inside the indices
    g.fillStyle = 'rgba(220,226,236,0.75)';
    for (let m = 0; m < 60; m++) { const a = (m / 60) * TAU; g.beginPath(); g.arc(R + Math.cos(a) * R * 0.62, R + Math.sin(a) * R * 0.62, m % 5 ? 2.2 : 4, 0, TAU); g.fill(); }
    return texture(c.getContext("2d").getImageData(0, 0, c.width, c.height));
  })();

  // ---- The movement (local space: mainplate top at y = 0, bridges above) -------------------------
  // Going train (module per stage; centre distance = m (N1 + N2) / 2).
  const C0 = [0, 0], CW = { n: 80, m: 1.0 }, P3 = { n: 10 }, TW = { n: 75, m: 0.9 }, P4 = { n: 10 }, FW = { n: 112, m: 0.75 }, PE = { n: 7 }, EW = { n: 15 };
  const at = (p, d, a) => [p[0] + d * Math.cos(a * DEG), p[1] + d * Math.sin(a * DEG)];
  const T3 = at(C0, CW.m * (CW.n + P3.n) / 2, -20);
  const F4 = at(T3, TW.m * (TW.n + P4.n) / 2, -30);
  const E5 = at(F4, FW.m * (FW.n + PE.n) / 2, -100);
  const PAL = at(E5, 30, 200);
  const BAL = at(E5, 70, 200);
  const ang = (p, q) => Math.atan2(q[1] - p[1], q[0] - p[0]);
  // Phases so that a tooth of each wheel points at its pinion and the pinion shows a gap there.
  const psiC3 = ang(C0, T3), psi34 = ang(T3, F4), psi4E = ang(F4, E5);
  const PH = { cw: psiC3, p3: psiC3 + PI - PI / P3.n, tw: psi34, p4: psi34 + PI - PI / P4.n, fw: psi4E, pe: psi4E + PI - PI / PE.n, ew: 0.1 };
  const Y = { cw: [1, 3.5], tw: [6, 8.5], fw: [10.5, 13], ew: [2.5, 5], bridge: [16, 21] };
  const wheelM = (spec, y, extra) => gear({ n: spec.n, rp: spec.m * spec.n / 2, add: spec.m * 1.25, ded: spec.m * 1.5, y0: y[0], y1: y[1], per: spec.n > 90 ? 9 : 12, tw: 0.46, ...extra });
  const pinionM = (n, m, y) => gear({ n, rp: m * n / 2, add: m * 0.75, ded: m * 1.4, rIn: 0, y0: y[0], y1: y[1], per: 14, tw: 0.42 });
  const centreWheel = wheelM(CW, Y.cw, { rIn: 30, spokes: 4, spokeW: 4.5, hubR: 6 });
  const thirdWheel = wheelM(TW, Y.tw, { rIn: 25, spokes: 4, spokeW: 3.5, hubR: 4.5 });
  const fourthWheel = wheelM(FW, Y.fw, { rIn: 34, spokes: 4, spokeW: 3, hubR: 4.5 });
  const escapeWheel = gear({ n: EW.n, rp: 20, add: 4, ded: 3.5, rIn: 13, spokes: 5, spokeW: 1.8, hubR: 3.2, y0: Y.ew[0], y1: Y.ew[1], kind: 'escape', per: 16, tw: 0.62 });
  const pinion3 = pinionM(P3.n, CW.m, [0.4, 4.4]);
  const pinion4 = pinionM(P4.n, TW.m, [5.4, 9.2]);
  const pinionE = pinionM(PE.n, FW.m, [9.8, 13.6]);
  // Barrel under its bridge, and the ratchet wheel on top.
  const BARREL = [-88, 4];
  const ratchet = gear({ n: 90, rp: 40, add: 1.1, ded: 1.4, rIn: 0, y0: 27, y1: 31.5, per: 8, tw: 0.5 });
  const crownWheel = gear({ n: 40, rp: 15, add: 0.9, ded: 1.1, rIn: 0, y0: 27, y1: 30, per: 8, tw: 0.5 });
  const CROWNW = [-52, 58];
  // Plates: the mainplate (perlage) and the barrel bridge (Côtes de Genève as real normals).
  const BW = 17; // stripe width
  const cotes = (x) => { const f = ((x / BW) % 1 + 1) % 1; const k = 0.22 * Math.sin(TAU * f) + 0.06 * Math.sin(2 * TAU * f); return [-k, 1, 0]; };
  const mainplate = polar({ r0: 0, r1: 150, nr: 3, na: 160, y: 0, y0: -18, normal: () => [0, 1, 0], uvR: 150 });
  const mainBottom = polar({ r0: 0, r1: 150, nr: 3, na: 160, y: -18.4, walls: false, normal: () => [0, -1, 0], uvR: 150, face: [0, -1, 0] }); // perlage dial side (seen after the flip)
  const barrelBridge = polar({ r0: 56, r1: 146, a0: 108 * DEG, a1: 252 * DEG, nr: 64, na: 240, y: 26, y0: 18, normal: (x) => cotes(x), uvR: 150 });
  // Rotor: a half disc with Côtes, and a heavy gold rim.
  const rotorPlate = polar({ r0: 15, r1: 128, a0: -90 * DEG, a1: 90 * DEG, nr: 54, na: 220, y: 41, y0: 38, normal: (x, z) => { const f = (((z + 500) / BW) % 1); return [0, 1, -(0.22 * Math.sin(TAU * f) + 0.06 * Math.sin(2 * TAU * f))]; }, uvR: 150 });
  const rotorRim = polar({ r0: 126, r1: 146, a0: -92 * DEG, a1: 92 * DEG, nr: 4, na: 160, y: 42, y0: 30, normal: (x, z, a, r) => [Math.cos(a) * (r - 136) * 0.04, 1, Math.sin(a) * (r - 136) * 0.04], uvR: 150 });
  const rotorHub = (() => { const b = new MB(); disc(b, 0, 0, 16, 36, 44, 64); return b.build(); })();
  // Bridges for the train and the balance cock: bars with round bosses.
  const bars = (() => {
    const b = new MB(), y0 = Y.bridge[0], y1 = Y.bridge[1];
    const bar = (p, q, w) => { const a = ang(p, q), l = Math.hypot(q[0] - p[0], q[1] - p[1]); b.box((p[0] + q[0]) / 2, (p[1] + q[1]) / 2, a, l, w, y0, y1); };
    const A1 = [112, 26], A2 = [104, -100], A3 = [-57, 12];
    bar(C0, T3, 10); bar(T3, F4, 10); bar(F4, A1, 12); bar(F4, E5, 9); bar(E5, A2, 11); bar(C0, A3, 10); bar(PAL, E5, 7);
    for (const [p, r] of [[C0, 9], [T3, 7.5], [F4, 7.5], [E5, 7], [PAL, 5.5], [A1, 13], [A2, 13]]) disc(b, p[0], p[1], r, y0, y1, 40);
    return b.build();
  })();
  const COCKF = [-58, -112];
  const cock = (() => {
    const b = new MB(), y0 = 22, y1 = 27;
    const a = ang(BAL, COCKF), l = Math.hypot(COCKF[0] - BAL[0], COCKF[1] - BAL[1]);
    b.box((BAL[0] + COCKF[0]) / 2, (BAL[1] + COCKF[1]) / 2, a, l, 13, y0, y1);
    disc(b, BAL[0], BAL[1], 10, y0, y1, 48); disc(b, COCKF[0], COCKF[1], 19, y0, y1, 48);
    disc(b, COCKF[0], COCKF[1], 13, 0, y0, 32); // pillar
    return b.build();
  })();
  // Pallet fork (local: pivot at origin, lever along +x towards the balance, arms towards −x).
  const pallet = (() => {
    const b = new MB(), y0 = 3, y1 = 4.6;
    b.box(11, 0, 0, 24, 2.4, y0, y1);              // lever
    b.box(23.5, -1.8, 0, 3.5, 1.2, y0, y1); b.box(23.5, 1.8, 0, 3.5, 1.2, y0, y1); // fork horns
    b.box(-9, 0, 0, 18, 3.2, y0, y1);              // body towards the escape wheel
    b.box(-13, 0, PI / 2, 26, 3.2, y0, y1);        // anchor arms
    disc(b, 0, 0, 2.6, y0, y1 + 0.4, 24);
    return b.build();
  })();
  const STONES = [[-14.5, -12.5], [-14.5, 12.5]];
  // Balance wheel (local: centre at origin, y 8–12) and its screws.
  const BY = [8.5, 12];
  const balanceWheel = gear({ n: 0, rp: 38, add: 0, ded: 0, rIn: 34.5, spokes: 3, spokeW: 3.2, hubR: 3.5, y0: BY[0], y1: BY[1] });
  const roller = (() => { const b = new MB(); disc(b, 0, 0, 7, 6, 7.4, 40); disc(b, 0, 0, 1.2, -1, 24, 16); return b.build(); })();
  const HSY = 6.5; // hairspring plane
  const hairPts = (th) => { // the breathing spiral: the inner end turns with the balance, the outer end is pinned
    const n = 1150, out = new Float32Array(n * 3), k = th / (270 * DEG);
    for (let i = 0; i < n; i++) {
      const u = i / (n - 1), a = u * 13.25 * TAU + th * (1 - u) * 0.98, r = (4.2 + 23.5 * u) * (1 + 0.055 * k * Math.sin(PI * u) * (1 - u));
      out[i * 3] = Math.cos(a) * r; out[i * 3 + 1] = HSY; out[i * 3 + 2] = Math.sin(a) * r;
    }
    return out;
  };

  // Shared primitives
  const cyl = mesh.cylinder(40, true), cube = mesh.cube(), sph = mesh.sphere(18, 32);

  // ---- The watch (world space; the case lies on a black mirror at y = 0, 12 o'clock towards −z) ---
  const caseMesh = lathe([[150, 80, 1], [153, 86, 1], [197, 87, 1], [205, 82, 1], [209, 70], [210, 52], [208, 30, 1], [200, 14, 1], [182, 6], [120, 3], [0, 2.5, 1]], 256);
  const caseFloor = polar({ r0: 0, r1: 152, nr: 1, na: 96, y: 4, walls: false, normal: () => [0, 1, 0] });
  const caseInner = lathe([[150, 4, 1], [150, 80, 1]], 192); // inner wall (faces inwards)
  const bezelMesh = lathe([[151, 85, 1], [152, 92, 1], [162, 96], [184, 95.5], [196, 91, 1], [199, 86, 1]], 256);
  const crystal = polar({ r0: 0, r1: 152, nr: 16, na: 160, y: 0, walls: false, yAt: (x, z) => 98.5 - 3 * ((x * x + z * z) / (152 * 152)), normal: (x, z) => [x * 0.0026, 1, z * 0.0026] });
  const dialR = 150;
  const dialTop = polar({ r0: 3, r1: dialR, nr: 1, na: 360, y: 0, flatSeg: true, uvR: dialR, normal: (x, z, a, r, j) => { const s = Math.sin(j * 12.9898 + 78.233) * 43758.5453, h = (s - Math.floor(s)) - 0.5; const tl = 0.13 * h; return [-Math.sin(a) * tl, 1, Math.cos(a) * tl]; } });
  const dialBody = (() => { const b = new MB(); disc(b, 0, 0, dialR - 0.5, -3, -1.2, 160); return b.build(); })();
  // Dauphine hands: two facets meeting on a raised ridge.
  const dauphine = (L, w, tail) => {
    const b = new MB(), h = 1.6;
    const tip = [0, h, -L], base = [0, h, tail], l = [-w, 0, -L * 0.22], r = [w, 0, -L * 0.22], lt = [-w * 0.55, 0, tail], rt = [w * 0.55, 0, tail];
    const face = (p, q, s) => { const ux = q[0] - p[0], uy = q[1] - p[1], uz = q[2] - p[2], vx = s[0] - p[0], vy = s[1] - p[1], vz = s[2] - p[2]; let n = [uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx]; if (n[1] < 0) n = n.map((v) => -v); b.tri(b.v(...p, ...n), b.v(...q, ...n), b.v(...s, ...n), n); };
    face(tip, l, base); face(tip, base, r); face(l, lt, base); face(r, base, rt);
    face([lt[0], 0, tail], [rt[0], 0, tail], [0, h, tail]);
    disc(b, 0, 0, w * 0.9, -0.6, 0.9, 32);
    return b.build();
  };
  const hourHand = dauphine(88, 7.5, 6), minuteHand = dauphine(132, 6.2, 6);
  const secondHand = (() => { const b = new MB(); b.box(0, -40, PI / 2, 182, 1.5, 0, 1.0); disc(b, 0, 0, 4, -0.3, 1.6, 32); disc(b, 0, 36, 4.2, 0, 1.0, 32); return b.build(); })();
  const crownMesh = gear({ n: 36, rp: 24, add: 0.9, ded: 1.2, rIn: 0, y0: -12, y1: 12, per: 8, tw: 0.55 });
  const lugPts = []; for (let i = 0; i <= 24; i++) { const u = i / 24; lugPts.push([0, 60 - 16 * u * u - 4 * u, 170 + 56 * u]); } // short, tapering horns
  const lugMesh = mesh.tube(lugPts, { radius: 12.5, sides: 24, radii: lugPts.map((_, i) => 1 - 0.3 * (i / 24)) });
  const lugEnd = lugPts[24];
  const floor = mesh.plane(1);

  // ---- Instances --------------------------------------------------------------------------------
  const GOLD = [1.0, 0.73, 0.42], STEEL = [0.88, 0.89, 0.92], BLUE = [0.16, 0.28, 0.75], RUBY = [0.75, 0.035, 0.07], PLATE = [1, 1, 1];
  const one = () => instances(1);
  const I = {
    plate: one(), plateB: one(), bbridge: one(), bars: one(), cock: one(), ratchet: one(), crownW: one(),
    cw: one(), tw: one(), fw: one(), ew: one(), p3: one(), p4: one(), pe: one(), pal: one(), bal: one(), roller: one(), hair: one(),
    rotor: one(), rotorRim: one(), rotorHub: one(),
    arbors: instances(5), jewels: instances(12), chatons: instances(12), screws: instances(14), slots: instances(14), balScrews: instances(12), stones: instances(3),
    case: one(), caseFloor: one(), caseInner: one(), bezel: one(), crystal: one(), dialTop: one(), dialBody: one(), indices: instances(13),
    hh: one(), mh: one(), sh: one(), crown: one(), stem: one(), lugs: instances(4), lugCaps: instances(4), floor: one(),
  };
  const JEWELS = [[C0, Y.bridge[1]], [T3, Y.bridge[1]], [F4, Y.bridge[1]], [E5, Y.bridge[1]], [PAL, Y.bridge[1]], [BAL, 27], [BARREL, 26], [CROWNW, 30]];
  const SCREWS = [[[112, 26], Y.bridge[1], 6.5, BLUE], [[104, -100], Y.bridge[1], 6.5, BLUE], [COCKF, 27, 7, BLUE], [[-100, 95], 26, 6, BLUE], [[-128, 40], 26, 6, BLUE], [[-128, -40], 26, 6, BLUE], [[-100, -95], 26, 6, BLUE], [[-62, -26], 26, 5.5, BLUE], [BARREL, 31.5, 9, STEEL], [CROWNW, 30, 5.5, STEEL], [[-57, 12], Y.bridge[1], 5, BLUE]];

  // ---- Time: a slow-motion hook that ramps to real time ------------------------------------------
  const speed = (t) => 0.12 + 0.88 * smooth(1.3, 2.9, t);
  const CLK = new Float64Array(D * 1200 + 2);
  for (let i = 1; i < CLK.length; i++) CLK[i] = CLK[i - 1] + speed((i - 0.5) / 1200) / 1200;
  const clockAt = (t) => { const f = clamp(t, 0, D) * 1200, i = Math.floor(f); return CLK[i] + (CLK[Math.min(i + 1, CLK.length - 1)] - CLK[i]) * (f - i); };
  // 28,800 vph: the balance swings at 4 Hz, ±270°; the escape wheel advances half a tooth on each of 8 beats a second.
  const beats = (c) => { const b = c * 8, k = Math.floor(b), f = b - k; return k + (1 - (1 - clamp(f / 0.13)) ** 3); };
  const balAngle = (c) => 270 * DEG * Math.sin(TAU * 4 * c);

  // ---- Assembly choreography ---------------------------------------------------------------------
  const YH = 260, YSEAT = 53;
  const flipA = (t) => PI * ease(8.5, 9.75, t);
  const movY = (t) => lerp(YH, YSEAT, easeQ(9.45, 10.7, t));
  const eo = (a, b, x) => 1 - (1 - clamp((x - a) / (b - a))) ** 3; // fast in from out of frame, precise landing
  const dialY = (t) => lerp(650, 74, eo(10.35, 11.35, t));
  const handDrop = (t, t0) => eo(t0, t0 + 0.6, t);
  const bezelY = (t) => lerp(700, 0, eo(12.15, 12.95, t));
  const crystalY = (t) => lerp(750, 0, eo(12.6, 13.45, t));
  const rotorA = (t) => 90 * DEG + TAU * ease(6.0, 8.1, t) * 1.0;
  const Gmov = (t) => chain(T(0, movY(t), 0), Rx(flipA(t)));

  // ---- Camera ------------------------------------------------------------------------------------
  const W0 = (p, y) => [p[0], YH + y, p[1]]; // movement local → world (before the flip)
  const keys = [
    // t, camera, target
    [0.0, [BAL[0] + 84, YH + 50, BAL[1] - 76], [BAL[0] + 8, YH + 9, BAL[1] - 10]],
    [1.6, [BAL[0] + 50, YH + 46, BAL[1] - 100], [BAL[0] - 2, YH + 9, BAL[1] - 14]],
    [3.2, [PAL[0] + 30, YH + 55, PAL[1] - 72], W0(PAL, 5)],
    [4.4, [E5[0] + 58, YH + 52, E5[1] - 42], W0(lerp3([...E5, 0], [...F4, 0], 0.3), 8)],
    [5.4, [F4[0] + 78, YH + 58, F4[1] + 4], W0(F4, 10)],
    [6.4, [T3[0] + 40, YH + 96, T3[1] + 92], W0([T3[0] * 0.4, T3[1] * 0.4], 6)],
    [7.4, [-10, YH + 150, 150], W0([-62, 6], 26)],
  ];
  const orbit = (t) => {
    // Once the watch is closed, the orbit turns into a slow push-in that rises over the dial (the end
    // lands closer on 10:10 rather than holding a wide shot).
    const az = (-16 + 4.4 * (t - 8.2) + 26 * smooth(13.4, 20, t)) * DEG;
    const el = (lerp(lerp(44, 56, smooth(8.2, 9.6, t)), 24, smooth(9.8, 16, t)) + 12 * smooth(14.6, 20, t)) * DEG;
    const dist = lerp(lerp(190, 540, smooth(8.0, 9.9, t)), 900, smooth(10.0, 14.2, t)) - 250 * smooth(13.8, 20.2, t);
    const ty = lerp(movY(t) - 10, 40, smooth(10.0, 12.5, t));
    const tg = [0, ty, 8 * smooth(12, 16, t)];
    return [[tg[0] + Math.sin(az) * Math.cos(el) * dist, tg[1] + Math.sin(el) * dist, tg[2] + Math.cos(az) * Math.cos(el) * dist], tg];
  };
  for (const tk of [8.3, 9.0]) { const [p, g] = orbit(tk); keys.push([tk, p, g]); }
  const herm = (k, t, which) => { // Catmull-Rom (non-uniform) through the keys
    let i = 0; while (i < k.length - 2 && t > k[i + 1][0]) i++;
    const a = k[Math.max(0, i - 1)], b = k[i], c = k[i + 1], d = k[Math.min(k.length - 1, i + 2)];
    const u = clamp((t - b[0]) / (c[0] - b[0])), h = c[0] - b[0];
    const m0 = b[which].map((_, j) => ((c[which][j] - a[which][j]) / (c[0] - a[0] || 1)) * h);
    const m1 = b[which].map((_, j) => ((d[which][j] - b[which][j]) / (d[0] - b[0] || 1)) * h);
    const u2 = u * u, u3 = u2 * u;
    return b[which].map((_, j) => (2 * u3 - 3 * u2 + 1) * b[which][j] + (u3 - 2 * u2 + u) * m0[j] + (-2 * u3 + 3 * u2) * c[which][j] + (u3 - u2) * m1[j]);
  };
  const camAt = (t) => {
    const w = smooth(7.8, 8.9, t);
    if (w <= 0) return [herm(keys, t, 1), herm(keys, t, 2)];
    const [sp, sg] = orbit(t);
    if (w >= 1) return [sp, sg];
    return [lerp3(herm(keys, t, 1), sp, w), lerp3(herm(keys, t, 2), sg, w)];
  };
  const cam = scene3d({ fov: 30, width: W, height: H, near: 2, far: 12000 });
  const r3 = renderer3d({ width: W, height: H });

  const film = composition({
    width: W, height: H, fps: FPS, duration: D, motionBlur: 1,
    post: {
      aa: 1, bloom: 0.32, bloomThreshold: 0.78, bloomRadius: 30,
      streaks: 0.0, flares: 0, aberration: 0.15, vignette: 0.5, grain: 2.2,
      contrast: 1.07, saturation: 0.98, exposure: 1.0,
      shadows: [0.0, 0.004, 0.012], highlights: [0.03, 0.012, -0.012],
    },
  });

  const mats = {
    steel: { ambient: 0.15, diffuse: 0.6, metal: 0.9, specular: 1.6, shininess: 160, rim: 0.1 },
    gold: { ambient: 0.15, diffuse: 0.6, metal: 0.86, specular: 1.5, shininess: 120, rim: 0.1 },
    blued: { ambient: 0.1, diffuse: 0.5, metal: 0.85, specular: 1.6, shininess: 180, rim: 0.15 },
    cotes: { ambient: 0.12, diffuse: 0.55, metal: 0.78, specular: 1.3, shininess: 70, rim: 0.08 },
    perlage: { ambient: 0.12, diffuse: 0.7, metal: 0.4, specular: 0.7, shininess: 40, rim: 0.05, texture: perlage, textureRepeat: [1, 1], textureFilter: 'aniso' },
    ruby: { ambient: 0.25, diffuse: 0.55, specular: 2.4, shininess: 420, emissive: 0.55, metal: 0.25, rim: 0.7 },
    slot: { ambient: 0.02, diffuse: 0.05, specular: 0, rim: 0 },
    dial: { ambient: 0.3, diffuse: 0.85, metal: 0.18, specular: 1.6, shininess: 26, rim: 0.05, texture: dialTex, textureFilter: 'aniso' },
    crystal: { ambient: 0, diffuse: 0, specular: 1.4, shininess: 300, rim: 0.08, metal: 0.16 },
    floor: { ambient: 0.4, diffuse: 0.2, specular: 0.25, shininess: 300, rim: 0 },
  };

  const hairCache = { m: null };
  const draw = ({ t, pixels }) => {
      const c = clockAt(t);
      const s = beats(c), th = balAngle(c);
      const G = Gmov(t);
      const P = (x, z, y = 0) => T(x, y, z);
      // Movement parts
      const showMov = t < 11.45;
      const gearAt = (inst, p, phase, theta, col) => put(inst, 0, chain(G, P(p[0], p[1]), spin(phase + theta)), col);
      const thE = -12 * DEG * s, th4 = 0.75 * DEG * s, th3 = -0.1 * DEG * s, thC = 0.0125 * DEG * s;
      put(I.plate, 0, G, [0.52, 0.52, 0.54]); put(I.plateB, 0, G, [0.6, 0.6, 0.62]);
      put(I.bbridge, 0, G, [0.86, 0.87, 0.9]); put(I.bars, 0, G, [0.84, 0.85, 0.88]); put(I.cock, 0, G, [0.84, 0.85, 0.88]);
      put(I.ratchet, 0, chain(G, P(BARREL[0], BARREL[1]), spin(0.02 * c)), STEEL);
      put(I.crownW, 0, chain(G, P(CROWNW[0], CROWNW[1]), spin(-0.05 * c)), STEEL);
      gearAt(I.cw, C0, PH.cw, thC, GOLD); gearAt(I.p3, T3, PH.p3, th3, STEEL); gearAt(I.tw, T3, PH.tw, th3, GOLD);
      gearAt(I.p4, F4, PH.p4, th4, STEEL); gearAt(I.fw, F4, PH.fw, th4, GOLD); gearAt(I.pe, E5, PH.pe, thE, STEEL); gearAt(I.ew, E5, PH.ew, thE, STEEL);
      [C0, T3, F4, E5].forEach((p, i) => put(I.arbors, i, chain(G, P(p[0], p[1], 8), S(1.3, 16, 1.3)), STEEL));
      put(I.arbors, 4, chain(G, P(PAL[0], PAL[1], 8), S(1.0, 16, 1.0)), STEEL);
      // Pallet: rocks ±7° on each beat.
      const kb = Math.floor(s), fb = s - kb, side = (kb % 2 ? 1 : -1) * (1 - 2 * smooth(0, 1, fb));
      const palRot = 7 * DEG * side;
      const Gp = chain(G, P(PAL[0], PAL[1]), spin(ang(PAL, BAL) + palRot));
      put(I.pal, 0, Gp, STEEL);
      STONES.forEach(([x, z], i) => put(I.stones, i, chain(Gp, P(x, z, 3.8), spin(0.5 * (i ? 1 : -1)), S(2.2, 2.0, 3.6)), RUBY));
      // Balance, roller (with its impulse jewel) and the breathing hairspring.
      const Gb = chain(G, P(BAL[0], BAL[1]), spin(th));
      put(I.bal, 0, Gb, GOLD); put(I.roller, 0, Gb, STEEL);
      put(I.stones, 2, chain(Gb, P(5.6, 0, 5), S(1.1, 2.4, 1.1)), RUBY);
      for (let k = 0; k < 12; k++) { const a = (k / 12) * TAU + PI / 12; put(I.balScrews, k, chain(Gb, P(Math.cos(a) * 39, Math.sin(a) * 39, 10.25), spin(a), Rz(PI / 2), S(1.7, 3.2, 1.7)), GOLD); }
      if (hairCache.m) hairCache.m.free();
      hairCache.m = mesh.tube(hairPts(th), { radius: 0.34, sides: 6 });
      put(I.hair, 0, chain(G, P(BAL[0], BAL[1])), [0.55, 0.6, 0.72]);
      // Rotor
      const Gr = chain(G, spin(rotorA(t)));
      put(I.rotor, 0, Gr, GOLD); put(I.rotorRim, 0, Gr, GOLD); put(I.rotorHub, 0, Gr, STEEL);
      JEWELS.forEach(([p, y], i) => { put(I.jewels, i, chain(G, P(p[0], p[1], y + 0.6), S(3.0, 1.7, 3.0)), RUBY); put(I.chatons, i, chain(G, P(p[0], p[1], y + 0.5), S(5.2, 1.2, 5.2)), GOLD); });
      put(I.jewels, 8, chain(Gr, P(0, 0, 44.6), S(4, 2, 4)), RUBY); put(I.chatons, 8, chain(Gr, P(0, 0, 44.4), S(7, 1.4, 7)), STEEL);
      for (let k = 9; k < 12; k++) put(I.jewels, k, chain(Gr, P(Math.cos(k * 2.1) * 9, Math.sin(k * 2.1) * 9, 44.6), S(2.2, 1.2, 2.2)), STEEL), put(I.chatons, k, chain(Gr, P(0, 0, 0), S(0.01)), STEEL);
      SCREWS.forEach(([p, y, r, col], i) => {
        put(I.screws, i, chain(G, P(p[0], p[1], y + 1.1), S(r, 2.2, r)), col);
        put(I.slots, i, chain(G, P(p[0], p[1], y + 2.25), spin(i * 1.7), S(r * 1.9, 0.12, r * 0.28)), [0.01, 0.01, 0.012]);
      });
      for (let i = SCREWS.length; i < 14; i++) { put(I.screws, i, S(0.001), STEEL); put(I.slots, i, S(0.001), STEEL); }

      // The watch
      put(I.case, 0, T(0, 0, 0), STEEL); put(I.caseFloor, 0, T(0, 0, 0), [0.3, 0.3, 0.32]); put(I.caseInner, 0, T(0, 0, 0), [0.6, 0.6, 0.62]);
      put(I.bezel, 0, T(0, bezelY(t), 0), STEEL);
      put(I.crystal, 0, T(0, crystalY(t), 0), [0.5, 0.52, 0.56]);
      const dy = dialY(t);
      put(I.dialTop, 0, T(0, dy, 0), [1, 1, 1]); put(I.dialBody, 0, T(0, dy, 0), [0.02, 0.03, 0.06]);
      for (let h = 0; h < 12; h++) {
        const a = (h / 12) * TAU - PI / 2; // 12 o'clock at −z
        if (h === 0) { put(I.indices, 0, chain(T(Math.cos(a) * 124 - 5.5, dy + 1.7, Math.sin(a) * 124), spin(a), S(24, 3.4, 4.6)), STEEL); put(I.indices, 12, chain(T(Math.cos(a) * 124 + 5.5, dy + 1.7, Math.sin(a) * 124), spin(a), S(24, 3.4, 4.6)), STEEL); }
        else put(I.indices, h, chain(T(Math.cos(a) * 124, dy + 1.7, Math.sin(a) * 124), spin(a), S(h % 3 ? 20 : 25, 3.4, h % 3 ? 4.6 : 6.2)), STEEL);
      }
      // Hands at 10:10, seconds sweeping 8 steps a second (on the fourth wheel's arbor).
      const hand = (inst, y, deg, t0, extra) => { const k = handDrop(t, t0); put(inst, 0, chain(T(0, y + 500 * (1 - k), 0), spin((deg + extra * (1 - k)) * DEG)), STEEL); };
      hand(I.hh, 78.2, 305, 11.2, -70); hand(I.mh, 80.6, 60, 11.45, 80);
      hand(I.sh, 83.2, 0.75 * s + 200, 11.7, 0);
      put(I.crown, 0, chain(T(226, 46, 0), Rz(PI / 2)), STEEL); put(I.stem, 0, chain(T(208, 46, 0), Rz(PI / 2), S(5, 18, 5)), STEEL);
      [[-1, -1], [1, -1], [-1, 1], [1, 1]].forEach(([sx, sz], i) => { put(I.lugs, i, chain(T(sx * 104, 0, 0), S(1, 1, sz)), STEEL); put(I.lugCaps, i, chain(T(sx * 104, lugEnd[1], sz * lugEnd[2]), S(17.5)), STEEL); });
      put(I.floor, 0, chain(T(0, 0, 0), S(40000, 1, 40000)), [0.02, 0.02, 0.022]);

      const items = [];
      if (showMov) items.push(
        { mesh: mainplate, instances: I.plate, material: mats.perlage },
        { mesh: mainBottom, instances: I.plateB, material: { ...mats.perlage, metal: 0.12, diffuse: 0.95, specular: 0.4 } },
        { mesh: barrelBridge, instances: I.bbridge, material: mats.cotes },
        { mesh: bars, instances: I.bars, material: mats.steel },
        { mesh: cock, instances: I.cock, material: mats.steel },
        { mesh: ratchet, instances: I.ratchet, material: mats.steel },
        { mesh: crownWheel, instances: I.crownW, material: mats.steel },
        { mesh: centreWheel, instances: I.cw, material: mats.gold }, { mesh: thirdWheel, instances: I.tw, material: mats.gold }, { mesh: fourthWheel, instances: I.fw, material: mats.gold },
        { mesh: escapeWheel, instances: I.ew, material: mats.steel },
        { mesh: pinion3, instances: I.p3, material: mats.steel }, { mesh: pinion4, instances: I.p4, material: mats.steel }, { mesh: pinionE, instances: I.pe, material: mats.steel },
        { mesh: cyl, instances: I.arbors, material: mats.steel },
        { mesh: pallet, instances: I.pal, material: mats.steel },
        { mesh: cube, instances: I.stones, material: mats.ruby },
        { mesh: balanceWheel, instances: I.bal, material: mats.gold },
        { mesh: roller, instances: I.roller, material: mats.steel },
        { mesh: cyl, instances: I.balScrews, material: mats.gold },
        { mesh: hairCache.m, instances: I.hair, material: { ...mats.steel, metal: 0.7 } },
        { mesh: rotorPlate, instances: I.rotor, material: mats.cotes }, { mesh: rotorRim, instances: I.rotorRim, material: mats.gold }, { mesh: rotorHub, instances: I.rotorHub, material: mats.steel },
        { mesh: sph, instances: I.jewels, material: mats.ruby },
        { mesh: cyl, instances: I.chatons, material: mats.gold },
        { mesh: cyl, instances: I.screws, material: mats.blued },
        { mesh: cube, instances: I.slots, material: mats.slot },
      );
      items.push(
        { mesh: caseMesh, instances: I.case, material: mats.steel },
        { mesh: caseFloor, instances: I.caseFloor, material: mats.steel },
        { mesh: caseInner, instances: I.caseInner, cull: false, material: mats.steel },
        { mesh: lugMesh, instances: I.lugs, cull: false, material: mats.steel }, { mesh: sph, instances: I.lugCaps, material: mats.steel },
        { mesh: crownMesh, instances: I.crown, material: mats.steel },
        { mesh: cyl, instances: I.stem, material: mats.steel },
        { mesh: floor, instances: I.floor, mirror: true, reflect: false, material: mats.floor },
      );
      if (t > 10.4) items.push(
        { mesh: dialTop, instances: I.dialTop, material: mats.dial },
        { mesh: dialBody, instances: I.dialBody, material: mats.steel },
        { mesh: cube, instances: I.indices, material: mats.steel },
      );
      if (t > 11.0) items.push({ mesh: hourHand, instances: I.hh, material: mats.steel }, { mesh: minuteHand, instances: I.mh, material: mats.steel }, { mesh: secondHand, instances: I.sh, material: mats.steel });
      if (t > 12.1) items.push({ mesh: bezelMesh, instances: I.bezel, material: mats.steel });
      if (t > 12.5) items.push({ mesh: crystal, instances: I.crystal, blend: 'additive', material: mats.crystal });

      const [cp, tg] = camAt(t);
      Object.assign(cam.camera, { x: cp[0], y: cp[1], z: cp[2] }); Object.assign(cam.target, { x: tg[0], y: tg[1], z: tg[2] });
      const dist = Math.hypot(cp[0] - tg[0], cp[1] - tg[1], cp[2] - tg[2]);
      const macro = 1 - smooth(8.0, 11.0, t);
      const px = r3.render(cam, items, {
        sort: 'near', background: '#040405',
        key: [-0.5, 0.85, 0.45], keyColor: [1.0, 0.96, 0.9], fill: [0.7, 0.35, -0.5], fillColor: [0.07, 0.08, 0.1], rimColor: [0.5, 0.55, 0.62],
        sky: [0.6, 0.6, 0.62], horizon: [0.2, 0.2, 0.21], ground: [0.012, 0.012, 0.014], strips: [1.7, 1.66, 1.6],
        lights: [{ position: [cp[0] + 40 * Math.sin(t * 0.7), cp[1] + 30, cp[2] + 20], color: [1, 0.95, 0.88], radius: 60 + 400 * (1 - macro), intensity: 0.5 }],
        shadows: { extent: lerp(140, 300, 1 - macro), center: tg, size: 1536, strength: 0.75, bias: 0.004 },
        mirror: { y: 0, strength: 0.32 * smooth(9.0, 12.0, t) },
        dof: { focus: dist, aperture: lerp(30, 95, macro), max: lerp(14, 28, macro) },
      });
      pixels(px);
  };
  // The balance swings at full speed (up to ~110° a frame) from about 1.2 s: average sub-frames there
  // so the spokes blur like a real shutter instead of strobing.
  film.scene('hook', 0, 1.2, { draw });
  film.scene('tick', 1.2, 5.2, { draw, motionBlur: 4 });
  film.scene('glide', 5.2, 8.6, { draw });
  // The flip sweeps the studio strips across every flat bridge at once: blur it like a shutter would.
  film.scene('flip', 8.6, 10.8, { draw, motionBlur: 6 });
  film.scene('hero', 10.8, D, { draw });
  return film;
}
