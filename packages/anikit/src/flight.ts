/**
 * Camera flight paths: fly a `scene3d` camera smoothly through a list of
 * points, looking ahead along the path and banking into turns — the
 * building block for "journey" shots in films and scroll-driven 3D.
 *
 * ```ts
 * const path = flightPath([[0, 0, 2000], [300, 80, 0], [-200, -60, -3000], [0, 0, -6000]], { lookAhead: 500, bank: 1 });
 * const s = path.timing([[0, 0], [2, 0.3], [5, 1]]); // seconds → distance (smooth, monotone)
 * film.scene('fly', 0, 5, { draw: ({ t }) => path.apply(cam, s(t)) });
 * ```
 */
import type { Scene3D } from './scene';

export type Point3 = [number, number, number] | { x: number; y: number; z: number };

export interface FlightPathOptions {
  /** Distance ahead along the path the camera looks at (world units, default 400). */
  lookAhead?: number;
  /** Banking strength: 0 = level, 1 = natural (default), 2 = aggressive. */
  bank?: number;
  /** Maximum bank angle in degrees (default 55). */
  maxBank?: number;
  /** Close the loop (last point joins the first). */
  closed?: boolean;
}

/** Handheld wobble for `FlightPath.apply()`. */
export interface FlightShake {
  /** Strength in world units (0 = none). */
  shake?: number;
  /** Seconds driving the noise (default: distance flown / 600, so it quickens with speed). */
  time?: number;
  /** Speed multiplier for the wobble (default 1). */
  rate?: number;
}

export interface FlightPose {
  /** Camera position on the path. */
  eye: [number, number, number];
  /** Point the camera looks at, `lookAhead` further along. */
  target: [number, number, number];
  /** Bank angle in degrees (apply as the camera's `roll`). */
  roll: number;
  /** Unit direction of travel. */
  forward: [number, number, number];
}

const v3 = (p: Point3): [number, number, number] => (Array.isArray(p) ? [p[0], p[1], p[2]] : [p.x, p.y, p.z]);

export class FlightPath {
  /** Total arc length in world units. */
  readonly length: number;
  private pts: [number, number, number][];
  private cum: Float64Array; // arc length at each table sample
  private tab: Float64Array; // xyz per sample
  private o: Required<FlightPathOptions>;

  constructor(points: Point3[], o: FlightPathOptions = {}) {
    if (points.length < 2) throw new Error('anikit: flightPath needs at least 2 points');
    this.o = { lookAhead: o.lookAhead ?? 400, bank: o.bank ?? 1, maxBank: o.maxBank ?? 55, closed: o.closed ?? false };
    this.pts = points.map(v3);
    const segs = this.o.closed ? this.pts.length : this.pts.length - 1;
    const per = 48;
    const n = segs * per + 1;
    this.tab = new Float64Array(n * 3);
    this.cum = new Float64Array(n);
    for (let i = 0; i < n; i++) {
      const p = this.raw(i / per);
      this.tab.set(p, i * 3);
      if (i) {
        const dx = p[0] - this.tab[i * 3 - 3], dy = p[1] - this.tab[i * 3 - 2], dz = p[2] - this.tab[i * 3 - 1];
        this.cum[i] = this.cum[i - 1] + Math.hypot(dx, dy, dz);
      }
    }
    this.length = this.cum[n - 1];
  }

