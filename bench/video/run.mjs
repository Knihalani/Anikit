#!/usr/bin/env node
// End-to-end video benchmark: Anikit Video vs Remotion vs HyperFrames.
//
//   node run.mjs [--tools anikit,remotion,hyperframes] [--comps kinetic,product,explainer]
//                [--runs 3] [--configs default,fast,...] [--no-quality] [--no-cold]
//
// For every (tool, config, composition) it renders an H.264 MP4 `runs` times and records the
// median wall time, fps, peak memory of the whole process tree and output size. It then scores
// one output against that tool's own lossless PNG render (PSNR/SSIM via ffmpeg), and times a
// single-frame still ("cold start"). Results go to out/results.json; `node report.mjs` turns
// them into the tables in docs/video/BENCHMARKS.md.
import { spawn, spawnSync, execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, "../..");
const OUT = path.join(HERE, "out");
const AV = process.env.ANIKIT_VIDEO || path.join(ROOT, "target/release/anikit-video");
const BIN = path.join(HERE, "node_modules/.bin");
// Pre-installed browsers (no download here). Remotion and HyperFrames both prefer chrome-headless-shell.
const HEADLESS_SHELL = process.env.HEADLESS_SHELL ||
  "/opt/pw-browsers/chromium_headless_shell-1243/chrome-headless-shell-linux64/chrome-headless-shell";
const FPS = 30, FRAMES = 600, W = 1920, H = 1080;

const argv = process.argv.slice(2);
const opt = (k, d) => { const i = argv.indexOf(`--${k}`); return i >= 0 ? argv[i + 1] : d; };
const flag = (k) => argv.includes(`--${k}`);
const RUNS = Number(opt("runs", 3));
const TOOLS = opt("tools", "anikit,remotion,hyperframes").split(",");
const COMPS = opt("comps", "kinetic,product,explainer").split(",");
const ONLY = opt("configs", null)?.split(",");

// ---------------------------------------------------------------- tools and their settings
const hfEnv = { HYPERFRAMES_BROWSER_PATH: HEADLESS_SHELL, HYPERFRAMES_NO_TELEMETRY: "1", DO_NOT_TRACK: "1" };
const TOOLDEFS = {
  anikit: {
    configs: {
      // `render x.mp4` with no flags = ak264 at --quality good.
      default: [],
      "ak264-draft": ["--codec", "ak264", "--quality", "draft"],
      "ak264-draft-full": ["--codec", "ak264", "--quality", "draft", "--scale", "1"],
      "ak264-best": ["--codec", "ak264", "--quality", "best"],
      "h264-draft": ["--codec", "h264", "--quality", "draft", "--scale", "1"],
      "h264-good": ["--codec", "h264", "--quality", "good"],
      "h264-best": ["--codec", "h264", "--quality", "best"],
    },
    render: (comp, out, extra) => [AV, ["render", `anikit/${comp}.json`, "-o", out, "--quiet", ...extra], {}],
    // One `still` per frame (true RGB). `render --codec png` is not used: it pipes 4:2:0 YUV to ffmpeg
    // without signalling BT.709, so its PNGs are chroma-subsampled with BT.601-shifted colours.
    reference: (comp, dir) => ["bash", ["-c", `seq 0 ${FRAMES - 1} | xargs -P ${os.cpus().length} -I{} "${AV}" still anikit/${comp}.json --at {}f -o "${dir}/f_{}.png" > /dev/null`], {}],
    still: (comp, out) => [AV, ["still", `anikit/${comp}.json`, "--at", "0", "-o", out], {}],
  },
  remotion: {
    configs: {
      // Remotion defaults: concurrency = half the cores, JPEG frames, x264 CRF 18 / preset medium.
      default: [],
      // Fastest reasonable: every core, x264 veryfast (JPEG frames are already the default).
      fast: ["--concurrency=4", "--x264-preset=veryfast"],
    },
    render: (comp, out, extra) => [path.join(BIN, "remotion"), ["render", "remotion/src/index.ts", comp, out, "--codec=h264",
      "--public-dir=remotion/public", `--browser-executable=${HEADLESS_SHELL}`, "--log=error", ...extra], {}],
    reference: (comp, dir) => [path.join(BIN, "remotion"), ["render", "remotion/src/index.ts", comp, dir, "--sequence", "--image-format=png",
      "--public-dir=remotion/public", `--browser-executable=${HEADLESS_SHELL}`, "--log=error", "--concurrency=4"], {}],
    still: (comp, out) => [path.join(BIN, "remotion"), ["still", "remotion/src/index.ts", comp, out, "--frame=0",
      "--public-dir=remotion/public", `--browser-executable=${HEADLESS_SHELL}`, "--log=error"], {}],
  },
  hyperframes: {
    configs: {
      // HyperFrames defaults: workers auto, quality "looks" (x264 CRF 16).
      default: [],
      // Fastest reasonable: one worker per core, draft quality.
      fast: ["--workers", "4", "--quality", "draft"],
    },
    render: (comp, out, extra) => [path.join(BIN, "hyperframes"), ["render", `hyperframes/${comp}`, "-o", out, "--quiet", ...extra], hfEnv],
    reference: (comp, dir) => [path.join(BIN, "hyperframes"), ["render", `hyperframes/${comp}`, "--format", "png-sequence", "-o", dir, "--quiet", "--workers", "4"], hfEnv],
    still: (comp, out) => [path.join(BIN, "hyperframes"), ["snapshot", `hyperframes/${comp}`, "--at", "0"], hfEnv],
  },
};

