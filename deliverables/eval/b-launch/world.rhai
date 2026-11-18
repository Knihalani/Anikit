// Railyard: a signal-box track diagram. World coords: main route y=540, x 140..1780.
fn hsh(x) { let v = sin(x * 12.9898 + 78.233) * 43758.5453; v - floor(v) }
fn ex(s) { "=" + s }

let out = [];
let DIM = "#2C302A";
let DIM2 = "#363A33";
let LIT = "#FFF3D6";
let PALE = "#C9A35A";
let GREEN = "#7CF2A6";
let INK = "#D9D3C2";
let GONE = "=T<11.2?1:clamp(1-(T-11.2)*3,0,1)";

// ---------- panel tile seams ----------
let seams = [];
for c in -14..30 {
  let x = c.to_float() * 128.0;
  seams.push(#{ line: [x, -1000.0, x, 2100.0], stroke: "#0B0D0B", width: 2.0 });
  seams.push(#{ line: [x + 2.0, -1000.0, x + 2.0, 2100.0], stroke: "#1B1F1A", width: 1.0 });
}
for r in -8..17 {
  let y = r.to_float() * 128.0 + 12.0;
  seams.push(#{ line: [-1800.0, y, 3800.0, y], stroke: "#0B0D0B", width: 2.0 });
  seams.push(#{ line: [-1800.0, y + 2.0, 3800.0, y + 2.0], stroke: "#1B1F1A", width: 1.0 });
}
out.push(#{ draw: seams });

// ---------- yard: dim tracks, crossovers, and runs that fire in act 3 ----------
let yard = [];
let runs = [];
for k in -19..22 {
  if k >= -2 && k <= 4 { continue; }
  let kf = k.to_float();
  let y = 540.0 + 64.0 * kf;
  yard.push(#{ shape: "path", d: "M0 0 L5600 0", x: -1800.0, y: y, anchor: "topLeft", fill: "none",
               stroke: #{ color: DIM, dash: [34, 6] }, strokeWidth: 10.0 });
  // crossovers to the next row
  for q in 0..3 {
    let cx0 = -1600.0 + 5200.0 * hsh(kf * 3.1 + q.to_float() * 17.7);
    if k != -3 && k != 21 {
      yard.push(#{ shape: "path", d: "M0 0 L64 64", x: cx0, y: y, anchor: "topLeft", fill: "none",
                   stroke: #{ color: DIM, dash: [20, 6] }, strokeWidth: 10.0 });
    }
  }
  // two runs per row
  for q in 0..3 {
    let qf = q.to_float();
    let seed = kf * 7.3 + qf * 31.1;
    let len = 1300.0 + 1700.0 * hsh(seed + 1.0);
    let xa = -1700.0 + (5300.0 - len) * hsh(seed + 2.0);
    let s = 4.9 + 5.7 * sqrt(hsh(seed + 3.0));
    let d = 0.55 + 0.6 * hsh(seed + 4.0);
    let fade = "(T<11.25?1:clamp(1-(T-11.25)*2.2,0,1))";
    // pale route-set trace
    runs.push(#{ shape: "path", d: "M0 0 L" + len + " 0", x: xa, y: y, anchor: "topLeft", fill: "none",
                 stroke: #{ color: PALE, dash: [34, 6] }, strokeWidth: 10.0,
                 trimEnd: ex("clamp((T-" + (s - 0.25) + ")/0.2,0,1)"), opacity: ex("0.45*" + fade) });
    // the train: a lit window
    runs.push(#{ shape: "path", d: "M0 0 L" + len + " 0", x: xa, y: y, anchor: "topLeft", fill: "none",
                 stroke: #{ color: LIT, dash: [34, 6] }, strokeWidth: 10.0, glow: "#FFC86B",
                 trimEnd: ex("clamp((T-" + s + ")/" + d + ",0,1)"),
                 trimStart: ex("clamp((T-" + s + ")/" + d + "-0.3,0,1)"), opacity: ex(fade) });
    // webhook stubs along the run
    let ns = 2 + (hsh(seed + 5.0) * 2.0).to_int();
    for m in 0..ns {
      let u = (m.to_float() + 0.5 + 0.6 * (hsh(seed + 6.0 + m.to_float()) - 0.5)) / ns.to_float();
      let xs = xa + len * u;
      let tp = s + d * u;
      let up = if hsh(seed + 9.0 + m.to_float()) < 0.5 { 1.0 } else { -1.0 };
      let on = "clamp((T-" + tp + ")*14,0,1)*" + fade;
      if up > 0.0 {
        yard.push(#{ shape: "path", d: "M0 30 L30 0", x: xs, y: y - 30.0, anchor: "topLeft", fill: "none", stroke: DIM2, strokeWidth: 6.0 });
        runs.push(#{ shape: "path", d: "M0 30 L30 0", x: xs, y: y - 30.0, anchor: "topLeft", fill: "none", stroke: GREEN, strokeWidth: 6.0, opacity: ex(on) });
        yard.push(#{ shape: "circle", width: 13.0, height: 13.0, fill: "#1E231E", stroke: "#3B4139", strokeWidth: 2.0, x: xs + 36.0, y: y - 36.0 });
        runs.push(#{ shape: "circle", width: 13.0, height: 13.0, fill: GREEN, glow: GREEN, x: xs + 36.0, y: y - 36.0, opacity: ex(on) });
      } else {
        yard.push(#{ shape: "path", d: "M0 0 L30 30", x: xs, y: y, anchor: "topLeft", fill: "none", stroke: DIM2, strokeWidth: 6.0 });
        runs.push(#{ shape: "path", d: "M0 0 L30 30", x: xs, y: y, anchor: "topLeft", fill: "none", stroke: GREEN, strokeWidth: 6.0, opacity: ex(on) });
        yard.push(#{ shape: "circle", width: 13.0, height: 13.0, fill: "#1E231E", stroke: "#3B4139", strokeWidth: 2.0, x: xs + 36.0, y: y + 36.0 });
        runs.push(#{ shape: "circle", width: 13.0, height: 13.0, fill: GREEN, glow: GREEN, x: xs + 36.0, y: y + 36.0, opacity: ex(on) });
      }
    }
  }
}
out.push(#{ id: "yard", layers: yard, opacity: "=T<11.2?1:clamp(1-(T-11.2)*1.2,0.3,1)" });
out.push(#{ id: "runs", layers: runs });

// ---------- the hero route ----------
// train: starts T0, crosses 1640 px in D seconds (junction passes land on 8th notes at 128 bpm)
let T0 = 1.875;
let D = 1.40625;
let HEAD = "((T-" + T0 + ")/" + D + ")";

// dim route + branches
out.push(#{ shape: "path", d: "M0 0 L1640 0", x: 140.0, y: 540.0, anchor: "topLeft", fill: "none",
            stroke: #{ color: DIM2, dash: [34, 6] }, strokeWidth: 12.0 });
// buffer stops
out.push(#{ shape: "rect", width: 10.0, height: 54.0, radius: 2.0, fill: INK, x: 128.0, y: 540.0 });
out.push(#{ shape: "rect", width: 10.0, height: 54.0, radius: 2.0, fill: INK, x: 1792.0, y: 540.0 });

// route set (pale) at 0.9 s, and stays
out.push(#{ shape: "path", d: "M0 0 L1640 0", x: 140.0, y: 540.0, anchor: "topLeft", fill: "none",
            stroke: #{ color: PALE, dash: [34, 6] }, strokeWidth: 12.0, glow: "#7A5A20",
            trimEnd: "=clamp((T-0.95)/0.55,0,1)", opacity: "=T<12.25?0.75:0.75+0.25*clamp(1-(T-12.25)*1.5,0,1)" });
// the train window
out.push(#{ shape: "path", d: "M0 0 L1640 0", x: 140.0, y: 540.0, anchor: "topLeft", fill: "none",
            stroke: #{ color: LIT, dash: [34, 6] }, strokeWidth: 12.0, glow: "#FFC86B",
            trimEnd: ex("clamp(" + HEAD + ",0,1)"), trimStart: ex("clamp(" + HEAD + "-0.2,0,1)") });
// end-card route flash: all lamps on as the camera lands
out.push(#{ shape: "path", d: "M0 0 L1640 0", x: 140.0, y: 540.0, anchor: "topLeft", fill: "none",
            stroke: #{ color: LIT, dash: [34, 6] }, strokeWidth: 12.0, glow: "#FFC86B",
            trimEnd: "=clamp((T-11.95)/0.3,0,1)", opacity: "=T<11.9?0:1" });
// train head lamp
out.push(#{ shape: "rect", width: 30.0, height: 16.0, radius: 4.0, fill: "#FFFFFF", glow: "#FFD48A",
            x: ex("140+1640*clamp(" + HEAD + ",0,1)"), y: 540.0,
            opacity: ex("(T>" + T0 + ")*(T<" + (T0 + D + 0.05) + ")") });

let J = [540.0, 1010.0, 1480.0];
let EV = ["transfer.created", "transfer.submitted", "transfer.settled"];
for j in 0..3 {
  let jx = J[j];
  let tj = T0 + D * (jx - 140.0) / 1640.0;
  let bd = "M0 0 C50 0 60 10 90 40 L180 130 L230 130";
  out.push(#{ shape: "path", d: bd, x: jx, y: 540.0, anchor: "topLeft", fill: "none",
              stroke: #{ color: DIM2, dash: [26, 6] }, strokeWidth: 10.0 });
  out.push(#{ shape: "path", d: bd, x: jx, y: 540.0, anchor: "topLeft", fill: "none",
              stroke: #{ color: GREEN, dash: [26, 6] }, strokeWidth: 10.0, glow: "#2FA563",
              trimEnd: ex("clamp((T-" + tj + ")/0.2,0,1)") });
  // signal head above the junction: red until the train passes, then green
  out.push(#{ shape: "rect", width: 22.0, height: 44.0, radius: 11.0, fill: "#090B09", stroke: "#3B4139", strokeWidth: 2.0, x: jx - 34.0, y: 492.0 });
  out.push(#{ shape: "circle", width: 12.0, height: 12.0, fill: "#FF4A32", glow: "#FF4A32", x: jx - 34.0, y: 482.0,
              opacity: ex("T>0.95 && T<" + tj + " ? 1 : 0.12") });
  out.push(#{ shape: "circle", width: 12.0, height: 12.0, fill: GREEN, glow: GREEN, x: jx - 34.0, y: 502.0,
              opacity: ex("T>=" + tj + " ? 1 : 0.12") });
  // terminal lamp
  out.push(#{ shape: "circle", width: 22.0, height: 22.0, fill: "#141814", stroke: "#4A5048", strokeWidth: 3.0, x: jx + 246.0, y: 670.0 });
  out.push(#{ shape: "circle", width: 22.0, height: 22.0, fill: GREEN, glow: GREEN, x: jx + 246.0, y: 670.0,
              opacity: ex("clamp((T-" + (tj + 0.2) + ")*12,0,1)"),
              scale: ex("1+0.5*exp(-(T-" + (tj + 0.2) + ")*9)*(T>" + (tj + 0.2) + ")") });
  // engraved event label: dim, lights when fired
  out.push(#{ text: EV[j], font: "JetBrains Mono", size: 34.0, weight: 500, x: jx + 30.0, y: 708.0, anchor: "topLeft",
              color: ex("T<" + (tj + 0.2) + " ? #4E544B : mix(#7CF2A6, #E9F7E8, clamp((T-" + (tj + 0.2) + ")*3,0,1))") });
}

// source: plate, push button, code
out.push(#{ text: "FROM  ACCOUNT ····4821", font: "Archivo", axes: #{ wdth: 75.0 }, weight: 700, size: 30.0, letterSpacing: "0.08em",
            color: INK, x: 140.0, y: 404.0, anchor: "topLeft", opacity: GONE });
out.push(#{ text: "TO  ACCOUNT ····0937", font: "Archivo", axes: #{ wdth: 75.0 }, weight: 700, size: 30.0, letterSpacing: "0.08em",
            color: INK, x: 1800.0, y: 404.0, anchor: "topRight", opacity: GONE });
// push button
out.push(#{ shape: "rect", width: 64.0, height: 64.0, radius: 10.0, fill: "#0A0C0A", stroke: "#4A5048", strokeWidth: 3.0, x: 182.0, y: 628.0, opacity: GONE });
out.push(#{ shape: "rect", width: 44.0, height: 44.0, radius: 22.0, fill: "#262B25", x: 182.0, y: 628.0, opacity: GONE });
out.push(#{ shape: "rect", width: 44.0, height: 44.0, radius: 22.0, fill: "#FFF3D6", glow: "#FFC86B", opacity: "=T<0.47?0:(T<11.2?1:clamp(1-(T-11.2)*3,0,1))",
            x: 182.0, y: 628.0, scale: "=T<0.47?1:(1-0.12*exp(-(T-0.47)*14))" });
out.push(#{ text: "POST /v1/transfers", font: "JetBrains Mono", size: 26.0, opacity: GONE, weight: 600, color: LIT,
            x: 234.0, y: 606.0, anchor: "topLeft", enter: #{ preset: "type", duration: 0.45 } });
out.push(#{ text: "amount  48,250.00 USD", font: "JetBrains Mono", size: 20.0, opacity: GONE, color: "#9C9888",
            x: 234.0, y: 642.0, anchor: "topLeft", enter: #{ preset: "type", delay: 0.2, duration: 0.4 } });

// destination: the balance lights on arrival
let TA = T0 + D;
out.push(#{ text: "+48,250.00", font: "JetBrains Mono", size: 44.0, weight: 700, x: 1800.0, y: 452.0, anchor: "topRight",
            color: "#FFF3D6", glow: "#FFB547", opacity: ex("T<" + TA + " ? 0 : (T<11.2?1:clamp(1-(T-11.2)*3,0,1))"),
            scale: ex("1+0.15*exp(-(T-" + TA + ")*10)*(T>" + TA + ")") });
out.push(#{ text: "SETTLED", font: "Archivo", axes: #{ wdth: 75.0 }, weight: 700, size: 24.0, letterSpacing: "0.12em",
            x: 1800.0, y: 566.0, anchor: "topRight", color: "#D9D3C2", opacity: ex("T<" + TA + " ? 0 : (T<11.2?1:clamp(1-(T-11.2)*3,0,1))") });

out