  /** Centripetal Catmull–Rom at parameter u ∈ [0, segments] (no cusps or overshoot loops). */
  private raw(u: number): [number, number, number] {
    const P = this.pts, m = P.length, closed = this.o.closed;
    const segs = closed ? m : m - 1;
    const i = Math.min(segs - 1, Math.max(0, Math.floor(u)));
    const t = u - i;
    const get = (k: number) => (closed ? P[((k % m) + m) % m] : k < 0 ? mirror(P[0], P[1]) : k >= m ? mirror(P[m - 1], P[m - 2]) : P[k]);
    const p0 = get(i - 1), p1 = get(i), p2 = get(i + 1), p3 = get(i + 2);
    const d = (a: number[], b: number[]) => Math.max(1e-6, Math.sqrt(Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2])));
    const t0 = 0, t1 = t0 + d(p0, p1), t2 = t1 + d(p1, p2), t3 = t2 + d(p2, p3);
    const tt = t1 + (t2 - t1) * t;
    const out: [number, number, number] = [0, 0, 0];
    for (let k = 0; k < 3; k++) {
      const a1 = ((t1 - tt) / (t1 - t0)) * p0[k] + ((tt - t0) / (t1 - t0)) * p1[k];
      const a2 = ((t2 - tt) / (t2 - t1)) * p1[k] + ((tt - t1) / (t2 - t1)) * p2[k];
      const a3 = ((t3 - tt) / (t3 - t2)) * p2[k] + ((tt - t2) / (t3 - t2)) * p3[k];
      const b1 = ((t2 - tt) / (t2 - t0)) * a1 + ((tt - t0) / (t2 - t0)) * a2;
      const b2 = ((t3 - tt) / (t3 - t1)) * a2 + ((tt - t1) / (t3 - t1)) * a3;
      out[k] = ((t2 - tt) / (t2 - t1)) * b1 + ((tt - t1) / (t2 - t1)) * b2;
    }
    return out;
  }

  /** Position at arc length `s` (clamped; wraps when closed). */
  point(s: number): [number, number, number] {
    const L = this.length;
    s = this.o.closed ? ((s % L) + L) % L : Math.min(L, Math.max(0, s));
    const c = this.cum;
    let lo = 0, hi = c.length - 1;
    while (hi - lo > 1) {
      const mid = (lo + hi) >> 1;
      if (c[mid] <= s) lo = mid; else hi = mid;
    }
    const f = (s - c[lo]) / Math.max(1e-9, c[hi] - c[lo]);
    const T = this.tab;
    return [T[lo * 3] + (T[hi * 3] - T[lo * 3]) * f, T[lo * 3 + 1] + (T[hi * 3 + 1] - T[lo * 3 + 1]) * f, T[lo * 3 + 2] + (T[hi * 3 + 2] - T[lo * 3 + 2]) * f];
  }

  /** `n` points evenly spaced by arc length from `from` to `to` (defaults: the whole path) — e.g. for `mesh.tube()` light trails. */
  points(n: number, from = 0, to = this.length): [number, number, number][] {
    return Array.from({ length: Math.max(2, n) }, (_, i) => this.point(from + ((to - from) * i) / Math.max(1, n - 1)));
  }

  /** Unit tangent at arc length `s`. */
  tangent(s: number): [number, number, number] {
    const h = Math.max(1, this.length * 1e-4);
    const a = this.point(s - h), b = this.point(s + h);
    const d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    const l = Math.hypot(d[0], d[1], d[2]) || 1;
    return [d[0] / l, d[1] / l, d[2] / l];
  }

  /** Camera pose at arc length `s`: position, look-at target ahead, and bank. */
  at(s: number, o: {
    /** Look-at distance ahead along the path (default: the path's `lookAhead`). */
    lookAhead?: number;
    /** Banking strength (default: the path's `bank`). */
    bank?: number;
  } = {}): FlightPose {
    const ahead = o.lookAhead ?? this.o.lookAhead;
    const eye = this.point(s);
    const end = this.o.closed ? Infinity : this.length;
    let target = this.point(Math.min(end, s + ahead));
    const fwd = this.tangent(s);
    if (!this.o.closed && s + ahead > this.length) {
      // Past the end: keep looking along the final direction.
      const over = s + ahead - this.length;
      target = [target[0] + fwd[0] * over, target[1] + fwd[1] * over, target[2] + fwd[2] * over];
    }
    // Bank from the signed horizontal turn rate, averaged over the look-ahead window.
    let turn = 0;
    const k = 4, span = Math.max(50, ahead);
    for (let i = 0; i < k; i++) {
      const s0 = s - span * 0.25 + (span * i) / k;
      const a = this.tangent(s0), b = this.tangent(s0 + span / k);
      turn += (a[0] * b[2] - a[2] * b[0]) / (span / k); // y-component of a × b per unit length
    }
    turn /= k;
    const bankK = (o.bank ?? this.o.bank) * 900;
    const max = this.o.maxBank;
    const roll = Math.max(-max, Math.min(max, (Math.atan(turn * bankK) * 180) / Math.PI));
    return { eye, target, roll, forward: fwd };
  }

  /**
   * Put `scene`'s camera at arc length `s` (adds `extraRoll` degrees, e.g. a barrel roll).
   * `shake` adds a handheld wobble: smooth noise on the camera, its aim and roll, `shake`
   * world units strong, varying with `time` (seconds; default the distance flown, so it
   * quickens with speed). Deterministic: the same `s`/`time` always gives the same pose.
   */
  apply(scene: Scene3D, s: number, extraRoll = 0, o: FlightShake = {}): FlightPose {
    const p = this.at(s);
    let [ex, ey, ez] = p.eye, [tx, ty, tz] = p.target, roll = p.roll + extraRoll;
    const amp = o.shake ?? 0;
    if (amp) {
      const t = (o.time ?? s / 600) * (o.rate ?? 1);
      // Sums of incommensurate sines: smooth, never repeating, cheap.
      const n = (k: number) => Math.sin(t * 1.7 + k * 1.3) * 0.5 + Math.sin(t * 3.1 + k * 2.9) * 0.3 + Math.sin(t * 7.3 + k * 4.1) * 0.2;
      ex += n(1) * amp; ey += n(2) * amp; ez += n(3) * amp * 0.3;
      tx += n(4) * amp * 2; ty += n(5) * amp * 2;
      roll += n(6) * amp * 0.04;
    }
    Object.assign(scene.camera, { x: ex, y: ey, z: ez });
    Object.assign(scene.target, { x: tx, y: ty, z: tz });
    scene.roll = roll;
    return p;
  }

  /**
   * A smooth, monotone time → distance function from keys `[seconds, fraction of length]`
   * (monotone cubic: speed changes smoothly and never reverses). Hold a fraction to stop.
   */
  timing(keys: [number, number][]): (t: number) => number {
    const k = keys.slice().sort((a, b) => a[0] - b[0]);
    const n = k.length;
    if (n === 1) return () => k[0][1] * this.length;
    const x = k.map((p) => p[0]), y = k.map((p) => p[1] * this.length);
    const dx = x.slice(1).map((v, i) => Math.max(1e-9, v - x[i]));
    const sl = dx.map((h, i) => (y[i + 1] - y[i]) / h);
    // Fritsch–Carlson tangents.
    const m = new Array<number>(n);
    m[0] = sl[0];
    m[n - 1] = sl[n - 2];
    for (let i = 1; i < n - 1; i++) m[i] = sl[i - 1] * sl[i] <= 0 ? 0 : (3 * (dx[i - 1] + dx[i])) / ((2 * dx[i] + dx[i - 1]) / sl[i - 1] + (dx[i] + 2 * dx[i - 1]) / sl[i]);
    for (let i = 0; i < n - 1; i++) {
      if (sl[i] === 0) { m[i] = 0; m[i + 1] = 0; continue; }
      const a = m[i] / sl[i], b = m[i + 1] / sl[i], h = Math.hypot(a, b);
      if (h > 3) { m[i] = (3 * a * sl[i]) / h; m[i + 1] = (3 * b * sl[i]) / h; }
    }
    return (t: number) => {
      if (t <= x[0]) return y[0];
      if (t >= x[n - 1]) return y[n - 1];
      let i = 0;
      while (i < n - 2 && t > x[i + 1]) i++;
      const h = dx[i], u = (t - x[i]) / h, u2 = u * u, u3 = u2 * u;
      return (2 * u3 - 3 * u2 + 1) * y[i] + (u3 - 2 * u2 + u) * h * m[i] + (-2 * u3 + 3 * u2) * y[i + 1] + (u3 - u2) * h * m[i + 1];
    };
  }
}

function mirror(a: [number, number, number], b: [number, number, number]): [number, number, number] {
  return [2 * a[0] - b[0], 2 * a[1] - b[1], 2 * a[2] - b[2]];
}

/** A smooth camera flight path through `points` (see {@link FlightPath}). */
export function flightPath(points: Point3[], options?: FlightPathOptions): FlightPath {
  return new FlightPath(points, options);
}