// ---------------------------------------------------------------- process-tree sampler
const PAGE = Number(execFileSync("getconf", ["PAGESIZE"]).toString().trim());
function descendants(root) {
  const kids = new Map();
  for (const d of fs.readdirSync("/proc")) {
    if (!/^\d+$/.test(d)) continue;
    try {
      const stat = fs.readFileSync(`/proc/${d}/stat`, "utf8");
      const ppid = Number(stat.slice(stat.lastIndexOf(")") + 2).split(" ")[1]);
      if (!kids.has(ppid)) kids.set(ppid, []);
      kids.get(ppid).push(Number(d));
    } catch {}
  }
  const out = [root];
  for (let i = 0; i < out.length; i++) out.push(...(kids.get(out[i]) || []));
  return out;
}
function treeMemory(root) {
  let rss = 0, pss = 0;
  for (const pid of descendants(root)) {
    try { rss += Number(fs.readFileSync(`/proc/${pid}/statm`, "utf8").split(" ")[1]) * PAGE; } catch {}
    try {
      const m = /^Pss:\s+(\d+) kB/m.exec(fs.readFileSync(`/proc/${pid}/smaps_rollup`, "utf8"));
      if (m) pss += Number(m[1]) * 1024;
    } catch {}
  }
  return { rss, pss };
}

/** Run a command; returns wall seconds, peak tree RSS/PSS, exit code and output tail. */
function measure(cmd, args, env = {}) {
  return new Promise((resolve) => {
    const t0 = process.hrtime.bigint();
    const child = spawn(cmd, args, { cwd: HERE, env: { ...process.env, ...env }, stdio: ["ignore", "pipe", "pipe"] });
    let peakRss = 0, peakPss = 0, log = "";
    const keep = (b) => { log = (log + b.toString()).slice(-4000); };
    child.stdout.on("data", keep);
    child.stderr.on("data", keep);
    const sample = () => { const m = treeMemory(child.pid); peakRss = Math.max(peakRss, m.rss); peakPss = Math.max(peakPss, m.pss); };
    const timer = setInterval(sample, 100);
    child.on("close", (code) => {
      clearInterval(timer);
      resolve({ wall: Number(process.hrtime.bigint() - t0) / 1e9, peakRss, peakPss, code, log });
    });
  });
}

const median = (xs) => { const s = [...xs].sort((a, b) => a - b); return s[Math.floor(s.length / 2)]; };
const sh = (cmd, args) => { try { return execFileSync(cmd, args, { cwd: HERE, stdio: ["ignore", "pipe", "pipe"] }).toString().trim(); } catch (e) { return `n/a (${e.message.split("\n")[0]})`; } };

function probe(file) {
  const j = JSON.parse(execFileSync("ffprobe", ["-v", "error", "-select_streams", "v:0", "-count_frames", "-show_entries",
    "stream=codec_name,profile,width,height,pix_fmt,color_space,color_range,nb_read_frames", "-of", "json", file]).toString());
  return j.streams[0];
}

