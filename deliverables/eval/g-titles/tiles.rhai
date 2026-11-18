// One hand-painted azulejo tile, stamped into a wall and driven by rules.
// Bars at 96 bpm: 2.5 s. bar2 = instances, bar3 = rule A, bar4 = offset + rule B, bar5 = the gap.
let S = 120.0;
let COB = "#1B3C8F";
let H = S;
let Z = 0.0;
let out = [];

fn rot_a(c, r) { // rosette rule: 2x2 blocks meet at their painted corners
  let a = if r % 2 == 0 { if c % 2 == 0 { 180 } else { 270 } } else { if c % 2 == 0 { 90 } else { 0 } };
  a
}
fn rot_b(c, r) { // after the offset: diagonal flow, alternate rows mirrored
  let a = if r % 2 == 0 { 0 } else { 180 };
  a
}
fn fwd(from, to) { // next angle >= from+90 that is congruent to `to`
  let d = ((to - from) % 360 + 360) % 360;
  if d == 0 { d = 360; }
  from + d
}
fn petal(cx, cy, ang, r0, r1, wid) {
  let a = ang.to_radians();
  let dx = cos(a); let dy = sin(a);
  let px = -dy; let py = dx;
  let x0 = cx + dx * r0; let y0 = cy + dy * r0;
  let x1 = cx + dx * r1; let y1 = cy + dy * r1;
  let mx = cx + dx * (r0 + r1) * 0.5; let my = cy + dy * (r0 + r1) * 0.5;
  "M" + x0 + " " + y0 + " Q" + (mx + px * wid) + " " + (my + py * wid) + " " + x1 + " " + y1
    + " Q" + (mx - px * wid) + " " + (my - py * wid) + " " + x0 + " " + y0 + " Z"
}
fn qdisc(cx, cy, r, a0) { // quarter disc at a corner, a0 = start angle in degrees
  let a1 = a0 + 90.0;
  let x0 = cx + r * cos(a0.to_radians()); let y0 = cy + r * sin(a0.to_radians());
  let x1 = cx + r * cos(a1.to_radians()); let y1 = cy + r * sin(a1.to_radians());
  "M" + cx + " " + cy + " L" + x0 + " " + y0 + " A" + r + " " + r + " 0 0 1 " + x1 + " " + y1 + " Z"
}
fn qarc(cx, cy, r, a0) {
  let a1 = a0 + 90.0;
  let x0 = cx + r * cos(a0.to_radians()); let y0 = cy + r * sin(a0.to_radians());
  let x1 = cx + r * cos(a1.to_radians()); let y1 = cy + r * sin(a1.to_radians());
  "M" + x0 + " " + y0 + " A" + r + " " + r + " 0 0 1 " + x1 + " " + y1
}

