#!/usr/bin/env python3
"""Benchmark-set helper for the JUDGE (and anyone checking taste).

    python3 docs/video/judge/benchmark/tools/bench.py index            # rebuild pieces.json + INDEX.md tables
    python3 docs/video/judge/benchmark/tools/bench.py verify [slug..]  # re-check URLs via oEmbed
    python3 docs/video/judge/benchmark/tools/bench.py fetch [slug..]   # download official stills into the cache
    python3 docs/video/judge/benchmark/tools/bench.py match --spec our.json [--analysis our.analysis.json]
                                          [--tags "medium=3d-cg pace=fast purpose=launch"] [--text "..."] [-n 3]
    python3 docs/video/judge/benchmark/tools/bench.py export --out DIR  # JUDGE layout: DIR/index.json + DIR/<slug>/01.jpg…
    python3 docs/video/judge/benchmark/tools/bench.py stats            # numbers for WHAT-WORLD-CLASS-LOOKS-LIKE.md
    python3 docs/video/judge/benchmark/tools/bench.py measure slug clip.mp4   # run `anikit-video analyze`, store stats

Stills and clips live OUTSIDE git, in $ANIKIT_BENCH_CACHE (default: <repo>/.cache/benchmark/, gitignored).
They are for evaluation only and are never committed. Standard library only.
"""
import argparse, json, os, re, subprocess, sys, urllib.parse, urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent                      # docs/video/judge/benchmark
REPO = ROOT.parents[3]
CACHE = Path(os.environ.get("ANIKIT_BENCH_CACHE", REPO / ".cache" / "benchmark"))

CATEGORIES = [
    ("kinetic-type", "Kinetic typography & title sequences"),
    ("product-launch", "Product launch films (hardware)"),
    ("brand-idents", "Brand identity motion & idents"),
    ("explainer-2d", "2D motion graphics & explainers"),
    ("ui-product", "Software / UI launch films"),
    ("illustration", "Illustration-led animation"),
    ("cg-3d", "3D / CG product & abstract"),
    ("generative", "Generative, simulation & shader work"),
    ("data-story", "Data storytelling"),
    ("music-video", "Music videos & lyric videos"),
    ("social-vertical", "Social-first 9:16 / 1:1"),
    ("broadcast-sports", "Sports & broadcast packages"),
    ("event-promo", "Festival, conference & event promos"),
    ("award-shorts", "Award-winning commissioned & festival shorts"),
]
PACE = ["calm", "steady", "fast", "frantic"]
TYPE_ROLE = ["none", "supporting", "lead", "only"]

# ---------------------------------------------------------------- front matter

def _scalar(v):
    v = v.strip()
    if v == "" or v in ("null", "~"):
        return None
    if v.startswith("{"):
        return json.loads(v)
    if v.startswith("["):
        inner = v[1:-1].strip()
        if not inner:
            return []
        out, cur, q = [], "", None
        for ch in inner:
            if q:
                cur += ch
                if ch == q:
                    q = None
            elif ch in "\"'":
                q = ch; cur += ch
            elif ch == ",":
                out.append(_scalar(cur)); cur = ""
            else:
                cur += ch
        out.append(_scalar(cur))
        return [x for x in out if x is not None]
    if v[0] in "\"'" and v[-1] == v[0] and len(v) > 1:
        return v[1:-1].replace('\\"', '"') if v[0] == '"' else v[1:-1].replace("''", "'")
    if re.fullmatch(r"-?\d+", v):
        return int(v)
    if re.fullmatch(r"-?\d+\.\d*", v):
        return float(v)
    return v


def _strip_comment(line):
    # a comment is " #" outside quotes
    q = None
    for i, ch in enumerate(line):
        if q:
            if ch == q:
                q = None
        elif ch in "\"'":
            q = ch
        elif ch == "#" and i > 0 and line[i - 1] in " \t":
            return line[:i].rstrip()
    return line.rstrip()


def parse_piece(path):
    text = path.read_text(encoding="utf-8")
    m = re.match(r"---\n(.*?)\n---\n", text, re.S)
    if not m:
        raise ValueError(f"{path}: no front matter")
    meta, key = {}, None
    for raw in m.group(1).splitlines():
        line = _strip_comment(raw)
        if not line.strip():
            continue
        if re.match(r"\s+-\s", line) and key:
            if not isinstance(meta.get(key), list):
                meta[key] = []
            meta[key].append(_scalar(line.strip()[2:]))
            continue
        km = re.match(r"([A-Za-z_]+):\s*(.*)$", line)
        if km:
            key = km.group(1)
            meta[key] = _scalar(km.group(2))
    meta["path"] = str(path.relative_to(REPO))
    meta.setdefault("category", path.parent.name)
    for k in ("medium", "palette", "mood", "techniques", "anikit_features", "awards", "stills", "alt_urls"):
        v = meta.get(k)
        meta[k] = [] if v is None else (v if isinstance(v, list) else [v])
    return meta