/** Collect a tool's PNG sequence into ref/<n>.png in frame order, so ffmpeg can read it. */
function normaliseSequence(dir) {
  const files = [];
  const walk = (d) => { for (const f of fs.readdirSync(d, { withFileTypes: true })) { const p = path.join(d, f.name); if (f.isDirectory()) walk(p); else if (/\.png$/i.test(f.name)) files.push(p); } };
  walk(dir);
  files.sort((a, b) => a.localeCompare(b, "en", { numeric: true }));
  const ref = path.join(dir, "ref");
  fs.mkdirSync(ref, { recursive: true });
  files.forEach((f, i) => fs.renameSync(f, path.join(ref, `${String(i + 1).padStart(5, "0")}.png`)));
  return { pattern: path.join(ref, "%05d.png"), count: files.length };
}

/** PSNR (dB, RGB average) and SSIM (all) of an MP4 against a PNG sequence, compared in RGB at 1920×1080.
 *  The MP4 is decoded with its signalled matrix (BT.709 when unsignalled, the HD convention); a smaller
 *  output (draft at half size) is upscaled with bicubic. */
function qualityParse(mp4, ref, info) {
  const matrix = ["bt709", "bt470bg", "smpte170m", "bt2020nc"].includes(info.color_space) ? info.color_space : "bt709";
  const range = info.color_range === "pc" ? "pc" : "tv";
  const fc = `[0:v]scale=${W}:${H}:flags=bicubic:in_color_matrix=${matrix}:in_range=${range},format=gbrp,split[a1][a2];` +
    `[1:v]format=gbrp,split[b1][b2];[a1][b1]psnr;[a2][b2]ssim`;
  const res = spawnSync("ffmpeg", ["-hide_banner", "-nostats", "-i", mp4, "-framerate", String(FPS), "-i", ref, "-lavfi", fc, "-f", "null", "-"],
    { encoding: "utf8", maxBuffer: 1 << 26 }).stderr;
  const psnr = /PSNR .*average:([\d.inf]+)/.exec(res)?.[1];
  const ssim = /SSIM .*All:([\d.]+)/.exec(res)?.[1];
  return { psnr: psnr === "inf" ? Infinity : Number(psnr), ssim: Number(ssim), qualityError: psnr ? undefined : res.slice(-500), decodeMatrix: matrix };
}

// ---------------------------------------------------------------- main
fs.mkdirSync(OUT, { recursive: true });
const resultsFile = path.join(OUT, "results.json");
const results = fs.existsSync(resultsFile) && !flag("fresh") ? JSON.parse(fs.readFileSync(resultsFile, "utf8")) : { rows: [], cold: [] };
const save = () => fs.writeFileSync(resultsFile, JSON.stringify(results, null, 1));

results.machine = {
  date: new Date().toISOString(),
  nproc: os.cpus().length,
  cpu: os.cpus()[0].model,
  memGB: Math.round(os.totalmem() / 2 ** 30),
  kernel: os.release(),
  node: process.version,
  ffmpeg: sh("ffmpeg", ["-version"]).split("\n")[0],
  chrome: sh(HEADLESS_SHELL, ["--version"]),
  anikit: sh("git", ["-C", ROOT, "rev-parse", "--short", "HEAD"]),
  remotion: JSON.parse(fs.readFileSync(path.join(HERE, "node_modules/remotion/package.json"))).version,
  hyperframes: JSON.parse(fs.readFileSync(path.join(HERE, "node_modules/hyperframes/package.json"))).version,
  gsap: JSON.parse(fs.readFileSync(path.join(HERE, "node_modules/gsap/package.json"))).version,
  react: JSON.parse(fs.readFileSync(path.join(HERE, "node_modules/react/package.json"))).version,
};
save();

// Remotion serves fonts from its public dir; copy the bundled Inter there (symlinks are not followed).
fs.mkdirSync(path.join(HERE, "remotion/public"), { recursive: true });
fs.copyFileSync(path.join(HERE, "shared/fonts/InterVariable.ttf"), path.join(HERE, "remotion/public/InterVariable.ttf"));

const log = (...a) => console.error(`[${new Date().toISOString().slice(11, 19)}]`, ...a);