// ---- the motif, in tile-local px, painted corner at (0,0) ----
let washA = [ #{ d: qdisc(Z, Z, 0.56 * S, 0.0), fill: "#86A6DA" },
              #{ d: qdisc(H, H, 0.27 * S, 180.0), fill: "#86A6DA" },
              #{ d: petal(Z, Z, 45.0, 0.60 * S, 1.02 * S, 0.11 * S), fill: "#86A6DA" } ];
let washB = [ #{ d: qdisc(Z, Z, 0.56 * S, 0.0), fill: "#E3A82B" },
              #{ d: qdisc(H, H, 0.27 * S, 180.0), fill: "#E3A82B" },
              #{ d: petal(Z, Z, 45.0, 0.60 * S, 1.02 * S, 0.11 * S), fill: "#E3A82B" } ];
let ink = [ #{ d: qdisc(Z, Z, 0.26 * S, 0.0), fill: COB },
            #{ d: qarc(Z, Z, 0.40 * S, 0.0), stroke: COB, width: 0.045 * S, cap: "round" },
            #{ d: qarc(Z, Z, 0.56 * S, 0.0), stroke: COB, width: 0.018 * S, cap: "round" },
            #{ d: petal(Z, Z, 16.0, 0.58 * S, 0.86 * S, 0.075 * S), fill: COB },
            #{ d: petal(Z, Z, 74.0, 0.58 * S, 0.86 * S, 0.075 * S), fill: COB },
            #{ d: petal(Z, Z, 45.0, 0.66 * S, 0.94 * S, 0.05 * S), stroke: COB, width: 0.02 * S },
            #{ d: qdisc(H, H, 0.13 * S, 180.0), fill: COB },
            #{ d: qarc(H, H, 0.27 * S, 180.0), stroke: COB, width: 0.02 * S },
            #{ d: qdisc(H, Z, 0.09 * S, 90.0), fill: COB },
            #{ d: qdisc(Z, H, 0.09 * S, 270.0), fill: COB },
            #{ d: "M" + (0.60 * S) + " " + (0.04 * S) + " Q" + (0.80 * S) + " " + (0.22 * S) + " " + (0.96 * S) + " " + (0.10 * S), stroke: COB, width: 0.022 * S, cap: "round" },
            #{ d: "M" + (0.04 * S) + " " + (0.60 * S) + " Q" + (0.22 * S) + " " + (0.80 * S) + " " + (0.10 * S) + " " + (0.96 * S), stroke: COB, width: 0.022 * S, cap: "round" } ];

// split the ink: fills stamp, strokes are drawn by hand (hero tile only)
let inkFill = []; let inkLine = [];
for e in ink { if e.contains("fill") { inkFill.push(e); } else { inkLine.push(e); } }

// gap in the wall, in final columns (after the offset), ragged at the edges
fn in_gap(fc, r) {
  if r == 3 { return fc >= 4 && fc <= 12; }
  if r == 4 { return fc >= 3 && fc <= 13; }
  if r == 5 { return fc >= 4 && fc <= 12; }
  if r == 2 { return fc == 6 || fc == 7 || fc == 11; }
  if r == 6 { return fc == 5 || fc == 10 || fc == 11; }
  false
}
let HANG_C = 9; let HANG_R = 2;   // final column of the tile left hanging

let marks = [];
for r in -1..10 {
  for c in -2..18 {
    let k = (r + 2) * 24 + (c + 3);
    let hero = (c == 8 && r == 4);
    let pair = floor(r.to_float() / 2.0).to_int();
    let sh = if pair % 2 != 0 { 1 } else { 0 };          // block-rows that slide one tile
    let fc = c + sh;
    let cx = c * S;
    let cy = r * S + 60.0;
    let dist = sqrt(((c - 8) * (c - 8) + (r - 4) * (r - 4)).to_float());
    let r0 = if hero { 0 } else { floor(rand(k * 7 + 3) * 4.0).to_int() * 90 };
    let ra = if hero { 360 + rot_a(c, r) } else { fwd(r0, rot_a(c, r)) };
    let rb = ra + 180;
    let ta = 2.55 + dist * 0.16 + rand(k * 3 + 1) * 0.1;      // instance appears
    let tr = 4.95 + (c + r) * 0.03;                          // rule A wave
    let tb = 8.05 + (fc - r) * 0.025;                         // invert + recolour wave
    let tint = 0.5 + 0.5 * rand(k * 11 + 5);
    let glaze = "linear-gradient(135deg, " + oklch(0.975 - 0.012 * tint, 0.010, 85.0) + ", " + oklch(0.925 - 0.02 * tint, 0.014, 80.0) + ")";

    let anim = [];
    anim.push(#{ to: #{ rotation: ra }, at: tr, duration: 0.55, ease: "outBack" });
    if sh > 0 {
      anim.push(#{ to: #{ x: "+" + S }, at: 7.4 + (r + 2) * 0.03, duration: 0.6, ease: "inOutExpo" });
    }
    anim.push(#{ to: #{ rotation: rb }, at: tb, duration: 0.5, ease: "outBack" });
    let gap = in_gap(fc, r);
    if gap {
      let tf = 10.0 + rand(k * 5 + 2) * 0.3;
      anim.push(#{ to: #{ scale: 0.97 }, at: tf - 0.1, duration: 0.1, ease: "outQuad" });
      anim.push(#{ to: #{ y: "+" + (1000.0 + rand(k) * 200.0), rotation: "+" + ((rand(k * 13) - 0.5) * 70.0) }, at: tf, duration: 0.85, ease: "inQuad" });
      marks.push(#{ x: fc * S, y: cy, k: k });
    }
    if fc == HANG_C && r == HANG_R {
      anim.push(#{ keyframes: #{ rotation: [rb, rb + 9, rb + 6, rb + 8] }, at: 10.25, duration: 0.9, ease: "outCubic" });
      anim.push(#{ to: #{ y: "+3" }, at: 10.25, duration: 0.3 });
      anim.push(#{ to: #{ y: "+1150", rotation: "+38" }, at: 13.35, duration: 0.8, ease: "inQuad" });
      marks.push(#{ x: fc * S, y: cy, k: k });
    }

    let kids = [ #{ shape: "rect", width: S - 4.0, height: S - 4.0, x: Z + 2.0, y: Z + 2.0, anchor: "topLeft", radius: 5.0, fill: glaze,
                    shadow: #{ color: "#3A2E1F55", blur: 3, x: 1, y: 2 } } ];
    let fadeA = #{ to: #{ opacity: 0.0 }, at: tb + 0.05, duration: 0.3 };
    if hero {
      kids.push(#{ draw: washA, opacity: 0.0, blend: "multiply", inkTexture: #{ amount: 0.45, scale: 0.6, seed: 3 },
                   animate: [ #{ to: #{ opacity: 0.95 }, at: 1.5, duration: 0.8, ease: "outCubic" }, fadeA ] });
      kids.push(#{ draw: inkLine, blend: "multiply", inkTexture: #{ amount: 0.3, scale: 0.7, seed: 5 },
                   enter: #{ preset: "draw", duration: 2.1, delay: 0.0 } });
      kids.push(#{ draw: inkFill, blend: "multiply", inkTexture: #{ amount: 0.3, scale: 0.7, seed: 6 }, opacity: 0.0,
                   animate: [ #{ to: #{ opacity: 1.0 }, at: 1.0, duration: 0.9, ease: "outCubic" } ] });
    } else {
      kids.push(#{ draw: washA, opacity: 0.0, blend: "multiply", inkTexture: #{ amount: 0.45, scale: 0.6, seed: k % 9 },
                   animate: [ #{ to: #{ opacity: 0.92 }, at: ta + 0.05, duration: 0.25 }, fadeA ] });
      kids.push(#{ draw: ink, opacity: 0.0, blend: "multiply", inkTexture: #{ amount: 0.3, scale: 0.7, seed: k % 11 },
                   animate: [ #{ to: #{ opacity: 1.0 }, at: ta, duration: 0.1 } ] });
    }
    kids.push(#{ draw: washB, opacity: 0.0, blend: "multiply", inkTexture: #{ amount: 0.45, scale: 0.6, seed: k % 7 },
                 animate: [ #{ to: #{ opacity: 0.9 }, at: tb + 0.05, duration: 0.3 } ] });
    out.push(#{ layers: kids, x: cx, y: cy, width: S, height: S, anchor: "center", rotation: r0, animate: anim });
  }
}

// what a fallen tile leaves: grey adhesive combed in ridges
let back = [];
for m in marks {
  back.push(#{ shape: "rect", width: S - 10.0, height: S - 10.0, x: m.x, y: m.y, radius: 8.0, fill: "#D8CFBD", opacity: 0.6,
               rotation: (rand(m.k * 3) - 0.5) * 6.0, inkTexture: #{ amount: 0.5, scale: 0.8, seed: m.k % 13 } });
  back.push(#{ shape: "rect", width: S - 22.0, height: S - 22.0, x: m.x + (rand(m.k) - 0.5) * 8.0, y: m.y, opacity: 0.22,
               fill: "#B7AC97", pattern: #{ kind: "lines", spacing: 11, mark: 0.3, angle: 90.0 + (rand(m.k * 5) - 0.5) * 30.0 },
               inkTexture: #{ amount: 0.6, scale: 0.6, seed: m.k % 5 } });
}
back + out
