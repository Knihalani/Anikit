#!/usr/bin/env python3
"""Blind judge for Anikit Video output: compares a piece against style-matched
professional references in a fresh model context that does not know which is which.

Rubric and procedure: docs/video/judge/RUBRIC.md. References: docs/video/judge/refs/.

    python3 scripts/judge.py run CANDIDATE --tags kinetic-type,product-launch [--name x] [--out dir]
    python3 scripts/judge.py control SLUG --tags ...     # a held-out reference judged as the candidate
    python3 scripts/judge.py table results/*.json         # markdown table of verdicts

CANDIDATE is an .mp4, a directory of frames, or a contact sheet (.jpg/.png; pass --grid CxR).

Stage 1 (blind compare): 6 frames sampled evenly in time from the candidate and from each of
2-3 references are normalised identically (640 px longest side, JPEG q80, same 3x2 panel, no
labels), shuffled, and shown as A/B/C/D to a fresh `claude -p` session that can only Read the
panels. It must say which entries are professional, score each on the rubric and name the
three biggest gaps. Repeated over --trials shuffles; the median counts.
Stage 2 (critic, MP4 only): the candidate alone at full resolution, frame pairs 1/6 s apart,
a 100% crop, pace metrics and an audio spectrogram, scored for motion, sound and craft.

Nothing the judge sees carries a name, a path or a hint which entry is ours.
"""
import argparse, json, os, random, re, shutil, statistics, subprocess, sys, tempfile, time
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
# one or more reference dirs (os.pathsep-separated): our refs/ plus REFS's exported benchmark set
BENCH = ROOT / ".cache/judge-bench"  # REFS's benchmark set, rebuilt by `judge.py sync-bench` (stills never in git)
REFS_DIRS = [Path(p) for p in os.environ.get("JUDGE_REFS", os.pathsep.join([str(ROOT / "docs/video/judge/refs"), str(BENCH / "export")])).split(os.pathsep)
             if p and Path(p).is_dir()]
REFS = REFS_DIRS[0]
# our tag -> REFS benchmark category (both are kept on each ref, so either vocabulary matches)
SYNONYMS = {"ident": "brand-idents", "explainer": "explainer-2d", "3d-cg": "cg-3d", "sports-broadcast": "broadcast-sports",
            "editorial-data": "data-story", "vertical": "social-vertical", "title-sequence": "kinetic-type"}
PACE = ROOT / "docs/video/quality/pace.py"
DIMS = ["idea", "composition", "typography", "colour", "light_texture", "motion", "craft", "originality"]
CRITIC_DIMS = ["motion", "sound", "craft"]
N_FRAMES = 6
MIN_FRAMES = int(os.environ.get("JUDGE_MIN_FRAMES", 4))  # refs need this many real frames
CELL = (480, 300)
MODEL = os.environ.get("JUDGE_MODEL", "claude-opus-5-5")

# ---------------------------------------------------------------- frames

def ffprobe(path):
    r = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration:stream=codec_type,width,height,r_frame_rate",
                        "-of", "json", str(path)], capture_output=True, text=True, check=True)
    d = json.loads(r.stdout)
    v = next(s for s in d["streams"] if s["codec_type"] == "video")
    num, den = v["r_frame_rate"].split("/")
    return {"duration": float(d["format"]["duration"]), "w": v["width"], "h": v["height"],
            "fps": float(num) / float(den), "audio": any(s["codec_type"] == "audio" for s in d["streams"])}


def grab(path, t):
    r = subprocess.run(["ffmpeg", "-v", "error", "-ss", f"{t:.3f}", "-i", str(path), "-frames:v", "1",
                        "-f", "image2pipe", "-vcodec", "png", "-"], capture_output=True, check=True)
    from io import BytesIO
    return Image.open(BytesIO(r.stdout)).convert("RGB")


def video_frames(path, n=N_FRAMES):
    """Evenly spaced in time (no cherry-picking: the judge sees what a viewer sees)."""
    d = ffprobe(path)["duration"]
    return [grab(path, d * (i + 0.5) / n) for i in range(n)]


def sheet_frames(path, grid, n=N_FRAMES):
    im = Image.open(path).convert("RGB")
    c, r = (int(x) for x in grid.lower().split("x"))
    w, h = im.width / c, im.height / r
    cells = [im.crop((round(x * w), round(y * h), round((x + 1) * w), round((y + 1) * h))) for y in range(r) for x in range(c)]
    # drop empty/black cells (sheets padded at the end)
    cells = [x for x in cells if np.asarray(x.convert("L")).std() > 2]
    if len(cells) <= n:
        return cells
    return [cells[round(i * (len(cells) - 1) / (n - 1))] for i in range(n)]