for (const tool of TOOLS) {
  const def = TOOLDEFS[tool];
  for (const comp of COMPS) {
    // Lossless reference (one render, not timed for the tables).
    let ref = null, refreshed = false;
    if (!flag("no-quality")) {
      const dir = path.join(OUT, "ref", tool, comp);
      const pat = path.join(dir, "ref", "%05d.png");
      if (fs.existsSync(path.join(dir, "ref", "00600.png")) && !flag("fresh-ref")) ref = { pattern: pat, count: FRAMES };
      else {
        refreshed = true;
        fs.rmSync(dir, { recursive: true, force: true });
        fs.mkdirSync(dir, { recursive: true });
        const [c, a, e] = def.reference(comp, dir);
        log(tool, comp, "reference PNGs");
        const r = await measure(c, a, e);
        if (r.code !== 0) log("reference failed", r.log);
        ref = normaliseSequence(dir);
        log(`  ${ref.count} frames, ${r.wall.toFixed(1)} s`);
      }
      // Ceiling: the reference through lossless 4:2:0 BT.709 (x264 -qp 0). No 4:2:0 MP4 can beat this.
      if (refreshed) results.ceiling = (results.ceiling || []).filter((c) => !(c.tool === tool && c.comp === comp));
      if (!results.ceiling?.find((c) => c.tool === tool && c.comp === comp)) {
        const ll = path.join(OUT, "ref", tool, comp, "lossless420.mp4");
        spawnSync("ffmpeg", ["-y", "-v", "error", "-framerate", String(FPS), "-i", ref.pattern, "-vf", "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p",
          "-c:v", "libx264", "-qp", "0", "-preset", "ultrafast", "-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709", ll]);
        const q = qualityParse(ll, ref.pattern, probe(ll));
        results.ceiling = [...(results.ceiling || []), { tool, comp, psnr: q.psnr, ssim: q.ssim }];
        log(`  4:2:0 ceiling: PSNR ${q.psnr.toFixed(2)} dB, SSIM ${q.ssim}`);
        save();
      }
    }
    for (const [cfg, extra] of Object.entries(def.configs)) {
      if (ONLY && !ONLY.includes(cfg)) continue;
      const runs = [];
      let out;
      for (let i = 0; i < RUNS; i++) {
        out = path.join(OUT, "mp4", `${tool}-${cfg}-${comp}.mp4`);
        fs.mkdirSync(path.dirname(out), { recursive: true });
        fs.rmSync(out, { force: true });
        const [c, a, e] = def.render(comp, out, extra);
        const r = await measure(c, a, e);
        if (r.code !== 0 || !fs.existsSync(out)) { log(`${tool} ${cfg} ${comp} run ${i + 1} FAILED`, r.log); runs.push({ ...r, failed: true }); continue; }
        log(`${tool} ${cfg} ${comp} run ${i + 1}: ${r.wall.toFixed(2)} s, peak RSS ${(r.peakRss / 2 ** 20).toFixed(0)} MiB`);
        runs.push(r);
      }
      const ok = runs.filter((r) => !r.failed);
      const row = { tool, config: cfg, comp, args: extra, runs: runs.map(({ wall, peakRss, peakPss, failed }) => ({ wall, peakRss, peakPss, failed })) };
      if (ok.length) {
        const info = probe(out);
        Object.assign(row, {
          wall: median(ok.map((r) => r.wall)),
          fps: FRAMES / median(ok.map((r) => r.wall)),
          peakRss: median(ok.map((r) => r.peakRss)),
          peakPss: median(ok.map((r) => r.peakPss)),
          bytes: fs.statSync(out).size,
          video: info,
        });
        if (ref) Object.assign(row, qualityParse(out, ref.pattern, info));
        log(`  => median ${row.wall.toFixed(2)} s (${row.fps.toFixed(0)} fps), ${(row.bytes / 1e6).toFixed(2)} MB, PSNR ${row.psnr?.toFixed?.(2)} dB, SSIM ${row.ssim}`);
      }
      results.rows = results.rows.filter((r) => !(r.tool === tool && r.config === cfg && r.comp === comp));
      results.rows.push(row);
      save();
    }
    // Cold start: one still frame from a fresh process.
    if (!flag("no-cold")) {
      const walls = [];
      for (let i = 0; i < RUNS; i++) {
        const png = path.join(OUT, "stills", `${tool}-${comp}-cold.png`);
        fs.mkdirSync(path.dirname(png), { recursive: true });
        const [c, a, e] = def.still(comp, png);
        const r = await measure(c, a, e);
        if (r.code !== 0) log(`${tool} still failed`, r.log);
        else walls.push(r.wall);
      }
      results.cold = results.cold.filter((r) => !(r.tool === tool && r.comp === comp));
      results.cold.push({ tool, comp, wall: walls.length ? median(walls) : null, runs: walls });
      log(`${tool} ${comp} cold still: ${walls.map((w) => w.toFixed(3)).join(", ")} s`);
      save();
    }
  }
}
log("done →", resultsFile);
