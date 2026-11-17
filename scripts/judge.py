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
REFS = Path(os.environ.get("JUDGE_REFS", ROOT / "docs/video/judge/refs"))
PACE = ROOT / "docs/video/quality/pace.py"
DIMS = ["idea", "composition", "typography", "colour", "light_texture", "motion", "craft", "originality"]
CRITIC_DIMS = ["motion", "sound", "craft"]
N_FRAMES = 6
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
    cols, rows = 3, 2
    W, H = CELL
    gap = 6
    sheet = Image.new("RGB", (cols * W + (cols + 1) * gap, rows * H + (rows + 1) * gap), (24, 24, 24))
    for i, f in enumerate(frames[: cols * rows]):
        f = degrade(f)
        s = min(W / f.width, H / f.height)
        f = f.resize((max(1, round(f.width * s)), max(1, round(f.height * s))), Image.LANCZOS)
        x = gap + (i % cols) * (W + gap) + (W - f.width) // 2
        y = gap + (i // cols) * (H + gap) + (H - f.height) // 2
        sheet.paste(f, (x, y))
    sheet.save(out, "JPEG", quality=85)

# ---------------------------------------------------------------- references

def load_refs():
    """refs/index.json, plus any refs/<slug>/ref.json not yet in it."""
    refs = {}
    idx = REFS / "index.json"
    if idx.exists():
        refs = {r["slug"]: r for r in json.loads(idx.read_text())}
    for f in REFS.glob("*/ref.json"):
        try:
            r = json.loads(f.read_text())
            refs.setdefault(r["slug"], r)
        except (json.JSONDecodeError, KeyError):
            pass
    out = []
    for r in refs.values():
        r.setdefault("style_tags", [])
        if r.get("category") and r["category"] not in r["style_tags"]:
            r["style_tags"] = [r["category"]] + list(r["style_tags"])
        if (REFS / r["slug"]).is_dir() and len(list((REFS / r["slug"]).glob("0*.jpg"))) >= 3:
            out.append(r)
    if not out:
        sys.exit(f"no references in {REFS}")
    return out


def ref_frames(slug):
    return dir_frames(REFS / slug)


def pick_refs(tags, k, rng, exclude=(), fmt=None):
    refs = [r for r in load_refs() if r["slug"] not in exclude]
    tags = set(tags)
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
    return json.loads(m[-1]), meta


COMPARE_PROMPT = """You are on the final jury of an international motion-design award (the level of Ciclope, D&AD Pencils, Art of the Title, Vimeo Staff Picks). You have seen thousands of studio reels and you are hard to impress.

In this directory are {n} entries: {files}. Each is ONE short film, shown as 6 frames sampled evenly in time (left to right, top to bottom). Read every file with the Read tool before answering.

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
- crop.jpg: a 100% (1:1 pixel) crop of the most detailed region of one frame, to judge finish: type rendering, edges, banding, noise, aliasing.
{audio_line}
Measured pace (shot lengths, change per second):
{pace}

Scale (integers 1-10): 10 = among the best of the year; 8 = strong studio work; 6 = competent professional; 4 = template / student; 2 = obvious AI or automated output.
Score: motion (choreography, timing, weight, variety of movement, transitions), sound ({sound_rule}), craft (finish at 100%, defects: stepping, banding, aliasing, clipped or soft text, pops).

Also answer these yes/no checks strictly (yes only if clearly true from what you see):
- idea: can you state it as "X shown as Y" where Y is not just "nice animation" or "kinetic text"?
- invariant: is there one rule/material/device held across every shot, while the rest varies?
- arc: does energy build to a peak at 60-80% and release (not flat, not one move repeated)?
- type_meaning: is some type property (weight, width, size, placement, timing) mapped to meaning, beyond fade/slide in?
- freeze_frame: would each of the 6 frames work as a poster?
- ending: does the end pay off and settle, ideally rhyming with the opening?
- sync: do visible impacts/cuts appear to land on audio transients? (null if silent)
Checks never raise a score; a "no" is a gap.

Answer with ONLY one fenced ```json block:
{{"scores": {{"motion": n, "sound": n or null, "craft": n}}, "checks": {{"idea": bool, "invariant": bool, "arc": bool, "type_meaning": bool, "freeze_frame": bool, "ending": bool, "sync": bool or null}},
 "defects": ["..."], "gaps": ["the 3 biggest motion/sound/finish gaps vs top studio work"]}}"""

# ---------------------------------------------------------------- stage 1

def compare(cand_frames, refs, trials, seed, workdir):
    entries = [("candidate", cand_frames)] + [(r["slug"], ref_frames(r["slug"])) for r in refs]
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

def critic(src, workdir):
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
    ans, meta = claude(CRITIC_PROMPT.format(audio_line=audio_line, pace=pace, sound_rule=sound_rule), blind)
    shutil.copytree(blind, workdir / "critic", dirs_exist_ok=True)
    shutil.rmtree(blind, ignore_errors=True)
    return {"answer": ans, "meta": meta, "pace": pace}

# ---------------------------------------------------------------- commands

def judge(name, frames, tags, args, exclude=(), video=None):
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
        c = critic(video, out)
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
    judge(f"control-{args.slug}", ref_frames(args.slug), tags, args, exclude=(args.slug,))


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
    t = sub.add_parser("table")
    t.add_argument("results", nargs="+")
    args = ap.parse_args()
    {"run": cmd_run, "control": cmd_control, "table": cmd_table, "batch": cmd_batch}[args.cmd](args)


if __name__ == "__main__":
    main()