def dir_frames(path, n=N_FRAMES):
    files = sorted(p for p in Path(path).iterdir() if p.suffix.lower() in (".jpg", ".jpeg", ".png", ".webp"))
    if len(files) > n:
        files = [files[round(i * (len(files) - 1) / (n - 1))] for i in range(n)]
    return [Image.open(f).convert("RGB") for f in files]


def load_frames(src, grid=None):
    p = Path(src)
    if p.is_dir():
        return dir_frames(p)
    if p.suffix.lower() in (".mp4", ".mov", ".webm", ".mkv"):
        return video_frames(p)
    if not grid:
        sys.exit("a contact sheet needs --grid CxR (columns x rows)")
    return sheet_frames(p, grid)


def trim_bars(im, thr=10):
    """Crop baked-in black letterbox/pillarbox bars (YouTube grabs have them; our renders may too)."""
    a = np.asarray(im.convert("L"), dtype=np.float32)
    rows = np.where(a.mean(1) > thr)[0]
    cols = np.where(a.mean(0) > thr)[0]
    if len(rows) < a.shape[0] * 0.4 or len(cols) < a.shape[1] * 0.4:
        return im  # mostly dark frame: leave it alone
    return im.crop((int(cols[0]), int(rows[0]), int(cols[-1]) + 1, int(rows[-1]) + 1))


def degrade(im, longest=640, q=80):
    """Match the references' resolution and compression so neither gives anything away."""
    from io import BytesIO
    s = longest / max(im.size)
    if s < 1:
        im = im.resize((round(im.width * s), round(im.height * s)), Image.LANCZOS)
    b = BytesIO()
    im.save(b, "JPEG", quality=q)
    return Image.open(BytesIO(b.getvalue())).convert("RGB")


def panel(frames, out):
    cols = 2 if len(frames) == 4 else 3
    rows = (len(frames) + cols - 1) // cols
    W, H = CELL
    gap = 6
    sheet = Image.new("RGB", (cols * W + (cols + 1) * gap, rows * H + (rows + 1) * gap), (24, 24, 24))
    for i, f in enumerate(frames[: cols * rows]):
        f = degrade(trim_bars(f))
        s = min(W / f.width, H / f.height)
        f = f.resize((max(1, round(f.width * s)), max(1, round(f.height * s))), Image.LANCZOS)
        x = gap + (i % cols) * (W + gap) + (W - f.width) // 2
        y = gap + (i // cols) * (H + gap) + (H - f.height) // 2
        sheet.paste(f, (x, y))
    sheet.save(out, "JPEG", quality=85)

# ---------------------------------------------------------------- references

def load_refs():
    """Every <dir>/index.json entry and <dir>/<slug>/ref.json with >= 3 stills, across REFS_DIRS."""
    out, seen = [], set()
    for d in REFS_DIRS:
        refs = {}
        if (d / "index.json").exists():
            refs = {r["slug"]: r for r in json.loads((d / "index.json").read_text())}
        for f in d.glob("*/ref.json"):
            try:
                r = json.loads(f.read_text())
                refs.setdefault(r["slug"], r)
            except (json.JSONDecodeError, KeyError):
                pass
        for r in refs.values():
            if r["slug"] in seen or not (d / r["slug"]).is_dir():
                continue
            if len(list((d / r["slug"]).glob("0*.jpg"))) < MIN_FRAMES:
                continue
            tags = list(r.get("style_tags", []))
            if r.get("category"):
                tags.insert(0, r["category"])
            tags += [SYNONYMS[t] for t in tags if t in SYNONYMS]
            r["style_tags"] = list(dict.fromkeys(tags))
            r["_dir"] = str(d / r["slug"])
            seen.add(r["slug"])
            out.append(r)
    if not out:
        sys.exit(f"no references in {REFS_DIRS}")
    return out


def ref_frames(slug):
    r = next(r for r in load_refs() if r["slug"] == slug)
    return dir_frames(Path(r["_dir"]))


def pick_refs(tags, k, rng, exclude=(), fmt=None):
    refs = [r for r in load_refs() if r["slug"] not in exclude]
    tags = set(tags) | {SYNONYMS[t] for t in tags if t in SYNONYMS}
    def score(r):
        s = 2 * len(tags & set(r["style_tags"]))
        if fmt and r.get("format") == fmt:
            s += 1
        return s + rng.random()  # random tie-break, seeded
    refs.sort(key=score, reverse=True)
    return refs[:k]

# ---------------------------------------------------------------- model calls

def claude(prompt, cwd, max_turns=30):
    t0 = time.time()
    cmd = ["claude", "-p", prompt, "--output-format", "json", "--model", MODEL, "--max-turns", str(max_turns),
           "--allowedTools", "Read", "--disallowedTools", "Bash,Glob,Grep,WebFetch,WebSearch,Write,Edit,Task,Agent,NotebookEdit"]
    r = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=1200)
    try:
        d = json.loads(r.stdout)
    except json.JSONDecodeError:
        raise RuntimeError(f"claude -p failed: {r.stdout[-2000:]} {r.stderr[-2000:]}")
    text = d.get("result", "")
    m = re.findall(r"```json\s*(\{.*?\})\s*```", text, re.S) or re.findall(r"(\{.*\})", text, re.S)
    if not m:
        raise RuntimeError(f"no JSON in answer: {text[-1500:]}")
    u = d.get("usage", {})
    meta = {"seconds": round(time.time() - t0, 1), "cost_usd": d.get("total_cost_usd"),
            "tokens": sum(u.get(k, 0) for k in ("input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")),
            "models": list(d.get("modelUsage", {}).keys())}
    return json.loads(m[-1], strict=False), meta


