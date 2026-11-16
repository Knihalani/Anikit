// A Schwarzschild black hole with a thin accretion disk, ray traced on the CPU.
//
// Lensing is precomputed: for a camera at distance D (units of M, so the horizon is
// at r = 2), every photon orbit depends only on its impact parameter, so one table
// of r(φ) per screen radius (u'' + u = 3u², RK4) serves the whole image. Per pixel
// we then solve where its orbit plane crosses the disk plane (direct image, the
// lensed far side over and under the shadow, and the photon ring), and where it
// escapes to (the starfield). Per frame only the disk is shaded: a turbulence
// texture in (log r, φ + Ω(r)·t) with Doppler beaming and redshift already baked
// into a per-pixel colour. Nothing is allocated per frame.

const TAU = Math.PI * 2;
const RIN = 6, ROUT = 28; // ISCO (3 Rs) to the outer edge, in M
const NR = 256, NAZ = 1024, AZM = NAZ - 1; // disk texture: log-radius × azimuth
const BC = 3 * Math.sqrt(3); // critical impact parameter
const H = 0.012, PHIMAX = 3 * Math.PI + 0.6, NP = Math.ceil(PHIMAX / H) + 2;
const STRIDE = 15; // star rgb, then two disk hits × (texRow, az, rate, r, g, b)

// ---------------------------------------------------------------------------
// Small deterministic noise (the texture is built once, in about 10 ms)
// ---------------------------------------------------------------------------
const hash3 = (x, y, z) => {
  let h = Math.imul(x, 374761393) ^ Math.imul(y, 668265263) ^ Math.imul(z, 1274126177);
  h = Math.imul(h ^ (h >>> 13), 1103515245);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
};
const vnoise = (x, y, z) => {
  const xi = Math.floor(x), yi = Math.floor(y), zi = Math.floor(z);
  let xf = x - xi, yf = y - yi, zf = z - zi;
  xf = xf * xf * (3 - 2 * xf); yf = yf * yf * (3 - 2 * yf); zf = zf * zf * (3 - 2 * zf);
  const l = (a, b, t) => a + (b - a) * t;
  return l(
    l(l(hash3(xi, yi, zi), hash3(xi + 1, yi, zi), xf), l(hash3(xi, yi + 1, zi), hash3(xi + 1, yi + 1, zi), xf), yf),
    l(l(hash3(xi, yi, zi + 1), hash3(xi + 1, yi, zi + 1), xf), l(hash3(xi, yi + 1, zi + 1), hash3(xi + 1, yi + 1, zi + 1), xf), yf),
    zf,
  );
};

/** Disk turbulence: streaks stretched along the orbit, periodic in azimuth. */
function diskTexture() {
  const tex = new Float32Array(NR * NAZ);
  const cs = new Float32Array(NAZ), sn = new Float32Array(NAZ);
  for (let a = 0; a < NAZ; a++) { cs[a] = Math.cos((a / NAZ) * TAU); sn[a] = Math.sin((a / NAZ) * TAU); }
  for (let i = 0; i < NR; i++) {
    const lr = i / (NR - 1); // 0 inner .. 1 outer (log radius)
    for (let a = 0; a < NAZ; a++) {
      const c = cs[a], s = sn[a];
      // Long arcs (few cells around the orbit, many across it), then finer filaments.
      const n1 = vnoise(c * 4.5 + 11, s * 4.5 + 7, lr * 18);
      const n2 = vnoise(c * 13 + 3, s * 13 + 5, lr * 46 + 9);
      const n3 = vnoise(c * 36 + 1, s * 36 + 2, lr * 120 + 4);
      let v = 0.6 * n1 + 0.27 * n2 + 0.13 * n3;
      v = (v - 0.22) * 1.7;
      v = v < 0.12 ? 0.12 : v;
      // Faint ring structure across the radius.
      v *= 0.92 + 0.08 * Math.sin(lr * 140 + n1 * 3);
      tex[i * NAZ + a] = v;
    }
  }
  return tex;
}

