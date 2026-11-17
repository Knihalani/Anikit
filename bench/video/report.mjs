#!/usr/bin/env node
// Turns out/results.json into Markdown tables and writes them between the
// `<!-- results:start -->` / `<!-- results:end -->` markers of docs/video/BENCHMARKS.md
// (or prints them with --print). Also measures authoring cost (source bytes, ~tokens).
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const DOC = path.resolve(HERE, "../../docs/video/BENCHMARKS.md");
const R = JSON.parse(fs.readFileSync(path.join(HERE, process.argv.find((a) => a.endsWith(".json")) || "out/results.json"), "utf8"));
const COMPS = ["kinetic", "product", "explainer", "long"].filter((c) => R.rows.some((r) => r.comp === c));
const NAMES = { kinetic: "(a) kinetic type", product: "(b) product UI", explainer: "(c) held-heavy explainer", long: "(d) long: kinetic ×3, **60 fps, 60 s**" };
const FRAMES = { kinetic: 600, product: 600, explainer: 600, long: 3600 };
const TOOL = { anikit: "Anikit Video", remotion: "Remotion", hyperframes: "HyperFrames" };
const LABEL = {
  "anikit/default": "Anikit Video, default (`ak264`, good)",
  "anikit/ak264-draft": "Anikit Video, `ak264` draft (**960×540**)",
  "anikit/ak264-draft-full": "Anikit Video, `ak264` draft `--scale 1`",
  "anikit/ak264-best": "Anikit Video, `ak264` best",
  "anikit/h264-draft": "Anikit Video, `h264` (x264) draft `--scale 1`",
  "anikit/h264-good": "Anikit Video, `h264` (x264) good",
  "anikit/h264-best": "Anikit Video, `h264` (x264) best",
  "remotion/default": "Remotion, default",
  "remotion/fast": "Remotion, fast (`--concurrency=4 --x264-preset=veryfast`)",
  "hyperframes/default": "HyperFrames, default",
  "hyperframes/fast": "HyperFrames, fast (`--workers 4 --quality draft`)",
};
const ORDER = Object.keys(LABEL);
const median = (xs) => { const s = [...xs].sort((a, b) => a - b); return s[Math.floor(s.length / 2)]; };
const f = (x, d = 1) => (x == null || Number.isNaN(x) ? "–" : x.toFixed(d));
const mib = (b) => (b ? (b / 2 ** 20).toFixed(0) : "–");
const mb = (b) => (b ? (b / 1e6).toFixed(2) : "–");
const row = (t, c, comp) => R.rows.find((r) => r.tool === t && r.config === c && r.comp === comp && !r.explore);

const out = [];
const m = R.machine;
out.push(`Measured ${m.date.slice(0, 16).replace("T", " ")} UTC on ${m.nproc} vCPU (${m.cpu}), ${m.memGB} GB RAM, Linux ${m.kernel}. ` +
  `Anikit \`${m.anikit}\`, Remotion ${m.remotion}, HyperFrames ${m.hyperframes} (GSAP ${m.gsap}), React ${m.react}, ${m.chrome}, ${m.ffmpeg.replace(/ Copyright.*/, "")}, Node ${m.node}. ` +
  `Each cell is the median of ${R.rows[0]?.runs?.length ?? 3} runs. Compositions (a)–(c) are 600 frames (20 s at 30 fps), (d) is 3,600 frames (60 s at 60 fps), all 1920×1080.\n`);

