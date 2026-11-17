#!/usr/bin/env node
// Side-by-side frames (Anikit Video | Remotion | HyperFrames) from the lossless PNG references
// that run.mjs renders into out/ref/<tool>/<comp>/ref/. Writes screens/<comp>-<t>s.png.
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const TIMES = { kinetic: [1.5, 7, 11, 17], product: [4, 8, 14], explainer: [3, 5.33, 12] };
const TOOLS = [["anikit", "Anikit Video"], ["remotion", "Remotion"], ["hyperframes", "HyperFrames"]];
const font = path.join(HERE, "shared/fonts/InterVariable.ttf");
fs.mkdirSync(path.join(HERE, "screens"), { recursive: true });

for (const [comp, times] of Object.entries(TIMES)) {
  for (const t of times) {
    const n = String(Math.round(t * 30) + 1).padStart(5, "0");
    const inputs = TOOLS.map(([tool]) => path.join(HERE, "out/ref", tool, comp, "ref", `${n}.png`));
    if (!inputs.every((p) => fs.existsSync(p))) { console.error("missing", inputs.filter((p) => !fs.existsSync(p))); continue; }
    const label = (i, name) => `[${i}:v]scale=960:540,drawtext=fontfile=${font}:text='${name}  t=${t}s':x=16:y=12:fontsize=22:fontcolor=white:box=1:boxcolor=black@0.6:boxborderw=6[v${i}]`;
    const fc = TOOLS.map(([, name], i) => label(i, name)).join(";") + ";[v0][v1][v2]hstack=3";
    const out = path.join(HERE, "screens", `${comp}-${t}s.png`);
    execFileSync("ffmpeg", ["-y", "-v", "error", ...inputs.flatMap((p) => ["-i", p]), "-filter_complex", fc, "-frames:v", "1", out]);
    console.log(out);
  }
}