COMPARE_PROMPT = """You are on the final jury of an international motion-design award (the level of Ciclope, D&AD Pencils, Art of the Title, Vimeo Staff Picks). You have seen thousands of studio reels and you are hard to impress.

In this directory are {n} entries: {files}. Each is ONE short film, shown as a few frames sampled across its length (left to right, top to bottom). Read every file with the Read tool before answering.

Some entries are professional work by top studios. Between 0 and {n} of them are NOT: they may be by an amateur, a template, or an automated / AI video tool. Decide each one on its own merits; do not assume any fixed number of each.
All entries were resized and recompressed identically, so resolution, sharpness of the JPEG and compression are NOT evidence. Judge what was designed: the idea, the image, the type, the light, the motion implied by the frame sequence.

Tells of non-professional work (any one is serious, several are decisive):
- generic "tasteful AI" look: dark or cream ground, small tracked-caps label above a serif or bold-sans headline, a single accent colour, mostly empty frame;
- typography set as defaults (Inter / generic grotesk / generic editorial serif) with no typographic idea, layout, scale contrast or craft; text placed like a slide; "AI slop typography";
- centred headline on a gradient or blurred blob; placeholder UI (pills, cards, grey bars); stock 3D primitives; effect with no subject;
- the same composition repeated across frames (slide-deck structure, one layout with the words swapped); little happening;
- flat, unmotivated light; no texture, material or depth where the subject wants it; illustration that is clip-art-like, symmetrical, vector-default;
- an idea that is literal and obvious (the product name + a diagram of the feature), nothing a viewer would remember or share.
Tells of top-tier work: a specific idea only this piece could have; dense, layered, considered frames; typography with a point of view (custom or carefully chosen faces, expressive scale, integration with image); a deliberate, unusual palette; real light and texture; every frame a poster; evidence of choreography between frames; surprise.

Scale for every score (integers 1-10): 10 = among the best pieces of the year; 8 = strong studio work; 6 = competent professional, unremarkable; 4 = template / student work; 2 = obvious AI or automated output.
Dimensions: idea, composition, typography (if no type is visible score how well its absence serves the piece, and say so), colour, light_texture, motion (inferred from how the frames change: choreography, variety, weight), craft (finish, detail, consistency), originality.

Answer with ONLY one fenced ```json block of this shape:
{{"entries": {{"A": {{"verdict": "professional" | "not professional", "confidence": 0.0-1.0,
  "scores": {{"idea": n, "composition": n, "typography": n, "colour": n, "light_texture": n, "motion": n, "craft": n, "originality": n}},
  "tells": ["specific observations that drove the verdict"],
  "gaps": ["the 3 biggest gaps between this entry and the best entry, specific and actionable"]}}, ...}},
 "ranking": ["best", "...", "worst"]}}"""


CRITIC_PROMPT = """You are a senior motion director reviewing one short film submitted to a top motion-design award. Be exact and hard to impress; the bar is the best studios in the world.

Files here (read every one with the Read tool):
- frames.jpg: 6 frames sampled evenly in time.
- pairs.jpg: 3 rows; each row is two frames 1/6 s apart, to judge motion: spacing, easing, weight, motion blur, choreography.
- strip1.jpg, strip2.jpg, strip3.jpg (if present): the three busiest moments, each 10 consecutive frames 1/10 s apart
  (left to right, top to bottom). Read the motion from them: spacing and easing (do positions accelerate and settle, or
  move linearly?), overlap and follow-through, weight, how elements hand off to each other, how cuts and transitions are built.
- crop.jpg: a 100% (1:1 pixel) crop of the most detailed region of one frame, to judge finish: type rendering, edges, banding, noise, aliasing.
{audio_line}
Measured pace (shot lengths, change per second):
{pace}
World-class median shot for this kind of piece (REFS benchmark): {pace_hint}. Judge dead air (anything static > ~1 s without a reason) more than cut count.

Scale (integers 1-10): 10 = among the best of the year; 8 = strong studio work; 6 = competent professional; 4 = template / student; 2 = obvious AI or automated output.
Score: motion (choreography, timing, weight, variety of movement, transitions), sound ({sound_rule}), craft (finish at 100%, defects: stepping, banding, aliasing, clipped or soft text, pops).

Also answer these yes/no checks strictly (yes only if clearly true from what you see):
- idea: can you state it as "X shown as Y" where Y is not just "nice animation" or "kinetic text"?
- invariant: is there one rule/material/device held across every shot, while the rest varies?
- arc: does energy build to a peak at 60-80% and release (not flat, not one move repeated)?
- type_meaning: is some type property (weight, width, size, placement, timing) mapped to meaning, beyond fade/slide in?
- freeze_frame: would each of the 6 frames work as a poster?
- ending: does the end pay off and settle, ideally rhyming with the opening?
- restraint: one subject per frame, 2-4 colours plus an accent, one deliberate type family?
- material: at the 100% crop, is there texture and light with falloff?
- sync: do visible impacts/cuts appear to land on audio transients? (null if silent)
Checks never raise a score; a "no" is a gap.

Answer with ONLY one fenced ```json block:
{{"scores": {{"motion": n, "sound": n or null, "craft": n}}, "checks": {{"idea": bool, "invariant": bool, "arc": bool, "type_meaning": bool, "freeze_frame": bool, "ending": bool, "restraint": bool, "material": bool, "sync": bool or null}},
 "defects": ["..."], "gaps": ["the 3 biggest motion/sound/finish gaps vs top studio work"]}}"""

