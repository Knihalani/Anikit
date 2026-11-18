fn shx(dx, a) {
  if dx == 0.0 { return ""; }
  let c = "clamp((t-(" + a + "))/0.6,0,1)";
  " + " + dx + "*(" + c + "*" + c + "*(3-2*" + c + "))"
}
// Shell generator: each cue [t, x, y, R, kind, ink, lift] draws one firework,
// hanabi-catalogue style: a prefire trail rises for `lift` s and breaks at t.
let out = [];
let k = 0;
let INK = #{ vermilion: "#E8432A", gold: "#E9A23B", cobalt: "#2346B8" };
for c0 in cues {
  let c = c0;
  if type_of(c0) == "map" {
    let ink = INK[c0.ink];
    let core = if c0.ink == "gold" { INK.vermilion } else { INK.gold };
    c = [c0.at, c0.x, c0.y, c0.r, c0.shell, ink, c0.lift, 0.0, core];
  }
  let t0 = c[0]; let cx = c[1] * w; let cy = c[2] * h; let R = c[3] * h; let kind = c[4]; let ink = c[5];
  let lift = if c.len() > 6 { c[6] } else { 0.0 };
  let ox = if c.len() > 7 { c[7] * w } else { 0.0 };
  let s = 1.0 * k;
  // lift trail: a thin capsule rising from below the frame to the break
  let og = if k < origins.len() { origins[k] } else { () };
  if lift > 0.0 && type_of(og) == "array" {
    let ts = t0 - lift;
    let gx = og[0] * w; let gy = og[1] * h;
    for q in 0..4 {
      let lg = 0.035 * q;
      let e = "(1-pow(1-clamp((t-" + lg + ")/" + lift + ",0,1),2.2))";
      out.push(#{ shape: "circle", width: 12.0 - 2.5 * q, fill: ink, start: ts, end: t0 + 0.03,
        x: "=" + gx + " + (" + (cx - gx) + ")*" + e + shx(mv[1] * w, mv[0] - ts),
        y: "=" + gy + " + (" + (cy - gy) + ")*" + e, opacity: 1.0 - 0.22 * q });
    }
  } else if lift > 0.0 {
    let y0 = h + 40.0;
    let ts = t0 - lift;
    out.push(#{ shape: "rect", width: 5, height: 70, radius: 3, fill: ink, x: "=" + (cx + ox) + shx(mv[1] * w, mv[0] - ts), start: ts, end: t0 + 0.02,
      y: "=" + y0 + " + (" + (cy - y0) + ")*(1-pow(1-clamp(t/" + lift + ",0,1),2.2))",
      opacity: "=0.85" });
  }
  if kind != "name" {
    out.push(#{ shape: "circle", width: R * 2.6, fill: ink, blur: 60, start: t0, end: t0 + 1.2,
      x: "=" + cx + shx(mv[1] * w, mv[0] - t0), y: cy, opacity: "=0.16*exp(-4.0*t)" });
    out.push(#{ shape: "circle", width: R * 0.5, fill: "#FFF8EC", blur: 18, start: t0, end: t0 + 0.4,
      x: "=" + cx + shx(mv[1] * w, mv[0] - t0), y: cy, opacity: "=0.9*exp(-12*t)" });
  }
  let rings = if kind == "pistil" { [1.0, 0.48] } else if kind == "ring" { [1.0] } else if kind == "peony" { [1.0] } else if kind == "chrys" { [1.0, 0.62] } else { [] };
  let ri = 0;
  for rf in rings {
    let n = if rf < 0.7 { 18 } else { 36 };
    if kind == "ring" { n = 48; }
    let col = if ri == 1 && c.len() > 8 { c[8] } else { ink };
    let mh = if w < h { w } else { h };
    let dot = if rf < 0.7 { 0.018 * mh } else { 0.016 * mh };
    for j in 0..n {
      let sd = k * 1000 + ri * 100 + j;
      let a = (j * 360.0 / n + 7.0 * ri + (rand(sd) - 0.5) * 3.0).to_radians();
      let rr = R * rf * (0.96 + 0.08 * rand(sd + 7));
      let life = 1.9 + 0.7 * rand(sd + 13);
      let fx = "(1-exp(-6.5*t))";
      if kind == "chrys" {
        // tailed stars: capsules oriented along their flight
        out.push(#{ shape: "rect", width: 0.9 * dot + 0.20 * rr, scaleX: "=(" + (0.9 * dot) + " + " + (0.20 * rr) + "*(1-exp(-25.0*t))*exp(-2.5*t))/" + (0.9 * dot + 0.20 * rr), height: dot * 0.9, radius: dot * 0.45,
          fill: col, rotation: a.to_degrees(), start: t0, end: t0 + 2.2,
          x: "=" + cx + " + " + rr + "*" + fx + "*" + cos(a) + shx(mv[1] * w, mv[0] - t0),
          y: "=" + cy + " + " + rr + "*" + fx + "*" + sin(a) + " + 34*t*t",
          opacity: "=clamp(" + (life + 0.2) + " - t*1.0, 0, 1)" });
        for q in 1..3 {
          let lag = 0.07 * q;
          let fq = "(1-exp(-6.5*max(t-" + lag + ",0)))";
          out.push(#{ shape: "circle", width: dot * (0.75 - 0.18 * q), fill: col, start: t0 + lag, end: t0 + 2.0,
            x: "=" + cx + " + " + rr + "*" + fq + "*" + cos(a) + shx(mv[1] * w, mv[0] - t0 - lag),
            y: "=" + cy + " + " + rr + "*" + fq + "*" + sin(a) + " + 34*t*t",
            opacity: "=clamp(1.6 - t*1.0, 0, 1)*0.8" });
        }
      } else {
        out.push(#{ shape: "circle", width: dot, scale: "=1.0 - 0.35*clamp(t/1.8,0,1)", fill: col, start: t0, end: t0 + 2.3,
          x: "=" + cx + " + " + rr + "*" + fx + "*" + cos(a) + shx(mv[1] * w, mv[0] - t0),
          y: "=" + cy + " + " + rr + "*" + fx + "*" + sin(a) + " + 30*t*t",
          opacity: "=clamp(" + (life + 0.3) + " - t*1.0, 0, 1)" });
      }
    }
    ri += 1;
  }
  if kind == "willow" {
    let n = 22;
    for j in 0..n {
      let a = (j * 360.0 / n + 4.0).to_radians();
      let vx = R * cos(a); let vy = R * sin(a) * 0.7;
      // parabolic strand: out, then hanging down
      let p1x = cx + vx * 0.9; let p1y = cy + vy * 0.9 - 0.05 * h;
      let p2x = cx + vx * (1.0 + 0.3 * rand(k * 50 + j)); let p2y = cy + vy + (0.30 + 0.16 * rand(k * 50 + j + 3)) * h;
      let d = "M" + cx + " " + cy + " Q" + p1x + " " + p1y + " " + p2x + " " + p2y;
      out.push(#{ draw: [#{ d: d, stroke: ink, width: 0.0042 * h, cap: "round" }], start: t0, end: t0 + 3.0,
        animate: [#{ from: #{ trimEnd: 0.0 }, to: #{ trimEnd: 1.0 }, duration: 1.6, ease: "outCubic" },
                  #{ from: #{ trimStart: 0.0 }, to: #{ trimStart: 1.0 }, at: 0.9, duration: 2.0, ease: "inQuad" }] });
    }
  }
  if kind == "name" {
    let rows = c[8];
    let p = c[3] * h;
    let nc = rows[0].len();
    let nr = rows.len();
    let x0 = cx - (nc - 1) * p / 2.0; let y0 = cy - (nr - 1) * p / 2.0;
    let bx = if c.len() > 9 { c[9] * w } else { cx };
    let by = if c.len() > 10 { c[10] * h } else { cy };
    let m = 0;
    for r in 0..nr {
      let line = rows[r];
      for q in 0..nc {
        if line.sub_string(q, 1) == "#" {
          let tx = x0 + q * p; let ty = y0 + r * p;
          let dl = 0.04 * rand(m * 7 + 3);
          out.push(#{ shape: "circle", width: p * 0.66, fill: ink, start: t0 + dl,
            x: "=" + bx + " + (" + (tx - bx) + ")*(1-exp(-5.5*t))",
            y: "=" + by + " + (" + (ty - by) + ")*(1-exp(-5.5*t)) + " + (0.10 * h) + "*t*exp(-2.2*t)" });
          m += 1;
        }
      }
    }
  }
  k += 1;
}
out
