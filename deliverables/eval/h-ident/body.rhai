// ---- scene 0: the projector strip, the gate comes alive, the stoop ----
let N = "9.0*(1.0-pow(1.0-clamp(T/2.25,0,1),2))";
let S = "(floor(" + N + ")+smoothstep(0.45,1,fract(" + N + ")))";
let GX = "400*(9-" + S + ")";
let WEAVE = "1.6*noise(T*9)";
let L = [];
// film base
L.push(#{ shape: "rect", width: 7000, height: 400, fill: "#17120d", x: "=960+" + GX + "-1600", y: "=540+" + WEAVE });
for k in 0..16 {
  let cx = 960.0 + 400.0 * (k.to_float() - 9.0);
  let pos = "=" + cx + "+" + GX;
  let fy = "=540+" + WEAVE;
  // sprocket holes (4 per frame, top and bottom)
  for j in 0..4 {
    let hx = cx - 150.0 + j.to_float() * 100.0;
    L.push(#{ shape: "rect", width: 34, height: 22, radius: 5, fill: "#050403", x: "=" + hx + "+" + GX, y: "=372+" + WEAVE });
    L.push(#{ shape: "rect", width: 34, height: 22, radius: 5, fill: "#050403", x: "=" + hx + "+" + GX, y: "=708+" + WEAVE });
  }
  // the plate: paper ground
  let sky = if k == 9 { "=mix(#E4D3B0, #F2C978, smoothstep(2.3, 3.2, T))" } else { "#E4D3B0" };
  L.push(#{ shape: "rect", width: 360, height: 270, radius: 8, fill: sky, x: pos, y: fy });
  // Muybridge backdrop grid: fine ruled lines
  let gi = [];
  for g in 0..10 { let gx = cx - 180.0 + 36.0 * g.to_float(); gi.push(#{ line: [gx, 405.0, gx, 675.0], stroke: "#8a7656", width: 1.2, opacity: 0.55 }); }
  for g in 0..7 { let gy = 405.0 + 45.0 * g.to_float(); gi.push(#{ line: [cx - 180.0, gy, cx + 180.0, gy], stroke: "#8a7656", width: 1.2, opacity: 0.55 }); }
  let gop = if k == 9 { "=1-smoothstep(2.4,3.2,T)" } else { "1" };
  L.push(#{ draw: gi, x: "=960+" + GX, y: "=540+" + WEAVE, opacity: gop });
  // low sun behind the hovering bird (gate frame only)
  if k == 9 {
    L.push(#{ shape: "circle", width: 120, fill: "#FFF1CF", x: 1010, y: 520, opacity: "=0.9*smoothstep(2.5,3.4,T)", blur: 6 });
    L.push(#{ shape: "circle", width: 300, fill: "#FFD98A", x: 1010, y: 520, opacity: "=0.5*smoothstep(2.5,3.4,T)", blur: 40 });
  }
  if k == 9 {
    let hp = [[cx - 180.0, 675.0]];
    for g in 0..25 { let gx = cx - 180.0 + 15.0 * g.to_float(); hp.push([gx, 618.0 - 9.0 * noise(g.to_float() * 0.35) - 4.0 * sin(g.to_float() * 0.7)]); }
    hp.push([cx + 180.0, 675.0]);
    L.push(#{ draw: [#{ points: hp, closed: true, smooth: true, fill: "#5a3f22" }], x: "=960+" + GX, y: "=540+" + WEAVE, opacity: "=0.85*smoothstep(2.4,3.2,T)" });
  }
  // the bird: plate pose, or live in the gate frame
  let p0 = k % 6;
  if k != 9 {
    L.push(#{ draw: bird(p0.to_float() / 6.0, 0.0, cx + 62.0, 492.0, 0.62, 48.0, 30.0, "#1b140e", "#3b2d22", ""), x: "=960+" + GX, y: "=540+" + WEAVE });
  } else {
    for q in 0..6 {
      let idx = "mod(" + p0 + "+floor(max(0,T-2.25)*30),6)";
      let op = "=(T<3.85)*(abs(" + idx + "-" + q + ")<0.5)";
      L.push(#{ draw: bird(q.to_float() / 6.0, 0.0, cx + 30.0, 488.0, 0.62, 48.0 + 4.0*sin(q.to_float()*1.047), 30.0, "#1b140e", "#3b2d22", "#E0A33A"), x: "=960+" + GX, y: "=540+" + WEAVE, opacity: op });
    }
    L.push(#{ draw: bird(0.0, 1.0, cx + 20.0, 466.0, 0.62, -72.0, 18.0, "#1b140e", "#3b2d22", ""), opacity: "=T>=3.85", x: "=960+" + GX, y: "=540+320*pow(clamp((T-3.85)/0.4,0,1),2.2)" });
  }
  // gate: plates away from the aperture sit in shadow
  L.push(#{ shape: "rect", width: 362, height: 272, radius: 8, fill: "#0c0906", x: pos, y: fy,
            opacity: "=0.72*smoothstep(120,620,abs(" + cx + "+" + GX + "-960))" });
}
spec.scenes[0].layers = L + spec.scenes[0].layers;

// ---- scene 1: the mark ----
let M = [];
for q in 0..6 {
  let op = "=abs(mod(floor(T*30),6)-" + q + ")<0.5";
  M.push(#{ draw: bird(q.to_float() / 6.0, 0.0, 1010.0, 300.0, 0.95, 48.0 + 4.0*sin(q.to_float()*1.047), 30.0, "#D9A441", "#8f6a2b", ""), opacity: op,
            enter: #{ preset: "fadeIn", duration: 0.5 } });
}
spec.scenes[1].layers = M + spec.scenes[1].layers;
spec