# ---------------------------------------------------------------- stage 1

def compare(cand_frames, refs, trials, seed, workdir):
    entries = [("candidate", cand_frames)] + [(r["slug"], ref_frames(r["slug"])) for r in refs]
    # same number of frames for every entry, so the count gives nothing away
    k = min(len(fr) for _, fr in entries)
    entries = [(n, fr if len(fr) == k else [fr[round(i * (len(fr) - 1) / max(1, k - 1))] for i in range(k)]) for n, fr in entries]
    out = []
    for t in range(trials):
        rng = random.Random(seed * 1000 + t)
        order = entries[:]
        rng.shuffle(order)
        blind = Path(tempfile.mkdtemp(prefix="j"))  # judge cwd: only the panels
        letters = "ABCDEFG"[: len(order)]
        key = {}
        for L, (name, fr) in zip(letters, order):
            panel(fr, blind / f"{L}.jpg")
            key[L] = name
        files = ", ".join(f"{L}.jpg" for L in letters)
        ans, meta = claude(COMPARE_PROMPT.format(n=len(order), files=files), blind)
        shutil.copytree(blind, workdir / f"trial{t}", dirs_exist_ok=True)
        shutil.rmtree(blind, ignore_errors=True)
        out.append({"key": key, "answer": ans, "meta": meta})
        print(f"  trial {t}: {key}  ranking={ans.get('ranking')}  {meta['seconds']}s ${meta['cost_usd']}", file=sys.stderr)
    return out


def summarise(trials):
    per = {}
    for tr in trials:
        for L, name in tr["key"].items():
            e = tr["answer"]["entries"].get(L)
            if not e:
                continue
            p = per.setdefault(name, {"scores": {d: [] for d in DIMS}, "pro": [], "gaps": [], "tells": [], "rank": []})
            for d in DIMS:
                v = e.get("scores", {}).get(d)
                if isinstance(v, (int, float)):
                    p["scores"][d].append(v)
            p["pro"].append(1 if str(e.get("verdict", "")).startswith("professional") else 0)
            p["gaps"] += e.get("gaps", [])
            p["tells"] += e.get("tells", [])
            rk = tr["answer"].get("ranking", [])
            if L in rk:
                p["rank"].append(rk.index(L) + 1)
    res = {}
    for name, p in per.items():
        med = {d: statistics.median(v) for d, v in p["scores"].items() if v}
        res[name] = {"scores": med, "overall": round(statistics.mean(med.values()), 2) if med else None,
                     "judged_professional": f"{sum(p['pro'])}/{len(p['pro'])}", "pro_rate": sum(p["pro"]) / max(1, len(p["pro"])),
                     "mean_rank": round(statistics.mean(p["rank"]), 2) if p["rank"] else None,
                     "gaps": p["gaps"], "tells": p["tells"]}
    return res

# ---------------------------------------------------------------- stage 2

PACE_HINTS = [("broadcast-sports", "about 1.0 s"), ("social-vertical", "1.5 s or less"), ("brand-idents", "1.5 s or less"),
              ("ui-product", "1.5 s or less"), ("product-launch", "about 2 s"), ("explainer-2d", "3-4 s with in-shot motion"),
              ("kinetic-type", "3-4 s with in-shot motion"), ("title-sequence", "3-4 s with in-shot motion")]


def pace_hint(tags):
    tags = set(tags) | {SYNONYMS[t] for t in tags if t in SYNONYMS}
    return next((h for t, h in PACE_HINTS if t in tags), "about 2.5 s overall, every shot alive")