// Headline: default vs default, and fastest vs fastest.
out.push("### Headline: wall time to a finished 1080p H.264 MP4 (seconds, lower is better)\n");
out.push("| Composition | Anikit Video (default) | Remotion (default) | HyperFrames (default) | Anikit Video vs fastest browser config |");
out.push("|---|---:|---:|---:|---|");
for (const comp of COMPS) {
  const a = row("anikit", "default", comp), r = row("remotion", "default", comp), h = row("hyperframes", "default", comp);
  const best = ["remotion/default", "remotion/fast", "hyperframes/default", "hyperframes/fast"].map((k) => row(...k.split("/"), comp)).filter((x) => x?.wall);
  const fastest = best.sort((x, y) => x.wall - y.wall)[0];
  const ratio = a?.wall && fastest ? `${(fastest.wall / a.wall).toFixed(0)}× faster than ${TOOL[fastest.tool]} ${fastest.config} (${f(fastest.wall)} s)` : "–";
  out.push(`| ${NAMES[comp]} | ${f(a?.wall, 2)} | ${f(r?.wall)} | ${f(h?.wall)} | ${ratio} |`);
}
out.push("");

for (const comp of COMPS) {
  out.push(`### ${NAMES[comp]}\n`);
  const ceil = (t) => R.ceiling?.find((c) => c.tool === t && c.comp === comp);
  out.push("| Tool and settings | Wall (s) | FPS | CPU (cores busy) | Peak RSS, tree (MiB) | Peak PSS, tree (MiB) | Size (MB) | Output | PSNR (dB) | SSIM |");
  out.push("|---|---:|---:|---:|---:|---:|---:|---|---:|---:|");
  for (const k of ORDER) {
    const [t, c] = k.split("/");
    const x = row(t, c, comp);
    if (!x) continue;
    if (!x.wall) { out.push(`| ${LABEL[k]} | failed | | | | | | | | |`); continue; }
    const v = x.video || {};
    out.push(`| ${LABEL[k]} | ${f(x.wall, 2)} | ${f(x.fps, 0)} | ${f(x.cores, 2)} | ${mib(x.peakRss)} | ${mib(x.peakPss)} | ${mb(x.bytes)} | ${v.codec_name} ${v.profile} ${v.width}×${v.height} | ${f(x.psnr, 2)} | ${f(x.ssim, 4)} |`);
  }
  const cs = ["anikit", "remotion", "hyperframes"].map((t) => ceil(t)).filter(Boolean);
  if (cs.length) out.push(`\n4:2:0 ceiling (each tool's PNG frames through lossless 4:2:0, the best any of these MP4s could score): ` +
    cs.map((c) => `${TOOL[c.tool]} ${f(c.psnr, 2)} dB / ${f(c.ssim, 4)}`).join(", ") + ".\n");
}

// Cold start.
out.push("### Cold start: one still frame (PNG at t = 0) from a fresh process (seconds)\n");
out.push("| Composition | Anikit Video `still` | Remotion `still` | HyperFrames `snapshot` |");
out.push("|---|---:|---:|---:|");
for (const comp of COMPS.filter((c) => c !== "long")) {
  const c = (t) => R.cold.find((x) => x.tool === t && x.comp === comp)?.wall;
  out.push(`| ${NAMES[comp]} | ${f(c("anikit"), 3)} | ${f(c("remotion"), 2)} | ${f(c("hyperframes"), 2)} |`);
}
out.push("");

// Settings tried.
const ex = R.rows.filter((r) => r.explore);
if (ex.length) {
  out.push("### Other browser-tool settings tried on (a) kinetic type (1 run each)\n");
  out.push("None of these beat the \"fast\" configurations above, so they are not in the main tables. Best browser result for comparison: " +
    ["remotion", "hyperframes"].map((t) => { const x = row(t, "fast", "kinetic"); return x ? `${TOOL[t]} fast ${f(x.wall)} s` : ""; }).join(", ") + ".\n");
  out.push("| Tool | Settings | Wall (s) | FPS | CPU (cores busy) | Size (MB) |");
  out.push("|---|---|---:|---:|---:|---:|");
  for (const x of ex) {
    const args = x.args.includes("__env_chromium") ? "defaults, full Chromium instead of chrome-headless-shell" : "`" + x.args.join(" ") + "`";
    if (!x.wall) { out.push(`| ${TOOL[x.tool]} | ${args} | refused: ${x.tool === "remotion" ? "concurrency above the 4 available cores" : "failed"} | | | |`); continue; }
    out.push(`| ${TOOL[x.tool]} | ${args} | ${f(x.wall, 2)} | ${f(x.fps, 0)} | ${f(x.cores, 2)} | ${mb(x.bytes)} |`);
  }
  out.push("");
}