// ---------------------------------------------------------------------------
// Colour
// ---------------------------------------------------------------------------
/** Approximate blackbody chromaticity (linear rgb, max channel 1) for T in kelvin. */
function blackbody(T) {
  const t = T / 100;
  let r, g, b;
  if (t <= 66) { r = 255; g = 99.47 * Math.log(t) - 161.12; b = t <= 19 ? 0 : 138.52 * Math.log(t - 10) - 305.04; }
  else { r = 329.7 * Math.pow(t - 60, -0.1332); g = 288.12 * Math.pow(t - 60, -0.0755); b = 255; }
  const c = (v) => { v = Math.min(255, Math.max(0, v)) / 255; return v * v * 0.6 + v * 0.4; }; // to linear-ish
  r = c(r); g = c(g); b = c(b);
  const m = Math.max(r, g, b);
  return [(r / m) ** 1.35, (g / m) ** 1.35, (b / m) ** 1.35];
}
const BB_N = 256, BB_MIN = 1200, BB_MAX = 24000;
const BB = new Float32Array(BB_N * 3);
for (let i = 0; i < BB_N; i++) { const c = blackbody(BB_MIN + ((BB_MAX - BB_MIN) * i) / (BB_N - 1)); BB.set(c, i * 3); }

// ---------------------------------------------------------------------------
// Stars: procedural on the sky sphere, so lensed arcs come out of the same lookup
// ---------------------------------------------------------------------------
const CELL = 0.006; // star cells: a 3D lattice around the unit sky sphere
function starAt(dx, dy, dz, sig, out) {
  out[0] = 0; out[1] = 0; out[2] = 0;
  const cx = Math.floor(dx / CELL), cy = Math.floor(dy / CELL), cz = Math.floor(dz / CELL);
  if (hash3(cx, cy, cz) > 0.06) return;
  // The star sits near the middle of its cell, projected onto the sphere.
  let sx = cx + 0.4 + 0.2 * hash3(cx, cy, cz + 7), sy = cy + 0.4 + 0.2 * hash3(cx + 3, cy, cz), sz = cz + 0.4 + 0.2 * hash3(cx, cy + 5, cz);
  const l = Math.hypot(sx, sy, sz);
  sx /= l; sy /= l; sz /= l;
  const d2 = ((dx - sx) ** 2 + (dy - sy) ** 2 + (dz - sz) ** 2) / (CELL * CELL), s2 = sig * sig;
  if (d2 > 9 * s2) return;
  // Magnitudes: a steep power law, so most stars are faint and a few are bright.
  const h = hash3(cy, cz, cx);
  const m = Math.pow(h, 8) * 26 + 0.12;
  const k = m * Math.exp(-d2 / (2 * s2)) * 0.7;
  let ti = ((2800 + Math.pow(hash3(cz, cx, cy), 1.4) * 12000 - BB_MIN) / (BB_MAX - BB_MIN)) * (BB_N - 1);
  ti = Math.min(BB_N - 1, Math.max(0, ti)) | 0;
  out[0] += BB[ti * 3] * k; out[1] += BB[ti * 3 + 1] * k; out[2] += BB[ti * 3 + 2] * k;
}

// ---------------------------------------------------------------------------
// The lens
// ---------------------------------------------------------------------------
/**
 * One camera band: a radial orbit table and a per-pixel map, built in slices.
 * `build()` returns a generator; each `next()` does a few milliseconds of work.
 */
export class Lens {
  constructor(w, h, vfov) {
    this.w = w; this.h = h;
    this.F = h / 2 / Math.tan((vfov * Math.PI) / 360);
    const rhoMax = Math.hypot(w / 2, h / 2) / this.F;
    this.NA = Math.ceil(Math.hypot(w / 2, h / 2) * 1.6) + 2;
    this.drho = (rhoMax * 1.01) / (this.NA - 1);
    this.rows = new Float32Array(this.NA * NP);
    this.end = new Float32Array(this.NA);
    this.fate = new Uint8Array(this.NA); // 0 escaped, 1 captured
    this.px = new Float32Array(w * h * STRIDE);
    this.D = 0;
  }

