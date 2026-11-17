# Anikit Video: end-to-end benchmarks

How long it takes to go from a composition to a finished 1080p H.264 MP4, with Anikit Video and with the
browser-based tools agents use today (Remotion and HyperFrames), on the same machine, for the same three
videos. Everything here was measured by `bench/video/run.mjs`; nothing is estimated.

## The three compositions

Each is 1920×1080, 30 fps, 20 s (600 frames), dark theme, bundled Inter (variable; "Inter Display" is Inter
at optical size 32 in every tool), with the same layout, colours, eases and timings in every tool.

| | Composition | What moves |
|---|---|---|
| (a) | **Kinetic type title sequence** (`kinetic`) | 4 hard-cut scenes; per-character staggered rises and pop-ins, sliding words, a line that grows, a rotating outline square; a slow camera push-in on every scene, so **every pixel changes on every frame**. |
| (b) | **Product UI** (`product`) | A browser-window frame, 3 stat cards with two counters, a 7-bar chart that builds and later updates, a pulsing "Live" dot, a toast that slides in and out, a cursor that moves to a button that presses. Moves in bursts; long stretches are nearly static. |
| (c) | **Held-heavy explainer** (`explainer`) | 4 slides with 0.5 s crossfades; title and three bullets fade up, then the slide holds while a small dot floats. **Mostly static frames.** |

Sources (each written idiomatically for its tool):

| Tool | Files |
|---|---|
| Anikit Video | `bench/video/anikit/{kinetic,product,explainer}.json` |
| Remotion | `bench/video/remotion/src/{Kinetic,Product,Explainer}.tsx` + `common.tsx`, `Root.tsx`, `index.ts` |
| HyperFrames | `bench/video/hyperframes/{kinetic,product,explainer}/index.html` + `bench/video/shared/base.css` (GSAP timelines) |

Side-by-side frames (Anikit Video | Remotion | HyperFrames, same time code): `bench/video/screens/*.png`.

## Results

<!-- results:start -->
<!-- results:end -->

## Method

- **Wall time** is measured by the runner around the tool's own CLI, from spawn to exit, so it includes
  everything a user waits for: process start, bundling (Remotion's webpack), browser launch, lint
  (HyperFrames), rendering, encoding and muxing. FPS = 600 / wall time.
- **Runs:** every configuration renders 3 times in a row; the tables show the median. Tools run one at a
  time on an otherwise idle machine.
- **Peak memory** is sampled every 100 ms over the whole process tree (the CLI and every child: Chrome
  renderers, ffmpeg, workers). *RSS* sums each process's resident set, which double-counts pages shared
  between Chrome processes; *PSS* splits shared pages between processes and is the fairer number for
  multi-process tools.
- **Quality:** each tool also renders the composition losslessly to PNG (Anikit Video `--codec png`,
  Remotion `--sequence --image-format=png`, HyperFrames `--format png-sequence`). Each MP4 is scored
  against **its own tool's** PNG frames with ffmpeg `psnr` and `ssim` in RGB (`gbrp`), decoding the MP4 with
  its signalled matrix (all outputs signal BT.709, limited range). Half-size draft outputs are upscaled
  with bicubic before scoring. The **4:2:0 ceiling** is the PNG frames through lossless 4:2:0 x264
  (`-qp 0`): chroma subsampling alone costs that much, so no 4:2:0 MP4 can score higher.
- **Cold start** is the wall time of one still frame at t = 0 from a fresh process: `anikit-video still`,
  `remotion still` (bundles the project first) and `hyperframes snapshot` (launches a browser).
- **Settings:**
  - *Anikit Video:* `render spec.json -o out.mp4` (default: `ak264`, `--quality good`, QP 23), plus `--codec
    ak264|h264` × `--quality draft|good|best`. Note that `--quality draft` also renders at **half size**
    (960×540) unless `--scale 1` is given; both are shown. `h264` pipes frames to ffmpeg/libx264
    (draft: ultrafast CRF 26; good: veryfast CRF 20; best: slow CRF 16).
  - *Remotion:* `remotion render … --codec=h264` with its defaults (concurrency = half the cores, JPEG
    frames, x264 CRF 18 preset medium), and "fast": `--concurrency=4 --x264-preset=veryfast`.
  - *HyperFrames:* `hyperframes render` with its defaults (workers `auto`, quality `looks`, CRF 16) and
    "fast": `--workers 4 --quality draft`.
  - Both browser tools use the pre-installed **chrome-headless-shell** (Chrome for Testing, see versions),
    via `--browser-executable` (Remotion) and `HYPERFRAMES_BROWSER_PATH` (HyperFrames); downloads are not
    possible here. HyperFrames uses its faster BeginFrame capture path only with the headless shell; with full
    Chromium it falls back to screenshots (the kinetic composition took 38 s instead of the time below).

## Caveats

- **One machine:** a 4-vCPU cloud VM with no GPU. Browser tools may do relatively better with a GPU or more
  cores (they parallelise across Chrome instances; Anikit Video parallelises across threads).
- **Quality is not matched.** Each tool runs at its own defaults, so file sizes and PSNR differ; compare the
  speed columns together with the size and PSNR columns. Anikit's default `ak264` at `good` trades about
  2–3 dB of PSNR for speed against the browser tools' x264 CRF 16–18; `--quality best` or `--codec h264`
  close that gap, and their times are in the tables too.
- **Each MP4 is scored against its own tool's lossless frames**, so PSNR measures encoder fidelity, not how
  similar the tools' pictures are. Text rasterisation and anti-aliasing differ slightly between Chrome and
  Anikit Video's renderer (vello_cpu); the screenshots show how close the pictures are.
- **Authoring cost** counts the sources as written here, by hand, for one composition. The Remotion and
  HyperFrames versions use loops and helpers (code), while the Anikit specs spell every layer out (data);
  the Anikit specs also avoid its high-level components (`bar-chart`, `browser`, `counter`) so that the
  picture matches the other tools exactly. Using them would make the product spec much shorter.
- **Motion Canvas / Revideo** were not benchmarked: there was no time to port the three compositions to them
  in this session. No numbers are given for them.

## Reproduce

```bash
cargo build --release -p anikit-video
cd bench/video
npm install                      # Remotion, HyperFrames, GSAP, React (versions pinned in package-lock.json)
node run.mjs                     # all tools × compositions × configs, 3 runs each → out/results.json
node run.mjs --tools anikit --comps kinetic --runs 5 --configs default   # a subset
node report.mjs                  # rewrite the tables above from out/results.json
node screens.mjs                 # side-by-side frames → screens/
```

Set `HEADLESS_SHELL=/path/to/chrome-headless-shell` if it is not at the path used here, and
`ANIKIT_VIDEO=/path/to/anikit-video` to benchmark another build. `out/` (MP4s, PNG references, results) is
gitignored; the results of the run shown above are committed as `bench/video/results.json`.
