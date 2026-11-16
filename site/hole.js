// A Schwarzschild black hole with a thin accretion disk, ray traced on the CPU, exact
// for the current camera distance on every frame (no bands, no zoom tricks).
//
// Every photon orbit depends only on its impact parameter, so each frame integrates
// one table of r(φ) per screen radius (u'' + u = 3u², RK4, a few hundred rows) for the
// current distance. Per-pixel geometry that does not depend on the distance (where
// the pixel's orbit plane crosses the disk plane, and the disk azimuth there) is
// computed once per resize. Shading reads the tables: disk hits with Doppler beaming,
// redshift and blackbody colour, a turbulence texture sheared by Keplerian rotation,
// a lensed star catalogue splatted as tiny points, drifting dust, and, past the
// horizon, a tunnel of streaming light. Nothing is allocated per frame.
//
// API (the same shape as anikit's core `blackHole()`, so either can drive the page):
//   const bh = blackHole({ width, height, canvas });
//   bh.render({ distance, inclination, fov, time, exposure, bloom, grain });
//   bh.resize(w, h); bh.destroy();

const TAU = Math.PI * 2;
const RIN = 6, ROUT = 30; // ISCO (3 Rs) to the outer edge, in M
const NR = 256, NAZ = 1024, AZM = NAZ - 1; // disk texture: log-radius × azimuth
const BC = 3 * Math.sqrt(3); // critical impact parameter
const H = 0.03, PHMAX = TAU + 0.5, NP = Math.ceil(PHMAX / H) + 2; // orbit samples per row
const NA = 320; // orbit rows (screen radii)

// ---------------------------------------------------------------------------
// Noise and textures (built once)
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

/** Disk turbulence: streaks stretched along the orbit, periodic in azimuth, plus hot spots. */
function diskTexture() {
  const tex = new Float32Array(NR * NAZ);
  const cs = new Float32Array(NAZ), sn = new Float32Array(NAZ);
  for (let a = 0; a < NAZ; a++) { cs[a] = Math.cos((a / NAZ) * TAU); sn[a] = Math.sin((a / NAZ) * TAU); }
  // A few compact hot spots, orbiting with the gas.
  const spots = [];
  for (let k = 0; k < 9; k++) spots.push([hash3(k, 1, 2) * NR * 0.7 + NR * 0.04, hash3(k, 3, 4) * NAZ, 3 + hash3(k, 5, 6) * 5, 40 + hash3(k, 7, 8) * 90, 0.8 + hash3(k, 9, 1) * 1.4]);
  for (let i = 0; i < NR; i++) {
    const lr = i / (NR - 1);
    for (let a = 0; a < NAZ; a++) {
      const c = cs[a], s = sn[a];
      const n1 = vnoise(c * 4.5 + 11, s * 4.5 + 7, lr * 16);
      const n2 = vnoise(c * 12 + 3, s * 12 + 5, lr * 40 + 9);
      const n3 = vnoise(c * 26 + 1, s * 26 + 2, lr * 80 + 4);
      let v = 0.6 * n1 + 0.28 * n2 + 0.12 * n3;
      v = (v - 0.2) * 1.6;
      v = v < 0.14 ? 0.14 : v;
      for (const [si, sa, sr, sw, sb] of spots) {
        let da = Math.abs(a - sa); if (da > NAZ / 2) da = NAZ - da;
        const q = ((i - si) / sr) ** 2 + (da / sw) ** 2;
        if (q < 9) v += sb * Math.exp(-q);
      }
      tex[i * NAZ + a] = v;
    }
  }
  return tex;
}

/** Approximate blackbody chromaticity (linear rgb, max channel 1) for T in kelvin. */
function blackbody(T) {
  const t = T / 100;
  let r, g, b;
  if (t <= 66) { r = 255; g = 99.47 * Math.log(t) - 161.12; b = t <= 19 ? 0 : 138.52 * Math.log(t - 10) - 305.04; }
  else { r = 329.7 * Math.pow(t - 60, -0.1332); g = 288.12 * Math.pow(t - 60, -0.0755); b = 255; }
  const c = (v) => { v = Math.min(255, Math.max(0, v)) / 255; return v * v * 0.6 + v * 0.4; };
  r = c(r); g = c(g); b = c(b);
  const m = Math.max(r, g, b);
  return [(r / m) ** 1.7, (g / m) ** 1.7, (b / m) ** 1.7];
}
const BB_N = 512, BB_MIN = 1200, BB_MAX = 30000, BBS = (BB_N - 1) / (BB_MAX - BB_MIN);
const BB = new Float32Array(BB_N * 3);
for (let i = 0; i < BB_N; i++) BB.set(blackbody(BB_MIN + i / BBS), i * 3);