  *build(D, elev, spin = 1) {
    this.D = D;
    const { NA, drho, rows, end, fate } = this;
    const lapse = Math.sqrt(Math.max(1e-4, 1 - 2 / D));
    // 1. Orbits r(φ) for each screen radius.
    let t0 = performance.now();
    for (let i = 0; i < NA; i++) {
      const al = Math.atan(i * drho), base = i * NP;
      const b = (D * Math.sin(al)) / lapse;
      let u = 1 / D;
      if (b < 1e-6) { fate[i] = 1; end[i] = 0; rows.fill(0, base, base + NP); continue; }
      let v = Math.sqrt(Math.max(0, 1 / (b * b) - u * u * (1 - 2 * u)));
      let k = 0, f = 2;
      rows[base] = D;
      for (k = 1; k < NP; k++) {
        // RK4 for u'' = -u + 3u²
        const a1 = v, b1 = -u + 3 * u * u;
        const u2 = u + 0.5 * H * a1, v2 = v + 0.5 * H * b1, b2 = -u2 + 3 * u2 * u2;
        const u3 = u + 0.5 * H * v2, v3 = v + 0.5 * H * b2, b3 = -u3 + 3 * u3 * u3;
        const u4 = u + H * v3, v4 = v + H * b3, b4 = -u4 + 3 * u4 * u4;
        const un = u + (H / 6) * (a1 + 2 * v2 + 2 * v3 + v4);
        v += (H / 6) * (b1 + 2 * b2 + 2 * b3 + b4);
        if (un >= 0.5) { f = 1; end[i] = (k - 1 + (0.5 - u) / (un - u)) * H; break; }
        if (un <= 0) { f = 0; end[i] = (k - 1 + u / (u - un)) * H; break; }
        u = un;
        rows[base + k] = 1 / u;
      }
      if (f === 2) { f = 1; end[i] = PHIMAX; } // still orbiting: counts as captured
      fate[i] = f;
      for (let j = k; j < NP; j++) rows[base + j] = 0;
      if (performance.now() - t0 > 6) { yield; t0 = performance.now(); }
    }
    // 2. Per pixel: disk crossings and escape directions.
    const { w, h, F, px } = this;
    const se = Math.sin(elev), ce = Math.cos(elev);
    const star = [0, 0, 0];
    const lnIn = Math.log(RIN), lnSpan = Math.log(ROUT) - lnIn;
    const Tin = 8200;
    const rate0 = NAZ / 7; // texels per second at the inner edge
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const o = (y * w + x) * STRIDE;
        const X = (x + 0.5 - w / 2) / F, Y = -(y + 0.5 - h / 2) / F;
        let rho = Math.hypot(X, Y);
        if (rho < 1e-7) rho = 1e-7;
        const nx = X / rho, ny = (Y * ce) / rho, nz = (-Y * se) / rho; // in-plane direction away from the hole
        const fi = rho / drho, i = Math.min(NA - 1, Math.round(fi)), base = i * NP, phEnd = end[i];
        const b = (D * Math.sin(Math.atan(rho))) / lapse;
        const Lz = -b * ce * (X / rho) * spin; // photon angular momentum about the disk axis (emitted towards us)
        // Crossings of the plane y = 0: se·cosφ + (Y·ce/ρ)·sinφ = 0.
        const c = (Y * ce) / rho;
        let ph0 = Math.atan2(-se, c);
        if (ph0 <= 0) ph0 += Math.PI;
        let hits = 0, trans = 1;
        px[o + 3] = -1; px[o + 9] = -1;
        for (let kk = 0; kk < 3 && hits < 2; kk++) {
          const ph = ph0 + kk * Math.PI;
          if (ph >= phEnd) break;
          const q = ph / H, qi = q | 0, qf = q - qi;
          const ra = rows[base + qi], rb = rows[base + qi + 1];
          if (!(ra > 0 && rb > 0)) break;
          const r = ra + (rb - ra) * qf;
          if (r < RIN * 0.94 || r > ROUT) continue;
          const cp = Math.cos(ph), sp = Math.sin(ph);
          const Px = sp * nx, Pz = cp * ce + sp * nz;
          let az = Math.atan2(Pz, Px) / TAU; if (az < 0) az += 1;
          const Om = Math.pow(r, -1.5) * spin;
          const g = Math.sqrt(Math.max(0, 1 - 3 / r)) / (1 - Om * Lz);
          // Thin-disk profile: T ∝ r^-3/4, soft inner edge at the ISCO, fading outer edge.
          const sIn = Math.min(1, Math.max(0, (r - RIN * 0.94) / (RIN * 0.22)));
          const sOut = Math.min(1, Math.max(0, (ROUT - r) / (ROUT * 0.45)));
          const T = Tin * Math.pow(RIN / r, 0.75) * g;
          const I = Math.pow(RIN / r, 2.4) * Math.pow(g, 3.6) * sIn * sIn * sOut * 1.1 * trans;
          let ti = ((T - BB_MIN) / (BB_MAX - BB_MIN)) * (BB_N - 1);
          ti = ti < 0 ? 0 : ti > BB_N - 1 ? BB_N - 1 : ti | 0;
          const lr = (Math.log(r) - lnIn) / lnSpan;
          const ri = Math.min(NR - 1, Math.max(0, Math.round(lr * (NR - 1))));
          const so = o + 3 + hits * 6;
          px[so] = ri * NAZ;
          px[so + 1] = az * NAZ;
          px[so + 2] = rate0 * Math.pow(RIN / r, 1.5) * spin;
          px[so + 3] = BB[ti * 3] * I; px[so + 4] = BB[ti * 3 + 1] * I; px[so + 5] = BB[ti * 3 + 2] * I;
          hits++;
          trans *= 0.12; // the disk is optically thick: later images show through faintly
        }
        // Escape direction, or the shadow.
        if (fate[i] === 0) {
          const cp = Math.cos(phEnd), sp = Math.sin(phEnd);
          const dx = sp * nx, dy = cp * se + sp * (ny), dz = cp * ce + sp * nz;
          // Star size follows the local magnification: sigma in cells per screen pixel.
          starAt(dx, dy, dz, Math.min(0.22, Math.max(0.06, 0.5 / F / CELL)), star);
          px[o] = star[0] * trans; px[o + 1] = star[1] * trans; px[o + 2] = star[2] * trans;
        } else { px[o] = 0; px[o + 1] = 0; px[o + 2] = 0; }
      }
      if (performance.now() - t0 > 5) { yield; t0 = performance.now(); }
    }
  }
}