def load(require_cache=False):
    pj = ROOT / "pieces.json"
    if require_cache and pj.exists():
        return json.loads(pj.read_text())
    out = []
    for cat, _ in CATEGORIES:
        d = ROOT / cat
        if d.is_dir():
            for p in sorted(d.glob("*.md")):
                out.append(parse_piece(p))
    measured = ROOT / "measured.json"
    if measured.exists():
        mm = json.loads(measured.read_text())
        for p in out:
            if p["slug"] in mm:
                p["measured"] = mm[p["slug"]]
    return out

# ---------------------------------------------------------------- index

def cmd_index(a):
    pieces = load()
    from collections import Counter
    for k in ("slug", "url"):
        dup = [x for x, n in Counter(str(p.get(k)).rstrip("/") for p in pieces).items() if n > 1]
        if dup:
            print(f"warning: duplicate {k}: {dup}", file=sys.stderr)
    (ROOT / "pieces.json").write_text(json.dumps(pieces, indent=1, ensure_ascii=False) + "\n")
    lines = []
    for cat, label in CATEGORIES:
        ps = [p for p in pieces if p["category"] == cat]
        if not ps:
            continue
        lines.append(f"\n### {label} (`{cat}/`, {len(ps)})\n")
        lines.append("| Piece | Studio | Year | Length | Pace | Medium | Link |")
        lines.append("|---|---|---|---|---|---|---|")
        for p in sorted(ps, key=lambda p: -(p.get("year") or 0)):
            ln = p.get("length_s")
            ln = f"{int(ln)//60}:{int(ln)%60:02d}" if isinstance(ln, (int, float)) else "?"
            rel = os.path.relpath(REPO / p["path"], ROOT)
            lines.append(f"| [{p['title']}]({rel}) | {p.get('studio','')} | {p.get('year','')} | {ln} | {p.get('pace','')} | {', '.join(p['medium'][:3])} | [watch]({p.get('url','')}) |")
    idx = ROOT / "INDEX.md"
    body = idx.read_text() if idx.exists() else "<!-- TABLES -->\n"
    start, end = "<!-- TABLES:START -->", "<!-- TABLES:END -->"
    table = start + "\n" + "\n".join(lines) + "\n\n" + end
    if start in body:
        body = re.sub(re.escape(start) + r".*?" + re.escape(end), lambda _: table, body, flags=re.S)
    else:
        body = body.rstrip() + "\n\n" + table + "\n"
    idx.write_text(body)
    print(f"{len(pieces)} pieces in {len({p['category'] for p in pieces})} categories → pieces.json, INDEX.md")

# ---------------------------------------------------------------- verify / fetch