// Radial lookup tables over r ∈ [RMIN, ROUT]: emissivity, temperature, Ω, redshift,
// texture row, rotation rate and opacity.
const RMIN = RIN * 0.92, RL = 2048, RLS = (RL - 1) / (ROUT - RMIN);
const L_E = new Float32Array(RL), L_T = new Float32Array(RL), L_OM = new Float32Array(RL), L_SQ = new Float32Array(RL);
const L_TR = new Float32Array(RL), L_RT = new Float32Array(RL), L_OP = new Float32Array(RL);
{
  const lnIn = Math.log(RMIN), lnSpan = Math.log(ROUT) - lnIn;
  for (let i = 0; i < RL; i++) {
    const r = RMIN + i / RLS;
    const sIn = Math.min(1, Math.max(0, (r - RMIN) / (RIN * 0.28)));
    const sOut = Math.min(1, Math.max(0, (ROUT - r) / (ROUT * 0.5)));
    const soft = sIn * sIn * (3 - 2 * sIn);
    L_E[i] = Math.pow(RIN / r, 2.4) * soft * sOut * sOut * 1.15;
    L_T[i] = 9000 * Math.pow(RIN / r, 0.75);
    L_OM[i] = Math.pow(r, -1.5);
    L_SQ[i] = Math.sqrt(Math.max(0, 1 - 3 / r));
    L_TR[i] = ((Math.log(r) - lnIn) / lnSpan) * (NR - 1.001);
    L_RT[i] = (NAZ / 6.5) * Math.pow(RIN / r, 1.5); // texels per second: inner edge turns in ~6.5 s
    L_OP[i] = 0.9 * soft * sOut;
  }
}
const GL = 2048, GMAX = 3, GLS = (GL - 1) / GMAX;
const L_G = new Float32Array(GL);
for (let i = 0; i < GL; i++) L_G[i] = Math.pow(i / GLS, 3.8);

// Tone curve: linear radiance → 0..255 (ACES fit, then a gamma ~ 1/2 encode).
const TSTEP = 2048, TMAX = TSTEP * 4 - 1;
const TONE = new Float32Array(TMAX + 1);
for (let i = 0; i <= TMAX; i++) { const x = i / TSTEP; const y = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14); TONE[i] = Math.sqrt(Math.min(1, y)) * 255; }
const GRAIN = new Float32Array(8192);
for (let i = 0; i < GRAIN.length; i++) GRAIN[i] = (Math.random() + Math.random() - 1);

// Star catalogue: direction, brightness (power law), colour index, twinkle phase.
const NSTAR = 5000;
const STARS = new Float32Array(NSTAR * 6);
{
  let seed = 12345;
  const rnd = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296);
  for (let i = 0; i < NSTAR; i++) {
    const u = rnd() * 2 - 1, ph = rnd() * TAU, q = Math.sqrt(1 - u * u);
    STARS[i * 6] = Math.cos(ph) * q; STARS[i * 6 + 1] = u; STARS[i * 6 + 2] = Math.sin(ph) * q;
    STARS[i * 6 + 3] = 0.04 + Math.pow(rnd(), 10) * 3;
    STARS[i * 6 + 4] = Math.min(BB_N - 1, Math.max(0, Math.round((2900 + Math.pow(rnd(), 1.4) * 12000 - BB_MIN) * BBS))) * 3;
    STARS[i * 6 + 5] = rnd() * TAU;
  }
}
// Dust motes near the camera (camera space), for parallax as you fall.
const NDUST = 260;
const DUST = new Float32Array(NDUST * 4);
{
  let seed = 777;
  const rnd = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296);
  for (let i = 0; i < NDUST; i++) { DUST[i * 4] = rnd() * 2 - 1; DUST[i * 4 + 1] = rnd() * 2 - 1; DUST[i * 4 + 2] = rnd(); DUST[i * 4 + 3] = 0.3 + rnd() * 0.7; }
}
// Streaks for the tunnel past the horizon: angle, depth phase, speed, brightness.
const NSTREAK = 420;
const STREAK = new Float32Array(NSTREAK * 4);
{
  let seed = 4242;
  const rnd = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296);
  for (let i = 0; i < NSTREAK; i++) { STREAK[i * 4] = rnd() * TAU; STREAK[i * 4 + 1] = rnd(); STREAK[i * 4 + 2] = 0.5 + rnd(); STREAK[i * 4 + 3] = Math.pow(rnd(), 3); }
}