/**
 * Shade one frame into `out` (Uint32 RGBA). `t` is disk time in seconds;
 * `exposure`, star tint and disk gain come from the scroll choreography.
 */
export function shade(lens, tex, out, t, exposure, sr, sg, sb, disk) {
  const px = lens.px, n = lens.w * lens.h;
  for (let i = 0, o = 0; i < n; i++, o += STRIDE) {
    let r = px[o] * sr, g = px[o + 1] * sg, b = px[o + 2] * sb;
    const h0 = px[o + 3];
    if (h0 < 0 && r + g + b < 0.002) { out[i] = 0xff000000; continue; }
    if (h0 >= 0) {
      let v = tex[h0 + ((px[o + 4] + px[o + 5] * t) & AZM)] * disk;
      r += px[o + 6] * v; g += px[o + 7] * v; b += px[o + 8] * v;
      const h1 = px[o + 9];
      if (h1 >= 0) {
        v = tex[h1 + ((px[o + 10] + px[o + 11] * t) & AZM)] * disk;
        r += px[o + 12] * v; g += px[o + 13] * v; b += px[o + 14] * v;
      }
    }
    // Filmic tone map (ACES fit), applied per channel.
    r *= exposure; g *= exposure; b *= exposure;
    r = (r * (2.51 * r + 0.03)) / (r * (2.43 * r + 0.59) + 0.14);
    g = (g * (2.51 * g + 0.03)) / (g * (2.43 * g + 0.59) + 0.14);
    b = (b * (2.51 * b + 0.03)) / (b * (2.43 * b + 0.59) + 0.14);
    // Approximate sRGB encode: sqrt is close and cheap.
    const R = Math.sqrt(r > 1 ? 1 : r) * 255, G = Math.sqrt(g > 1 ? 1 : g) * 255, B = Math.sqrt(b > 1 ? 1 : b) * 255;
    out[i] = 0xff000000 | (B << 16) | (G << 8) | R;
  }
}

export const shadowScale = (D) => { const s = Math.min(0.999, (BC * Math.sqrt(Math.max(1e-4, 1 - 2 / D))) / D); return s / Math.sqrt(1 - s * s); };
export { diskTexture };