def _get(url, timeout=20):
    url = urllib.parse.quote(url, safe=":/?&=%#,+;@~")
    req = urllib.request.Request(url, headers={"User-Agent": "anikit-benchmark/1.0"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read()


def oembed(url):
    host = urllib.parse.urlparse(url).netloc
    if "vimeo.com" in host:
        ep = "https://vimeo.com/api/oembed.json?url=" + urllib.parse.quote(url, safe="")
    elif "youtube.com" in host or "youtu.be" in host:
        ep = "https://www.youtube.com/oembed?format=json&url=" + urllib.parse.quote(url, safe="")
    else:
        return None
    return json.loads(_get(ep))


def _select(pieces, slugs):
    return [p for p in pieces if not slugs or p["slug"] in slugs]


def cmd_verify(a):
    bad = 0
    for p in _select(load(), a.slugs):
        try:
            o = oembed(p["url"])
            if o is None:
                code = _get_status(p["url"])
                print(f"page {code}  {p['slug']}  {p['url']}")
                bad += code >= 400
            else:
                print(f"ok    {p['slug']}  «{o.get('title')}» by {o.get('author_name')}  {o.get('duration','')}")
        except Exception as e:
            bad += 1
            print(f"FAIL  {p['slug']}  {p['url']}  {e}")
    sys.exit(1 if bad else 0)


def _get_status(url):
    try:
        req = urllib.request.Request(url, method="GET", headers={"User-Agent": "Mozilla/5.0"})
        with urllib.request.urlopen(req, timeout=20) as r:
            return r.status
    except urllib.error.HTTPError as e:
        return e.code
    except Exception:
        return 599


YT_ID = re.compile(r"(?:youtube\.com/watch\?v=|youtu\.be/|youtube\.com/shorts/|youtube\.com/embed/)([\w-]{11})")


def still_sources(p):
    """[(candidates, kind)]: candidate URLs tried in order. kind: frame (a true frame of the film),
    thumb (a designed poster/thumbnail, may not be a frame), studio (a still from the owner's page)."""
    out, seen = [], set()
    u = p.get("url", "")
    m = YT_ID.search(u)
    if m:
        vid = m.group(1)
        # YouTube's own auto-frames at 25/50/75%: 1280x720 when available, else 640x480, else 480x360
        for n in (1, 2, 3):
            out.append(([f"https://i.ytimg.com/vi/{vid}/{q}{n}.jpg" for q in ("maxres", "sd", "hq")], "frame"))
        out.append(([f"https://i.ytimg.com/vi/{vid}/maxresdefault.jpg", f"https://i.ytimg.com/vi/{vid}/sddefault.jpg"], "thumb"))
        seen.update(f"https://i.ytimg.com/vi/{vid}/{q}{n}.jpg" for q in ("maxres", "sd", "hq") for n in (1, 2, 3))
        seen.update(f"https://i.ytimg.com/vi/{vid}/{q}.jpg" for q in ("maxresdefault", "sddefault", "hqdefault", "oardefault"))
    elif "vimeo.com" in u:
        try:
            t = oembed(u).get("thumbnail_url")
            if t:
                out.append(([re.sub(r"_\d+(x\d+)?$", "_1280", t), t], "thumb"))
        except Exception:
            pass
    for s in p["stills"]:
        if s in seen or "vimeocdn" in s and any(k == "thumb" for _, k in out):
            continue
        sm = YT_ID.search(s) or re.search(r"i\.ytimg\.com/vi/([\w-]{11})/", s)
        kind = "frame" if re.search(r"/(hq|sd|maxres)[123]\.jpg", s) else "thumb" if ("ytimg" in s or "vimeocdn" in s) else "studio"
        out.append(([s], kind)); seen.add(s)
    return out


def still_urls(p):
    return [c[0] for c, _ in still_sources(p)]


def cmd_fetch(a):
    n_ok = n_fail = 0
    for p in _select(load(), a.slugs):
        d = CACHE / p["slug"]
        d.mkdir(parents=True, exist_ok=True)
        old = d / "stills.json"
        manifest = json.loads(old.read_text()) if old.exists() and not a.refresh else {}
        for i, (cands, kind) in enumerate(still_sources(p)):
            key = cands[0]
            if key in manifest and (d / manifest[key]["file"]).exists():
                n_ok += 1
                continue
            for s in cands:
                try:
                    data = _get(s)
                    if len(data) < 2000:  # YouTube's grey placeholder is ~1 KB
                        raise ValueError("placeholder")
                except Exception as e:
                    err = e
                    continue
                ext = os.path.splitext(urllib.parse.urlparse(s).path)[1].lower()
                ext = ext if ext in (".jpg", ".jpeg", ".png", ".webp") else ".jpg"
                f = f"{len(manifest):02d}-{kind}{ext}"
                (d / f).write_bytes(data)
                manifest[key] = {"file": f, "url": s, "kind": kind, "bytes": len(data)}
                n_ok += 1
                break
            else:
                n_fail += 1
                print(f"  miss {p['slug']} {key} ({err})", file=sys.stderr)
        old.write_text(json.dumps(manifest, indent=1) + "\n")
        (d / "SOURCE.txt").write_text(f"{p['title']} — {p.get('studio')} ({p.get('year')})\n{p.get('url')}\n"
                                      "Official stills, cached for evaluation only. Do not commit or redistribute.\n")
    print(f"stills: {n_ok} cached, {n_fail} missing → {CACHE}")


# ---------------------------------------------------------------- match

def _walk_layers(node, out):
    if isinstance(node, dict):
        t = node.get("type")
        if not t:  # shorthand layers: the type is implied by its key
            t = next((k for k in ("text", "shader", "image", "video", "shape", "component", "composition") if k in node), None)
            if t == "text" and not isinstance(node.get("text"), str):
                t = None
        if t:
            out.append(("type", str(t)))
        if "kinetic" in node:
            out.append(("split", "kinetic"))
        c = node.get("component")
        if isinstance(c, str):
            out.append(("component", c))
        if "split" in node:
            out.append(("split", str(node["split"])))
        for v in node.values():
            _walk_layers(v, out)
    elif isinstance(node, list):
        for v in node:
            _walk_layers(v, out)


def _iter_dicts(node):
    if isinstance(node, dict):
        yield node
        for v in node.values():
            yield from _iter_dicts(v)
    elif isinstance(node, list):
        for v in node:
            yield from _iter_dicts(v)


def _lum(hexs):
    try:
        h = hexs.lstrip("#")[:6]
        r, g, b = (int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))
        return 0.2126 * r + 0.7152 * g + 0.0722 * b, max(r, g, b) - min(r, g, b)
    except Exception:
        return None


def profile_from_spec(path):
    raw = json.loads(Path(path).read_text())
    pr = {k: set() for k in ("medium", "palette", "mood", "techniques", "purpose", "audio")}
    size = raw.get("size", "16:9")
    if isinstance(size, list) and len(size) == 2:
        w, h = size
        size = "9:16" if h > w * 1.3 else "1:1" if abs(w - h) < 5 else "4:5" if h > w else "16:9"
    pr["aspect"] = {"vertical": "9:16", "square": "1:1", "portrait": "4:5"}.get(size, size if size in ("16:9", "9:16", "1:1", "4:5") else "16:9")
    intent = raw.get("intent") or {}
    if isinstance(intent, dict):
        if intent.get("pace") in PACE:
            pr["pace"] = intent["pace"]
        for k in ("genre", "audience", "mood", "purpose"):
            v = intent.get(k)
            if isinstance(v, str):
                pr.setdefault("words", set()).update(re.findall(r"[a-z0-9-]+", v.lower()))
    look = raw.get("look")
    if isinstance(look, str):
        pr.setdefault("words", set()).add(look)
    items = []
    _walk_layers(raw.get("scenes", []), items)
    for sc in raw.get("scenes", []):
        if isinstance(sc, str):
            items.append(("type", "text"))
        elif isinstance(sc, dict):
            for l in sc.get("layers", []):
                if isinstance(l, str):
                    items.append(("type", "text"))
    comps = [v.lower() for k, v in items if k == "component"]
    types = [v for k, v in items if k == "type"]
    n_text = types.count("text")
    n_layers = max(1, sum(1 for t in types if t in ("text", "shape", "image", "video", "group", "box", "component", "shader", "composition")))
    if any(c in ("scene3d", "model", "text3d", "productshot", "product_shot", "globe") or "3d" in c for c in comps):
        pr["medium"].add("3d-cg")
    shaders = [str(n.get("shader")).lower() for n in _iter_dicts(raw.get("scenes", [])) if isinstance(n.get("shader"), str)]
    if any(x in sh for sh in shaders for x in ("chrome", "glass", "metal", "sdf", "3d", "raymarch")):
        pr["medium"].add("3d-cg")
    if "shader" in types or any(c in ("metaballs", "particles", "particletext", "starfield", "rays") for c in comps):
        pr["medium"].add("generative"); pr["techniques"].add("shader" if "shader" in types else "particles")
    if any("chart" in c or c in ("counter", "stat", "globe", "map") for c in comps):
        pr["medium"].add("data-viz"); pr["techniques"].add("chart-build")
    if any(c in ("laptop", "phone", "browser", "cursor", "toast", "code", "ui", "screens", "card_stack") or "ui" in c for c in comps):
        pr["medium"].add("ui"); pr["purpose"].add("ui-demo")
        if "cursor" in comps:
            pr["techniques"].add("cursor")
    if any(c in ("illustration", "sketch", "paper", "hatch", "cutout") for c in comps):
        pr["medium"].add("2d-illustrated")
    if "image" in types or "video" in types:
        pr["medium"].add("mixed")
    if any(k == "split" for k, _ in items) or n_text / n_layers > 0.45:
        pr["medium"].add("kinetic-type"); pr["techniques"].add("kinetic-type")
    pr["type_role"] = "only" if n_text / n_layers > 0.8 else "lead" if n_text / n_layers > 0.4 else "supporting" if n_text else "none"
    if not pr["medium"]:
        pr["medium"].add("2d-vector")
    theme = raw.get("theme") or {}
    cols = theme.get("colors") or theme.get("colours") or {}
    bg = raw.get("background")
    if not isinstance(bg, str):
        bg = next((sc.get("background") for sc in raw.get("scenes", []) if isinstance(sc, dict) and isinstance(sc.get("background"), str)), None)
    hexes = [v for v in list(cols.values()) + ([bg] if isinstance(bg, str) else []) if isinstance(v, str) and v.startswith("#")]
    ls = [x for x in map(_lum, hexes) if x]
    if ls:
        mean_l = sum(l for l, _ in ls) / len(ls)
        pr["palette"].add("dark" if (_lum(bg)[0] if isinstance(bg, str) and bg.startswith("#") and _lum(bg) else mean_l) < 0.35 else "light")
        if max(c for _, c in ls) > 0.6:
            pr["palette"].add("saturated")
    if raw.get("audio"):
        pr["audio"].add("music-led")
    return pr


def profile_from_analysis(path, pr=None):
    an = json.loads(Path(path).read_text())
    g = an.get("global", an)
    pr = pr or {k: set() for k in ("medium", "palette", "mood", "techniques", "purpose", "audio")}
    if g.get("pace") in PACE:
        pr["pace"] = g["pace"]
    if g.get("aspect") in ("16:9", "9:16", "1:1", "4:5"):
        pr["aspect"] = g["aspect"]
    l = g.get("mean_luma")
    if isinstance(l, (int, float)):
        pr["palette"].add("dark" if l < 0.35 else "light" if l > 0.65 else "high-contrast")
    sw = g.get("palette") or []
    if sw:
        chroma = sum(s.get("c", 0) * s.get("share", 0) for s in sw)
        pr["palette"].add("saturated" if chroma > 0.12 else "mono" if chroma < 0.03 else "limited")
    tm = dict((t, n) for t, n in g.get("technique_mix", []))
    m = {"kinetic-type": "kinetic-type", "scene3d": "3d-cg", "2d-motion-graphics": "2d-vector", "shader": "generative",
         "particles": "generative", "footage-or-scene3d": "photoreal-cg"}
    for t in sorted(tm, key=lambda t: -tm[t])[:2]:
        if t in m:
            pr["medium"].add(m[t])
        if t in ("particles", "shader", "kinetic-type"):
            pr["techniques"].add(t)
    pr["measured"] = {k: g.get(k) for k in ("median_shot", "cuts_per_minute", "mean_luma", "shot_count")}
    return pr


def profile_from_tags(s, pr):
    for tok in s.split():
        if "=" in tok:
            k, v = tok.split("=", 1)
            vals = v.split(",")
            if k in ("pace", "aspect", "type_role"):
                pr[k] = vals[0]
            else:
                pr.setdefault(k, set()).update(vals)
    return pr


def _jac(a, b):
    a, b = set(a), set(b)
    return len(a & b) / len(a | b) if a and b else 0.0


CAT_HINT = {"ui": "ui-product", "data-viz": "data-story", "kinetic-type": "kinetic-type", "3d-cg": "cg-3d",
            "photoreal-cg": "product-launch", "generative": "generative", "2d-illustrated": "illustration",
            "2d-vector": "explainer-2d", "cel": "illustration"}
PURPOSE_HINT = {"ui-demo": "ui-product", "launch": "product-launch", "explainer": "explainer-2d", "ident": "brand-idents",
                "title-sequence": "kinetic-type", "music-video": "music-video", "lyric-video": "music-video",
                "data-story": "data-story", "broadcast": "broadcast-sports", "promo": "event-promo", "social-ad": "social-vertical"}


def score(pr, p):
    s, why = 0.0, []
    hints = [CAT_HINT[m] for m in pr.get("medium", ()) if m in CAT_HINT] + [PURPOSE_HINT[x] for x in pr.get("purpose", ()) if x in PURPOSE_HINT] * 2
    if pr.get("aspect") in ("9:16", "4:5"):
        hints += ["social-vertical"] * 2
    if hints and p["category"] in hints:
        s += 2.0 * hints.count(p["category"]) / len(hints)
        why.append(f"category {p['category']}")
    j = _jac(pr.get("medium", ()), p["medium"]); s += 3 * j
    if j: why.append("medium " + "/".join(sorted(set(pr["medium"]) & set(p["medium"]))))
    if p.get("purpose") in pr.get("purpose", ()) or (p["category"] == "ui-product" and "ui-demo" in pr.get("purpose", ())):
        s += 2.5; why.append(f"purpose {p['purpose']}")
    if pr.get("pace") and p.get("pace") in PACE:
        d = abs(PACE.index(pr["pace"]) - PACE.index(p["pace"]))
        s += {0: 2, 1: 1}.get(d, 0)
        if d == 0: why.append(f"pace {p['pace']}")
    if pr.get("aspect") and pr["aspect"] == p.get("aspect"):
        s += 1.5 if pr["aspect"] != "16:9" else 0.5
        if pr["aspect"] != "16:9": why.append(f"aspect {p['aspect']}")
    if pr.get("type_role") and p.get("type_role") in TYPE_ROLE:
        s += 1 - abs(TYPE_ROLE.index(pr["type_role"]) - TYPE_ROLE.index(p["type_role"])) / 3
    for k, w in (("palette", 1.0), ("mood", 1.5), ("techniques", 1.5)):
        jj = _jac(pr.get(k, ()), p[k]); s += w * jj
        if jj: why.append(k + " " + "/".join(sorted(set(pr[k]) & set(p[k]))[:3]))
    if p.get("audio") in pr.get("audio", ()):
        s += 0.5
    words = pr.get("words", set())
    if words:
        hay = " ".join([p.get("title", ""), p.get("studio", ""), p.get("purpose", "") or "", p.get("category", "")]
                       + p["medium"] + p["mood"] + p["techniques"] + p["palette"]).lower()
        hits = [w for w in words if len(w) > 2 and w in hay]
        s += 0.6 * min(len(hits), 5)
        if hits: why.append("words " + "/".join(hits[:4]))
    return s, why


def build_profile(spec=None, analysis=None, tags=None, text=None):
    pr = {k: set() for k in ("medium", "palette", "mood", "techniques", "purpose", "audio")}
    if spec:
        pr = profile_from_spec(spec)
    if analysis:
        pr = profile_from_analysis(analysis, pr)
    if tags:
        pr = profile_from_tags(tags, pr)
    if text:
        words = set(re.findall(r"[a-z0-9-]+", text.lower()))
        pr.setdefault("words", set()).update(words)
        vocab = {"3d": ("medium", "3d-cg"), "cg": ("medium", "3d-cg"), "vertical": ("aspect", "9:16"), "launch": ("purpose", "launch"),
                 "explainer": ("purpose", "explainer"), "title": ("purpose", "title-sequence"), "ident": ("purpose", "ident"),
                 "music": ("purpose", "music-video"), "data": ("medium", "data-viz"), "chart": ("medium", "data-viz"),
                 "ui": ("medium", "ui"), "app": ("medium", "ui"), "saas": ("medium", "ui"), "type": ("medium", "kinetic-type"),
                 "typography": ("medium", "kinetic-type"), "illustrated": ("medium", "2d-illustrated"), "generative": ("medium", "generative")}
        for w in words:
            if w in vocab:
                k, v = vocab[w]
                if k == "aspect": pr[k] = v
                else: pr.setdefault(k, set()).add(v)
            if w in PACE: pr["pace"] = w
    return pr


def rank(pr, n=3):
    """Top n pieces for a profile, at most one per studio: [(score, reasons, piece)]."""
    pieces = load(require_cache=True)
    ranked = sorted(((score(pr, p), p) for p in pieces), key=lambda x: -x[0][0])
    picked, studios = [], set()
    for (sc, why), p in ranked:
        st = (p.get("studio") or "").split("(")[0].strip().lower()
        if st and st in studios:
            continue
        picked.append((sc, why, p)); studios.add(st)
        if len(picked) >= n:
            break
    return picked


def match_refs(spec=None, analysis=None, tags=None, text=None, n=3):
    """Importable API for the judge. Returns [{slug, title, studio, year, url, category, score, why, file}]."""
    pr = build_profile(spec, analysis, tags, text)
    return [dict(slug=p["slug"], title=p["title"], studio=p.get("studio"), year=p.get("year"), url=p.get("url"),
                 category=p["category"], score=round(sc, 2), why=why, file=p["path"]) for sc, why, p in rank(pr, n)]


def style_tags(p):
    return sorted(set([p["category"]] + p["medium"] + p["palette"] + p["mood"] + p["techniques"]
                      + [x for x in (p.get("pace"), p.get("purpose"), p.get("audio"), "type-" + str(p.get("type_role"))) if x]))


def cmd_export(a):
    """Write the JUDGE's layout: <out>/index.json + <out>/<slug>/01.jpg… from the cached stills (fetch first).
    True frames come first, then studio stills, then designed thumbnails (flagged "thumb": true)."""
    import shutil
    out = Path(a.out); out.mkdir(parents=True, exist_ok=True)
    order = {"frame": 0, "studio": 1, "thumb": 2}
    idx = []
    for p in _select(load(), a.slugs):
        src = CACHE / p["slug"]
        mf = src / "stills.json"
        entries = sorted(json.loads(mf.read_text()).values(), key=lambda e: (order.get(e["kind"], 3), e["file"])) if mf.exists() else []
        if a.no_thumbs:
            entries = [e for e in entries if e["kind"] != "thumb"]
        d = out / p["slug"]
        if d.exists():
            shutil.rmtree(d)
        d.mkdir()
        frames = []
        for i, e in enumerate(entries[:8], 1):
            ext = os.path.splitext(e["file"])[1]
            name = f"{i:02d}{ext}"
            shutil.copyfile(src / e["file"], d / name)
            w, h = _img_size(src / e["file"])
            frames.append({"file": name, "kind": e["kind"], "thumb": e["kind"] == "thumb", "w": w, "h": h, "src": e["url"]})
        real = sum(1 for f in frames if not f["thumb"])
        idx.append(dict(slug=p["slug"], title=p["title"], studio=p.get("studio"), year=p.get("year"), url=p.get("url"),
                        format=p.get("aspect"), category=p["category"], style_tags=style_tags(p), pace=p.get("pace"),
                        length_s=p.get("length_s"), file=p["path"], frames=len(frames), real_frames=real,
                        frame_info=frames, measured=p.get("measured")))
    (out / "index.json").write_text(json.dumps(idx, indent=1, ensure_ascii=False) + "\n")
    print(f"{len(idx)} refs → {out}  ({sum(1 for r in idx if r['real_frames'] >= 4)} with ≥4 real frames, "
          f"{sum(1 for r in idx if r['real_frames'] >= 3)} with ≥3)")


def _img_size(path):
    """(w, h) of a JPEG/PNG/WebP without dependencies; (None, None) if unknown."""
    import struct
    try:
        b = Path(path).read_bytes()
        if b[:8] == b"\x89PNG\r\n\x1a\n":
            return struct.unpack(">II", b[16:24])
        if b[:4] == b"RIFF" and b[8:12] == b"WEBP":
            if b[12:16] == b"VP8 ":
                w, h = struct.unpack("<HH", b[26:30]); return w & 0x3FFF, h & 0x3FFF
            if b[12:16] == b"VP8L":
                v = int.from_bytes(b[21:25], "little"); return (v & 0x3FFF) + 1, ((v >> 14) & 0x3FFF) + 1
            if b[12:16] == b"VP8X":
                return int.from_bytes(b[24:27], "little") + 1, int.from_bytes(b[27:30], "little") + 1
        i = 2
        while i < len(b) - 9:
            if b[i] != 0xFF:
                i += 1; continue
            m = b[i + 1]; ln = struct.unpack(">H", b[i + 2:i + 4])[0]
            if m in (0xC0, 0xC1, 0xC2):
                h, w = struct.unpack(">HH", b[i + 5:i + 9]); return w, h
            i += 2 + ln
    except Exception:
        pass
    return None, None


def cmd_match(a):
    pr = build_profile(a.spec, a.analysis, a.tags, a.text)
    picked = rank(pr, a.n)
    shown = {k: (sorted(v) if isinstance(v, set) else v) for k, v in pr.items() if v}
    if a.json:
        print(json.dumps({"profile": shown, "references": [
            {"slug": p["slug"], "score": round(sc, 2), "why": why, "file": p["path"], "url": p.get("url"),
             "stills_dir": str(CACHE / p["slug"])} for sc, why, p in picked]}, indent=1))
        return
    print("our profile:", json.dumps(shown))
    for i, (sc, why, p) in enumerate(picked, 1):
        print(f"\n{i}. {p['title']} — {p.get('studio')} ({p.get('year')})  score {sc:.1f}")
        print(f"   {p['path']}\n   {p.get('url')}\n   stills: {CACHE / p['slug']}  (run `bench.py fetch {p['slug']}` if empty)")
        print(f"   why: {'; '.join(why) or 'closest overall'}")

# ---------------------------------------------------------------- stats / measure

def cmd_stats(a):
    from collections import Counter
    ps = load()
    print(f"pieces: {len(ps)}  categories: {len({p['category'] for p in ps})}")
    lens = sorted(p["length_s"] for p in ps if isinstance(p.get("length_s"), (int, float)))
    if lens:
        print(f"length s: median {lens[len(lens)//2]}  p25 {lens[len(lens)//4]}  p75 {lens[3*len(lens)//4]}  <=90s: {sum(l <= 90 for l in lens)}/{len(lens)}")
    yrs = sorted(p["year"] for p in ps if isinstance(p.get("year"), int))
    if yrs:
        print(f"years: {yrs[0]}–{yrs[-1]}  2015+: {sum(y >= 2015 for y in yrs)}/{len(yrs)}")
    print("verified:", dict(Counter(p.get("verified") for p in ps)))
    for k in ("pace", "aspect", "type_role", "audio", "purpose"):
        print(f"{k}:", dict(Counter(p.get(k) for p in ps).most_common()))
    for k in ("medium", "palette", "mood", "techniques"):
        print(f"{k}:", dict(Counter(x for p in ps for x in p[k]).most_common(25)))
    feats = Counter(x for p in ps for x in p["anikit_features"])
    gaps = Counter(x for x in feats.elements() if str(x).upper().startswith("GAP"))
    print("anikit features:", dict(Counter({k: v for k, v in feats.items() if not str(k).upper().startswith("GAP")}).most_common(25)))
    print("gaps:", dict(gaps.most_common(40)))
    ms = [p for p in ps if isinstance(p.get("measured"), dict)]
    for p in ms:
        m = p["measured"]
        print(f"measured {p['slug']}: median shot {m.get('median_shot')} s, {m.get('cuts_per_minute')} cuts/min, luma {m.get('mean_luma')}")


def cmd_measure(a):
    out = CACHE / a.slug
    out.mkdir(parents=True, exist_ok=True)
    exe = os.environ.get("ANIKIT_VIDEO", str(REPO / "target" / "release" / "anikit-video"))
    aj = out / "analysis.json"
    subprocess.run([exe, "analyze", a.clip, "-o", str(aj), "--sheet", str(out / "sheet.jpg")], check=True)
    g = json.loads(aj.read_text())["global"]
    keep = {k: g[k] for k in ("duration", "shot_count", "median_shot", "mean_shot", "longest_shot", "cuts_per_minute",
                              "pace", "mean_luma", "grain", "vignette", "glow", "aspect")}
    keep["palette"] = [s["hex"] for s in g["palette"][:6]]
    keep["transition_mix"] = g["transition_mix"]
    keep["technique_mix"] = g["technique_mix"]
    keep["style_summary"] = g["style_summary"]
    an = json.loads(aj.read_text())
    if an.get("audio"):
        au = an["audio"]
        keep["audio"] = {k: au.get(k) for k in ("bpm", "cuts_on_beat", "cuts_on_downbeat") if k in au and not isinstance(au[k], list)}
    mf = ROOT / "measured.json"
    mm = json.loads(mf.read_text()) if mf.exists() else {}
    mm[a.slug] = keep
    mf.write_text(json.dumps(mm, indent=1, sort_keys=True) + "\n")
    print(json.dumps(keep, indent=1))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("index").set_defaults(f=cmd_index)
    s = sub.add_parser("verify"); s.add_argument("slugs", nargs="*"); s.set_defaults(f=cmd_verify)
    s = sub.add_parser("fetch"); s.add_argument("slugs", nargs="*"); s.add_argument("--refresh", action="store_true"); s.set_defaults(f=cmd_fetch)
    s = sub.add_parser("match")
    s.add_argument("--spec"); s.add_argument("--analysis"); s.add_argument("--tags"); s.add_argument("--text")
    s.add_argument("-n", type=int, default=3); s.add_argument("--json", action="store_true"); s.set_defaults(f=cmd_match)
    sub.add_parser("stats").set_defaults(f=cmd_stats)
    s = sub.add_parser("export"); s.add_argument("--out", required=True); s.add_argument("--no-thumbs", action="store_true"); s.add_argument("slugs", nargs="*"); s.set_defaults(f=cmd_export)
    s = sub.add_parser("measure"); s.add_argument("slug"); s.add_argument("clip"); s.set_defaults(f=cmd_measure)
    a = ap.parse_args()
    a.f(a)


if __name__ == "__main__":
    main()