def motion_strips(src, n_strips=3, n=10, fps=10, w=320):
    """Strips of n consecutive frames (1/fps s apart) around the n_strips busiest moments (by frame difference),
    at least 2 s apart. Each strip is one image, 5 frames per row, read left to right, top to bottom."""
    info = ffprobe(src)
    sw = 160
    sh = round(sw * info["h"] / info["w"]) // 2 * 2
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", str(src), "-vf", f"fps={fps},scale={sw}:{sh}", "-f", "rawvideo",
                          "-pix_fmt", "gray", "-"], capture_output=True, check=True).stdout
    fr = np.frombuffer(raw, np.uint8).reshape(-1, sh, sw).astype(np.float32)
    if len(fr) < n + 2:
        return []
    d = np.abs(np.diff(fr, axis=0)).mean(axis=(1, 2))
    # busiest window of n frames, excluding the first/last half second
    win = np.convolve(d, np.ones(n - 1), "valid")
    order = np.argsort(-win)
    picks = []
    for i in order:
        if i < fps // 2 or i + n > len(fr) - fps // 2:
            continue
        if all(abs(i - j) >= 2 * fps for j in picks):
            picks.append(int(i))
        if len(picks) == n_strips:
            break
    strips = []
    for i in sorted(picks):
        t0 = i / fps
        frames = [grab(src, t0 + k / fps) for k in range(n)]
        frames = [trim_bars(f) for f in frames]
        fw = w; fh = round(w * frames[0].height / frames[0].width)
        cols = 5
        img = Image.new("RGB", (cols * fw + (cols + 1) * 4, ((n + cols - 1) // cols) * (fh + 4) + 4), (24, 24, 24))
        for k, f in enumerate(frames):
            img.paste(f.resize((fw, fh), Image.LANCZOS), (4 + (k % cols) * (fw + 4), 4 + (k // cols) * (fh + 4)))
        strips.append((round(t0, 1), img))
    return strips


CLIPS = Path(os.environ.get("JUDGE_CLIPS", ROOT / ".cache/judge-clips"))  # <slug>.mp4, lawfully obtained (clips.json)

MOTION_PROMPT = """You are on the final jury of an international motion-design award, judging MOTION only.

In this directory are {n} entries: {files}. Each is ONE short film, shown as two strips. A strip is 10 consecutive
frames 1/10 s apart (left to right, top to bottom), taken at that film's two busiest moments. Read every file.
Some are by top studios; between 0 and {n} are not (amateur, template or automated). Do not assume a quota.
Ignore subject, style, palette and image resolution. Judge only how things move:
- spacing and easing: do things accelerate, overshoot and settle, or slide linearly / pop on and freeze?
- choreography: do elements hand off, overlap and follow through, or does one thing move while the rest holds still?
- weight and physicality; camera language; how cuts and transitions are built (match cuts, morphs, wipes with intent).
- life: is anything alive in every frame, or are there dead holds?
Scale (integers 1-10): 10 = best of the year; 8 = strong studio; 6 = competent; 4 = template; 2 = automated.

Answer with ONLY one fenced ```json block:
{{"entries": {{"A": {{"verdict": "professional" | "not professional", "confidence": 0.0-1.0, "motion": n,
  "tells": ["..."], "gaps": ["3 biggest motion gaps vs the best entry"]}}, ...}}, "ranking": ["best", "...", "worst"]}}"""


def clip_refs(tags, k, rng, exclude=()):
    have = {p.stem for p in CLIPS.glob("*.mp4")} - set(exclude)
    if len(have) < 2:
        return []
    refs = [r for r in load_refs() if r["slug"] in have]
    known = {r["slug"] for r in refs}
    refs += [{"slug": s_, "style_tags": []} for s_ in have - known]  # clips without stills still count
    tags = set(tags) | {SYNONYMS[t] for t in tags if t in SYNONYMS}
    refs.sort(key=lambda r: 2 * len(tags & set(r["style_tags"])) + rng.random(), reverse=True)
    return refs[:k]


def strip_sheet(src, out):
    st = motion_strips(src, n_strips=2)
    if not st:
        return False
    W = 1200
    ims = [degrade(img, longest=W, q=80) for _, img in st]
    sheet = Image.new("RGB", (W, sum(i.height for i in ims) + 12 * (len(ims) - 1)), (24, 24, 24))
    y = 0
    for im in ims:
        sheet.paste(im, (0, y)); y += im.height + 12
    sheet.save(out, "JPEG", quality=85)
    return True


def motion_compare(src, tags, trials, seed, workdir, exclude=()):
    refs = clip_refs(tags, 3, random.Random(seed), exclude)
    if len(refs) < 2:
        return None
    entries = [("candidate", src)] + [(r["slug"], CLIPS / f"{r['slug']}.mp4") for r in refs]
    out = []
    for t in range(trials):
        rng = random.Random(seed * 1000 + 500 + t)
        order = entries[:]
        rng.shuffle(order)
        blind = Path(tempfile.mkdtemp(prefix="m"))
        key = {}
        for L, (name, path) in zip("ABCDEFG", order):
            if strip_sheet(path, blind / f"{L}.jpg"):
                key[L] = name
        files = ", ".join(f"{L}.jpg" for L in key)
        ans, meta = claude(MOTION_PROMPT.format(n=len(key), files=files), blind)
        shutil.copytree(blind, workdir / f"motion{t}", dirs_exist_ok=True)
        shutil.rmtree(blind, ignore_errors=True)
        out.append({"key": key, "answer": ans, "meta": meta})
    per = {}
    for tr in out:
        for L, name in tr["key"].items():
            e = tr["answer"]["entries"].get(L, {})
            p = per.setdefault(name, {"m": [], "pro": [], "gaps": []})
            if isinstance(e.get("motion"), (int, float)):
                p["m"].append(e["motion"])
            p["pro"].append(1 if str(e.get("verdict", "")).startswith("professional") else 0)
            p["gaps"] += e.get("gaps", [])
    summ = {n: {"motion": statistics.median(p["m"]) if p["m"] else None, "judged_professional": f"{sum(p['pro'])}/{len(p['pro'])}",
                "gaps": p["gaps"]} for n, p in per.items()}
    rm = [v["motion"] for n, v in summ.items() if n != "candidate" and v["motion"] is not None]
    return {"refs": [r["slug"] for r in refs], "candidate": summ.get("candidate"), "refs_summary": {n: v for n, v in summ.items() if n != "candidate"},
            "ref_mean": round(statistics.mean(rm), 2) if rm else None, "trials": out,
            "cost_usd": round(sum(t["meta"]["cost_usd"] or 0 for t in out), 3)}


def critic(src, workdir, tags=()):
    info = ffprobe(src)
    blind = Path(tempfile.mkdtemp(prefix="c"))
    fr = video_frames(src)
    W = 640
    sheet = Image.new("RGB", (3 * W, 2 * round(W * info["h"] / info["w"])))
    for i, f in enumerate(fr):
        f = f.resize((W, round(W * f.height / f.width)), Image.LANCZOS)
        sheet.paste(f, ((i % 3) * W, (i // 3) * f.height))
    sheet.save(blind / "frames.jpg", quality=88)
    dt = 1 / 6
    rows = []
    for i in range(3):
        t = info["duration"] * (i + 1) / 4
        a, b = grab(src, t), grab(src, min(t + dt, info["duration"] - 0.05))
        a = a.resize((W, round(W * a.height / a.width))); b = b.resize(a.size)
        row = Image.new("RGB", (2 * W, a.height)); row.paste(a, (0, 0)); row.paste(b, (W, 0)); rows.append(row)
    pairs = Image.new("RGB", (2 * W, sum(r.height for r in rows)))
    y = 0
    for r in rows:
        pairs.paste(r, (0, y)); y += r.height
    pairs.save(blind / "pairs.jpg", quality=88)
    if os.environ.get("JUDGE_STRIPS", "0") == "1":
        for k, (t0, img) in enumerate(motion_strips(src)):
            img.save(blind / f"strip{k + 1}.jpg", quality=85)
    # 100% crop of the most detailed 640x400 window across the frames
    best = None
    for f in fr:
        g = np.asarray(f.convert("L").filter(ImageFilter.FIND_EDGES), dtype=np.float32)
        cw, ch = min(640, f.width), min(400, f.height)
        for y0 in range(0, f.height - ch + 1, max(1, ch // 2)):
            for x0 in range(0, f.width - cw + 1, max(1, cw // 2)):
                s = g[y0:y0 + ch, x0:x0 + cw].mean()
                if best is None or s > best[0]:
                    best = (s, f.crop((x0, y0, x0 + cw, y0 + ch)))
    best[1].save(blind / "crop.jpg", quality=95)
    pace = subprocess.run([sys.executable, str(PACE), str(src)], capture_output=True, text=True).stdout.strip()[-1500:] or "(unavailable)"
    audio_line, sound_rule = "", "null: the film is silent"
    if info["audio"]:
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(src), "-lavfi", "showspectrumpic=s=1280x360:legend=1:scale=log",
                        str(blind / "spectrum.png")], check=False)
        lufs = subprocess.run(["ffmpeg", "-hide_banner", "-i", str(src), "-af", "ebur128", "-f", "null", "-"],
                              capture_output=True, text=True).stderr
        m = re.search(r"I:\s+(-?[\d.]+) LUFS", lufs.split("Summary:")[-1]) if "Summary:" in lufs else None
        audio_line = (f"- spectrum.png: spectrogram of the soundtrack (integrated loudness {m.group(1) if m else '?'} LUFS). "
                      "You cannot hear it: infer arrangement density, dynamics, hit sync with cuts, and whether it is "
                      "thin generated synth (few partials, static) or produced music and sound design.")
        sound_rule = "from the spectrogram: arrangement, dynamics, sync to picture, sound design; be conservative"
    ans, meta = claude(CRITIC_PROMPT.format(audio_line=audio_line, pace=pace, sound_rule=sound_rule, pace_hint=pace_hint(tags)), blind)
    shutil.copytree(blind, workdir / "critic", dirs_exist_ok=True)
    shutil.rmtree(blind, ignore_errors=True)
    return {"answer": ans, "meta": meta, "pace": pace}

# ---------------------------------------------------------------- commands

def judge(name, frames, tags, args, exclude=(), video=None):
    if args.trials < 1:
        sys.exit("--trials must be >= 1")
    rng = random.Random(args.seed)
    refs = pick_refs(tags, args.refs, rng, exclude=exclude, fmt=args.format)
    out = Path(args.out or ROOT / "docs/video/judge/results") / name
    out.mkdir(parents=True, exist_ok=True)
    print(f"judging {name} vs {[r['slug'] for r in refs]}", file=sys.stderr)
    trials = compare(frames, refs, args.trials, args.seed, out)
    summ = summarise(trials)
    cand = summ.get("candidate", {})
    ref_scores = [summ[r["slug"]]["overall"] for r in refs if summ.get(r["slug"], {}).get("overall") is not None]
    ref_mean = round(statistics.mean(ref_scores), 2) if ref_scores else None
    result = {"name": name, "tags": tags, "refs": [r["slug"] for r in refs], "candidate": cand,
              "refs_summary": {r["slug"]: {k: summ.get(r["slug"], {}).get(k) for k in ("overall", "judged_professional", "mean_rank")} for r in refs},
              "ref_mean": ref_mean, "delta": round(cand["overall"] - ref_mean, 2) if ref_mean and cand.get("overall") else None,
              "trials": trials}
    if video and not args.no_critic:
        c = critic(video, out, tags)
        result["critic"] = c
        s = c["answer"].get("scores", {})
        dims = dict(cand.get("scores", {}))
        for d in ("motion", "craft"):
            if isinstance(s.get(d), (int, float)) and d in dims:
                dims[d] = (dims[d] + s[d]) / 2
        if isinstance(s.get("sound"), (int, float)):
            dims["sound"] = s["sound"]
        result["full_scores"] = dims
        result["full_overall"] = round(statistics.mean(dims.values()), 2)
    if video and os.environ.get("JUDGE_MOTION", "0") == "1":
        m = motion_compare(video, tags, args.trials, args.seed, out, exclude=exclude)
        if m:
            result["motion_blind"] = m
            print(f"  motion: cand {m['candidate']['motion']} ({m['candidate']['judged_professional']}) vs refs {m['ref_mean']}", file=sys.stderr)
    meta = [t["meta"] for t in trials] + ([result["critic"]["meta"]] if "critic" in result else [])
    result["cost_usd"] = round(sum(m["cost_usd"] or 0 for m in meta), 3)
    result["judge_seconds"] = round(sum(m["seconds"] for m in meta), 1)
    result["verdict"] = verdict(cand, result["delta"])
    (out / "result.json").write_text(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ("name", "verdict", "ref_mean", "delta", "cost_usd")} |
                     {"cand_overall": cand.get("overall"), "judged_pro": cand.get("judged_professional")}, indent=None))
    return result


def verdict(cand, delta):
    """PASS: indistinguishable from the style-matched refs. PRO: passes as real studio work but below the refs.
    FAIL: the jury can tell. Thresholds come from controls (held-out pro refs): every control is PRO or PASS."""
    pro = cand.get("pro_rate", 0) >= 0.6
    if pro and delta is not None and delta >= -1.0:
        return "PASS"
    if pro and (cand.get("overall") or 0) >= 5.0:
        return "PRO"
    return "FAIL"


def cmd_run(args):
    frames = load_frames(args.candidate, args.grid)
    name = args.name or Path(args.candidate).stem
    video = args.candidate if Path(args.candidate).suffix.lower() in (".mp4", ".mov", ".webm", ".mkv") else None
    judge(name, frames, args.tags.split(","), args, video=video)


def cmd_control(args):
    """False-positive check: a held-out professional reference takes the candidate's seat."""
    refs = {r["slug"]: r for r in load_refs()}
    r = refs[args.slug]
    tags = args.tags.split(",") if args.tags else r["style_tags"]
    clip = CLIPS / f"{args.slug}.mp4"
    judge(f"control-{args.slug}", ref_frames(args.slug), tags, args, exclude=(args.slug,),
          video=str(clip) if clip.exists() else None)


def cmd_table(args):
    rows = [json.loads(Path(p).read_text()) for p in args.results]
    print("| Piece | Verdict | Judged pro | Score | Refs mean | Δ | Full (incl. motion/sound) | Top gaps |")
    print("|---|---|---|---|---|---|---|---|")
    for r in rows:
        c = r["candidate"]
        gaps = "; ".join(g for g in c.get("gaps", [])[:3])
        print(f"| {r['name']} | {r['verdict']} | {c.get('judged_professional')} | {c.get('overall')} | {r['ref_mean']} | {r['delta']} | "
              f"{r.get('full_overall', '–')} | {gaps} |")


def resolve_src(src):
    """A repo path, an absolute path, or "git:<branch>:<path>" (fetched from origin into a temp file)."""
    if src.startswith("git:"):
        _, branch, path = src.split(":", 2)
        subprocess.run(["git", "-C", str(ROOT), "fetch", "-q", "origin", f"{branch}:refs/remotes/origin/{branch}"], check=False)
        blob = subprocess.run(["git", "-C", str(ROOT), "show", f"origin/{branch}:{path}"], capture_output=True, check=True).stdout
        dst = Path(tempfile.mkdtemp(prefix="src")) / Path(path).name
        dst.write_bytes(blob)
        return str(dst)
    return str(ROOT / src) if not os.path.isabs(src) else src


def cmd_sync_bench(args):
    """Fetch REFS's benchmark (branch claude/epic-ramanujan-qe7x4w-v-refs) and export its stills to .cache/judge-bench/export."""
    br = "claude/epic-ramanujan-qe7x4w-v-refs"
    subprocess.run(["git", "-C", str(ROOT), "fetch", "-q", "origin", f"{br}:refs/remotes/origin/{br}"], check=True)
    src = BENCH / "src"
    shutil.rmtree(src, ignore_errors=True); src.mkdir(parents=True)
    tar = subprocess.run(["git", "-C", str(ROOT), "archive", f"origin/{br}", "docs/video/judge/benchmark"], capture_output=True, check=True).stdout
    subprocess.run(["tar", "-x", "-C", str(src)], input=tar, check=True)
    env = dict(os.environ, ANIKIT_BENCH_CACHE=str(BENCH / "cache"))
    tool = src / "docs/video/judge/benchmark/tools/bench.py"
    subprocess.run([sys.executable, "-I", str(tool), "fetch", "--refresh"], cwd=src, env=env)
    shutil.rmtree(BENCH / "export", ignore_errors=True)
    subprocess.run([sys.executable, "-I", str(tool), "export", "--no-thumbs", "--out", str(BENCH / "export")], cwd=src, env=env, check=True)


def cmd_batch(args):
    """Judge every item of a manifest: [{"name","src","tags","format"?,"grid"?}], paths relative to the repo."""
    items = json.loads(Path(args.manifest).read_text())
    if args.only:
        items = [i for i in items if i["name"] in args.only.split(",")]
    for it in items:
        if args.skip_done and (Path(args.out or ROOT / "docs/video/judge/results") / it["name"] / "result.json").exists():
            continue
        src = resolve_src(it["src"])
        a = argparse.Namespace(**vars(args))
        a.format, a.grid = it.get("format"), it.get("grid")
        try:
            frames = load_frames(src, a.grid)
            video = src if Path(src).suffix.lower() in (".mp4", ".mov", ".webm", ".mkv") else None
            judge(it["name"], frames, it["tags"], a, video=video)
        except Exception as e:  # keep going; one bad item must not stop a round
            print(f"FAILED {it['name']}: {e}", file=sys.stderr)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for n in ("run", "control"):
        p = sub.add_parser(n)
        p.add_argument("candidate" if n == "run" else "slug")
        p.add_argument("--tags", required=(n == "run"), help="style tags, comma separated (see refs/INDEX.md)")
        p.add_argument("--format", help="preferred ref format, e.g. 16:9, 9:16, 1:1")
        p.add_argument("--grid", help="contact sheet layout CxR")
        p.add_argument("--name")
        p.add_argument("--refs", type=int, default=3)
        p.add_argument("--trials", type=int, default=3)
        p.add_argument("--seed", type=int, default=7)
        p.add_argument("--out")
        p.add_argument("--no-critic", action="store_true")
    b = sub.add_parser("batch")
    b.add_argument("manifest")
    b.add_argument("--only")
    b.add_argument("--skip-done", action="store_true")
    for k, v in (("--refs", 3), ("--trials", 3), ("--seed", 7)):
        b.add_argument(k, type=int, default=v)
    b.add_argument("--out")
    b.add_argument("--no-critic", action="store_true")
    sub.add_parser("sync-bench")
    t = sub.add_parser("table")
    t.add_argument("results", nargs="+")
    args = ap.parse_args()
    {"run": cmd_run, "control": cmd_control, "table": cmd_table, "batch": cmd_batch, "sync-bench": cmd_sync_bench}[args.cmd](args)


if __name__ == "__main__":
    main()