let TEX = null;

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------
export function blackHole({ width, height, canvas = null }) {
  TEX ??= diskTexture();
  const tex = TEX;
  let w = 0, h = 0, n = 0;
  let rpx, ux, uy, ph0, az0, az1, theta, hit, img, out, ctx = null, bloomCv = null, bctx = null;
  const rows = new Float32Array(NA * NP), end = new Float32Array(NA), brow = new Float32Array(NA), fate = new Uint8Array(NA);
  let geoIncl = NaN, tabD = NaN, tabF = NaN, drho = 1, ic = 0, se = 0, ce = 1;

  const resize = (W, Hh) => {
    w = Math.max(16, W | 0); h = Math.max(16, Hh | 0); n = w * h;
    rpx = new Float32Array(n); ux = new Float32Array(n); uy = new Float32Array(n);
    ph0 = new Float32Array(n); az0 = new Float32Array(n); az1 = new Float32Array(n); theta = new Float32Array(n);
    hit = new Uint8Array(n);
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      const i = y * w + x, X = x + 0.5 - w / 2, Y = -(y + 0.5 - h / 2), r = Math.hypot(X, Y) || 1e-3;
      rpx[i] = r; ux[i] = X / r; uy[i] = Y / r;
      let th = Math.atan2(Y, X) / TAU; if (th < 0) th += 1;
      theta[i] = th * NAZ;
    }
    if (canvas) {
      canvas.width = w; canvas.height = h;
      ctx = canvas.getContext('2d', { alpha: false });
      img = ctx.createImageData(w, h);
      bloomCv = typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(Math.max(16, w >> 2), Math.max(16, h >> 2)) : Object.assign(document.createElement('canvas'), { width: Math.max(16, w >> 2), height: Math.max(16, h >> 2) });
      bctx = bloomCv.getContext('2d', { alpha: false });
    } else img = { data: new Uint8ClampedArray(n * 4) };
    out = new Uint32Array(img.data.buffer);
    geoIncl = NaN; tabD = NaN;
  };

  /** Disk-plane crossings for each pixel (depend only on the inclination). */
  const geometry = (incl) => {
    const elev = Math.PI / 2 - incl;
    se = Math.sin(elev); ce = Math.cos(elev);
    for (let i = 0; i < n; i++) {
      const c = uy[i] * ce;
      let p = Math.atan2(-se, c);
      if (p <= 0) p += Math.PI;
      ph0[i] = p;
      const nx = ux[i], nz = -uy[i] * se;
      for (let k = 0; k < 2; k++) {
        const q = p + k * Math.PI, cp = Math.cos(q), sp = Math.sin(q);
        let a = Math.atan2(cp * ce + sp * nz, sp * nx) / TAU; if (a < 0) a += 1;
        (k ? az1 : az0)[i] = a * NAZ;
      }
    }
    geoIncl = incl;
  };

  /** Orbit table r(φ) per screen radius for a camera at distance D (M). */
  const table = (D, F) => {
    const lapse = Math.sqrt(Math.max(1e-4, 1 - 2 / D));
    const rhoMax = (Math.hypot(w / 2, h / 2) / F) * 1.02;
    drho = rhoMax / (NA - 1);
    ic = NA;
    for (let i = 0; i < NA; i++) {
      const al = Math.atan(i * drho), base = i * NP;
      const b = (D * Math.sin(al)) / lapse;
      brow[i] = b;
      let u = 1 / D;
      rows[base] = D;
      if (b < 1e-6) { fate[i] = 1; end[i] = 0; for (let j = 1; j < NP; j++) rows[base + j] = 0; continue; }
      let v = Math.sqrt(Math.max(0, 1 / (b * b) - u * u * (1 - 2 * u)));
      let k = 1, f = 2;
      for (; k < NP; k++) {
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
      if (f === 2) { f = 1; end[i] = PHMAX; }
      fate[i] = f;
      if (f === 0 && i < ic) ic = i;
      for (let j = k; j < NP; j++) rows[base + j] = 0;
    }
    tabD = D; tabF = F;
  };

  /** Angle from the camera axis → screen radius of the primary image (−1 if off screen). */
  const imageRadius = (th) => {
    if (ic >= NA - 1 || th > end[ic] || th < end[NA - 1]) return -1;
    let lo = ic, hi = NA - 1;
    while (hi - lo > 1) { const m = (lo + hi) >> 1; if (end[m] > th) lo = m; else hi = m; }
    return (lo + (end[lo] - th) / (end[lo] - end[hi] || 1)) * drho;
  };

  // Additive splat of a small round point (bilinear), skipped where the disk glows in front.
  const splat = (x, y, cr, cg, cb) => {
    if (x < 0 || y < 0 || x >= w - 1 || y >= h - 1) return;
    const xi = x | 0, yi = y | 0, fx = x - xi, fy = y - yi;
    for (let q = 0; q < 4; q++) {
      const pi = (yi + (q >> 1)) * w + xi + (q & 1);
      if (hit[pi]) continue;
      const wq = ((q & 1) ? fx : 1 - fx) * ((q >> 1) ? fy : 1 - fy);
      const v = out[pi];
      let R = (v & 255) + cr * wq, G = ((v >> 8) & 255) + cg * wq, B = ((v >> 16) & 255) + cb * wq;
      R = R > 255 ? 255 : R; G = G > 255 ? 255 : G; B = B > 255 ? 255 : B;
      out[pi] = 0xff000000 | (B << 16) | (G << 8) | R;
    }
  };

  let frameNo = 0;
  const render = ({ distance = 40, inclination = 84, fov = 40, time = 0, exposure = 2.2, bloom = 0.6, grain = 4, spin = 1, tint = null, stream = 0 } = {}) => {
    frameNo++;
    const F = h / 2 / Math.tan((fov * Math.PI) / 360);
    const go = (frameNo * 2971) & 4095;
    const sr = tint ? tint[0] : 1, sg = tint ? tint[1] : 1, sb = tint ? tint[2] : 1;
    const ks = exposure * TSTEP;
    if (distance > 2.02) {
      const incl = (inclination * Math.PI) / 180;
      if (incl !== geoIncl) geometry(incl);
      if (distance !== tabD || F !== tabF) table(distance, F);
      const invF = 1 / F, invDr = 1 / drho;
      for (let i = 0; i < n; i++) {
        hit[i] = 0;
        const fi = rpx[i] * invF * invDr;
        let ri = fi | 0;
        if (ri > NA - 2) { out[i] = 0xff000000; continue; }
        const fr = fi - ri, rb = ri * NP;
        const eEnd = fr < 0.5 ? end[ri] : end[ri + 1];
        let phi = ph0[i];
        if (phi >= eEnd) { out[i] = 0xff000000; continue; }
        const b = brow[ri] + (brow[ri + 1] - brow[ri]) * fr;
        const Lz = -b * ce * ux[i] * spin;
        let r = 0, g = 0, bl = 0, trans = 1;
        for (let k = 0; k < 2; k++) {
          if (k) { phi += Math.PI; if (phi >= eEnd) break; }
          const q = phi / H, qi = q | 0, qf = q - qi, a = rb + qi;
          let r0 = rows[a], r1 = rows[a + NP];
          const r0b = rows[a + 1], r1b = rows[a + NP + 1];
          r0 = r0 > 0 && r0b > 0 ? r0 + (r0b - r0) * qf : r0b > 0 ? r0b : r0;
          r1 = r1 > 0 && r1b > 0 ? r1 + (r1b - r1) * qf : r1b > 0 ? r1b : r1;
          const rr = r0 > 0 && r1 > 0 ? r0 + (r1 - r0) * fr : r0 > 0 ? r0 : r1;
          if (!(rr > RMIN && rr < ROUT)) continue;
          const li = ((rr - RMIN) * RLS) | 0;
          let gg = L_SQ[li] / (1 - L_OM[li] * Lz * spin);
          gg = gg < 0 ? 0 : gg > GMAX - 0.01 ? GMAX - 0.01 : gg;
          const I = L_E[li] * L_G[(gg * GLS) | 0] * trans;
          let ci = ((L_T[li] * gg - BB_MIN) * BBS) | 0;
          ci = (ci < 0 ? 0 : ci > BB_N - 1 ? BB_N - 1 : ci) * 3;
          // Texture: bilinear in (log r, φ + Ω t).
          const tr = L_TR[li], ti = tr | 0, tf = tr - ti, row = ti * NAZ;
          const az = (k ? az1[i] : az0[i]) + L_RT[li] * time * spin, ai = az | 0, af = az - ai;
          const a0 = ai & AZM, a1 = (ai + 1) & AZM;
          const v0 = tex[row + a0] + (tex[row + a1] - tex[row + a0]) * af;
          const v1 = tex[row + NAZ + a0] + (tex[row + NAZ + a1] - tex[row + NAZ + a0]) * af;
          const val = (v0 + (v1 - v0) * tf) * I;
          r += BB[ci] * val; g += BB[ci + 1] * val; bl += BB[ci + 2] * val;
          trans *= 1 - L_OP[li];
          hit[i] = 1;
        }
        if (!hit[i]) { out[i] = 0xff000000; continue; }
        const gn = GRAIN[(i + go) & 8191] * grain;
        r *= ks; g *= ks; bl *= ks;
        let R = TONE[r < TMAX ? r | 0 : TMAX] + gn, G = TONE[g < TMAX ? g | 0 : TMAX] + gn, B = TONE[bl < TMAX ? bl | 0 : TMAX] + gn;
        R = R < 0 ? 0 : R > 255 ? 255 : R; G = G < 0 ? 0 : G > 255 ? 255 : G; B = B < 0 ? 0 : B > 255 ? 255 : B;
        out[i] = 0xff000000 | (B << 16) | (G << 8) | R;
      }
      // Stars: lensed catalogue (primary and secondary images), twinkling slightly.
      const rc = ic * drho;
      for (let s = 0; s < NSTAR * 6; s += 6) {
        const sx = STARS[s], sy = STARS[s + 1], sz = STARS[s + 2];
        const c = sy * se + sz * ce;
        const th = Math.acos(c > 1 ? 1 : c < -1 ? -1 : c);
        const qx = sx, qy = sy * ce - sz * se, ql = Math.hypot(qx, qy) || 1e-9;
        const tw = 1 + 0.18 * Math.sin(time * 2.3 + STARS[s + 5] * 7) * Math.sin(time * 0.9 + STARS[s + 5]);
        const base = STARS[s + 3] * tw * exposure * 120, ci = STARS[s + 4];
        for (let im = 0; im < 2; im++) {
          const rho = imageRadius(im ? TAU - th : th);
          if (rho < 0) continue;
          const dm = Math.min(1, Math.max(0, (rho - rc) / (rc * 0.6)));
          const br = base * (im ? 0.25 : 1) * dm * dm;
          if (br < 1) continue;
          const sg2 = im ? -1 : 1;
          splat((sg2 * rho * qx / ql) * F + w / 2 - 0.5, -(sg2 * rho * qy / ql) * F + h / 2 - 0.5, BB[ci] * br * sr, BB[ci + 1] * br * sg, BB[ci + 2] * br * sb);
        }
      }
      // Dust drifting past the camera: faint warm motes with parallax as you fall.
      const travel = Math.log(distance) * 6 + time * 0.02;
      for (let d = 0; d < NDUST * 4; d += 4) {
        let z = (DUST[d + 2] - travel) % 1; if (z < 0) z += 1;
        const zz = 0.05 + z * 1.2;
        const x = (DUST[d] / zz) * h * 0.6 + w / 2, y = (DUST[d + 1] / zz) * h * 0.6 + h / 2;
        const fade = Math.min(1, z * 6) * Math.min(1, (1 - z) * 4) * DUST[d + 3] * 26;
        splat(x, y, fade, fade * 0.8, fade * 0.65);
      }
    } else {
      // Inside the horizon: a tunnel of streaming, lensed light.
      const depth = Math.max(0, 2.02 - distance); // 0 at the horizon, ~2 deep inside
      const fadeIn = Math.min(1, depth / 0.12);
      const flow = time * 0.35 + depth * 3.2 + stream;
      const halfMin = Math.min(w, h) / 2;
      const inv = 1 / halfMin;
      for (let i = 0; i < n; i++) {
        hit[i] = 0;
        const rn = rpx[i] * inv; // 0 at centre
        // Depth coordinate: rings rush outward; azimuth twists with depth.
        const zc = 0.55 / (rn + 0.08) + flow;
        let tr = (zc * 37) % (2 * (NR - 2)); if (tr > NR - 2) tr = 2 * (NR - 2) - tr;
        const ti = tr | 0, tf = tr - ti, row = ti * NAZ;
        const az = theta[i] + zc * 120 + time * 30, ai = az | 0, af = az - ai, a0 = ai & AZM, a1 = (ai + 1) & AZM;
        const v0 = tex[row + a0] + (tex[row + a1] - tex[row + a0]) * af;
        const v1 = tex[row + NAZ + a0] + (tex[row + NAZ + a1] - tex[row + NAZ + a0]) * af;
        let v = v0 + (v1 - v0) * tf;
        // Light gathers in a ring around the view; the centre stays dark and deep.
        let ring = (rn - 0.06) / 0.7; ring = ring < 0 ? 0 : ring > 1 ? 1 : ring; ring = ring * ring * (3 - 2 * ring);
        const I = v * v * v * ring * 0.16 * fadeIn;
        // Warm infalling gas at the rim, blue-shifted light towards the centre.
        const warm = ring;
        let r = I * (0.35 + 0.75 * warm), g = I * (0.42 + 0.2 * warm), bl = I * (0.75 - 0.6 * warm);
        r *= ks * sr; g *= ks * sg; bl *= ks * sb;
        const gn = GRAIN[(i + go) & 8191] * grain;
        let R = TONE[r < TMAX ? r | 0 : TMAX] + gn, G = TONE[g < TMAX ? g | 0 : TMAX] + gn, B = TONE[bl < TMAX ? bl | 0 : TMAX] + gn;
        R = R < 0 ? 0 : R > 255 ? 255 : R; G = G < 0 ? 0 : G > 255 ? 255 : G; B = B < 0 ? 0 : B > 255 ? 255 : B;
        out[i] = 0xff000000 | (B << 16) | (G << 8) | R;
      }
      // Starlight stretched into streaks, flowing outward past the camera.
      for (let s = 0; s < NSTREAK * 4; s += 4) {
        let z = (STREAK[s + 1] + flow * 0.22 * STREAK[s + 2]) % 1;
        const rad = halfMin * (0.06 + 1.6 * z * z * z);
        const len = 2 + z * z * 26;
        const ca = Math.cos(STREAK[s]), sa = Math.sin(STREAK[s]);
        const br = (20 + STREAK[s + 3] * 160) * Math.min(1, z * 5) * fadeIn * exposure * 0.5;
        for (let j = 0; j < len; j += 1) {
          const rr = rad + j, f = 1 - j / len;
          splat(w / 2 + ca * rr - 0.5, h / 2 + sa * rr - 0.5, br * f * 0.8 * sr, br * f * 0.88 * sg, br * f * sb);
        }
      }
    }
    if (ctx) {
      ctx.putImageData(img, 0, 0);
      if (bloom > 0 && 'filter' in bctx) {
        bctx.filter = 'blur(5px)';
        bctx.drawImage(canvas, 0, 0, bloomCv.width, bloomCv.height);
        ctx.globalCompositeOperation = 'screen';
        ctx.globalAlpha = bloom;
        ctx.drawImage(bloomCv, 0, 0, w, h);
        ctx.globalAlpha = 1;
        ctx.globalCompositeOperation = 'source-over';
      }
    }
    return img.data;
  };

  resize(width, height);
  return { render, resize, destroy() { rows.fill(0); }, get width() { return w; }, get height() { return h; } };
}

/** Apparent shadow radius in tan units for a camera at distance D. */
export const shadowScale = (D) => { const s = Math.min(0.999, (BC * Math.sqrt(Math.max(1e-4, 1 - 2 / D))) / D); return s / Math.sqrt(1 - s * s); };