// Throttling / noise.
const all = R.rows.filter((r) => r.wall && r.runs?.length > 1);
const steal = Math.max(...all.flatMap((r) => r.runs.map((x) => x.steal ?? 0)));
const spread = all.map((r) => { const w = r.runs.filter((x) => !x.failed).map((x) => x.wall); return (Math.max(...w) - Math.min(...w)) / Math.min(...w); });
const mhz = all.flatMap((r) => r.runs.flatMap((x) => x.mhz || [])).filter(Boolean);
if (all.some((r) => r.runs[0].steal != null)) {
  out.push(`**Throttling and noise.** Worst CPU steal during any timed run: ${(steal * 100).toFixed(1)}%. ` +
    `Spread between the fastest and slowest of the 3 runs: median ${(median(spread) * 100).toFixed(0)}%, worst ${(Math.max(...spread) * 100).toFixed(0)}%. ` +
    (mhz.length ? `Reported clock ${Math.min(...mhz).toFixed(0)}–${Math.max(...mhz).toFixed(0)} MHz (the VM reports a fixed nominal clock, so frequency throttling cannot be observed directly; steal and run-to-run spread are the evidence).` : "") + "\n");
}

// Authoring cost.
const read = (p) => { try { return fs.readFileSync(path.join(HERE, p), "utf8"); } catch { return ""; } };
const SRC = {
  anikit: { per: (c) => [`anikit/${c}.json`], shared: [] },
  remotion: { per: (c) => [`remotion/src/${c === "long" ? "Kinetic" : c[0].toUpperCase() + c.slice(1)}.tsx`], shared: ["remotion/src/common.tsx", "remotion/src/Root.tsx", "remotion/src/index.ts"] },
  hyperframes: { per: (c) => c === "long" ? ["hyperframes/long/index.html", "hyperframes/long/compositions/kinetic.html"] : [`hyperframes/${c}/index.html`], shared: ["shared/base.css"] },
};
out.push("### Authoring cost for an agent (source written by hand for this benchmark)\n");
out.push("Bytes of the composition file, plus the shared files that tool needs (Remotion: `common.tsx`, `Root.tsx`, `index.ts`; HyperFrames: `base.css`). Tokens ≈ characters / 4.\n");
out.push("| Composition | Anikit Video (JSON) | Remotion (TSX) | HyperFrames (HTML + GSAP) |");
out.push("|---|---:|---:|---:|");
const cost = (t, comp) => {
  const own = SRC[t].per(comp).map(read).join("");
  const shared = SRC[t].shared.map(read).join("");
  const total = own.length + shared.length;
  return `${own.length.toLocaleString("en-US")} B${shared ? ` + ${shared.length.toLocaleString("en-US")} B shared` : ""} ≈ ${Math.round(total / 4).toLocaleString("en-US")} tok`;
};
for (const comp of COMPS) out.push(`| ${NAMES[comp]} | ${cost("anikit", comp)} | ${cost("remotion", comp)} | ${cost("hyperframes", comp)} |`);
out.push("");

const md = out.join("\n");
if (process.argv.includes("--print") || !fs.existsSync(DOC)) console.log(md);
else {
  const doc = fs.readFileSync(DOC, "utf8");
  const a = doc.indexOf("<!-- results:start -->"), b = doc.indexOf("<!-- results:end -->");
  if (a < 0 || b < 0) throw new Error("markers missing in BENCHMARKS.md");
  fs.writeFileSync(DOC, doc.slice(0, a) + "<!-- results:start -->\n" + md + "\n" + doc.slice(b));
  console.error("updated", DOC);
}
