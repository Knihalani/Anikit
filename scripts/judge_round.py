#!/usr/bin/env python3
"""Eval rounds: 12 fixed briefs (docs/video/judge/BRIEFS.md, briefs.json) made by fresh agent sessions on
current main, then judged blind against professional references (scripts/judge.py).

1. Parent:  python3 scripts/judge_round.py prompts --round r1 > r1-sessions.json
            then for each entry: create_session(title, prompt, outcome_branch, source_url=repo,
            source_revision="claude/epic-ramanujan-qe7x4w"). Note the session ids in
            docs/video/judge/rounds/r1/sessions.json as {"<brief id>": "<session id>"} (optional).
2. Judge:   python3 scripts/judge_round.py collect --round r1     # fetch outputs, judge, write table
            (fill rounds/r1/usage.json with tokens/cost per brief from the sessions' result events)
            python3 scripts/judge_round.py table --round r1       # (re)write the ROUNDS.md section

Outputs land on branch claude/epic-ramanujan-qe7x4w-v-eval-<round>-<brief id>, in deliverables/eval/<brief id>/.
"""
import argparse, json, statistics, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
J = ROOT / "docs/video/judge"
BASE = "claude/epic-ramanujan-qe7x4w"

PROMPT = """You are making one video with Anikit Video, a JSON-spec -> MP4 engine. No human will answer questions: decide everything yourself.

Setup (prebuilt release of current main, installs in seconds):
  ANIKIT_INSTALL_DIR=$PWD/.bin sh install.sh && export PATH=$PWD/.bin:$PATH && anikit-video --version
If that fails, run `cargo build --release -p anikit-video` once, in the foreground, and use target/release.
Then read skills/anikit-video/SKILL.md and follow it (use `anikit-video docs <topic>` for detail).

BRIEF
{brief}

Make it as good as a top motion-design studio would, in whatever style the brief deserves.

Deliver in deliverables/eval/{id}/ :
- final.mp4 (exact name; the finished film with sound), spec.json, and any source you wrote;
- stats.json: {{"start_utc": "...", "end_utc": "...", "renders": [...]}} where start_utc is the time you
  started (run `date -u +%FT%TZ` first thing) and end_utc when final.mp4 is done; render the FINAL film with
  `python3 scripts/measure.py -o deliverables/eval/{id}/renders.jsonl -- anikit-video render ...`
  and copy those JSON lines into "renders".
Commit only deliverables/eval/{id}/ and push to branch {branch}. Your turn isn't over until final.mp4 exists and is
pushed: never end it with a render still running in the background. Then reply with one line: the path of final.mp4."""


def briefs():
    return json.loads((J / "briefs.json").read_text())


def cmd_prompts(a):
    out = []
    for b in briefs():
        if a.only and b["id"] not in a.only.split(","):
            continue
        branch = f"{BASE}-v-eval-{a.round}-{b['id']}"
        out.append({"id": b["id"], "title": f"Eval {a.round}: {b['id']}", "outcome_branch": branch,
                    "source_revision": BASE, "prompt": PROMPT.format(brief=b["brief"], id=b["id"], branch=branch)})
    print(json.dumps(out, indent=1))


VARIANTS = (("", "eval"), ("hf-", "eval-hf"))  # Anikit, and HyperFrames comparisons on the same briefs


def fetch(round_):
    """{name: {...}} for every output on origin; name is the brief id, or hf-<id> for HyperFrames."""
    found = {}
    for b in briefs():
        for pre, d in VARIANTS:
            branch = f"{BASE}-v-eval-{round_}-{pre}{b['id']}"
            r = subprocess.run(["git", "-C", str(ROOT), "fetch", "-q", "origin", f"{branch}:refs/remotes/origin/{branch}"], capture_output=True)
            if r.returncode:
                continue
            ls = subprocess.run(["git", "-C", str(ROOT), "ls-tree", "-r", "--name-only", f"origin/{branch}", f"deliverables/{d}/{b['id']}/"],
                                capture_output=True, text=True).stdout.split()
            mp4 = [p for p in ls if p.endswith("final.mp4")] or [p for p in ls if p.endswith(".mp4")]
            if mp4:
                found[pre + b["id"]] = {"branch": branch, "mp4": mp4[0], "dir": f"deliverables/{d}/{b['id']}", "brief": b}
    return found


def cmd_collect(a):
    rd = J / "rounds" / a.round
    rd.mkdir(parents=True, exist_ok=True)
    if not a.no_sync:
        subprocess.run([sys.executable, str(ROOT / "scripts/judge.py"), "sync-bench"])  # REFS's benchmark stills (not in git)
    found = fetch(a.round)
    print(f"outputs found: {sorted(found)}", file=sys.stderr)
    manifest = []
    for name, f in sorted(found.items()):
        b = f["brief"]
        manifest.append({"name": name, "src": f"git:{f['branch']}:{f['mp4']}", "tags": b["tags"], "format": b["format"]})
        stats = subprocess.run(["git", "-C", str(ROOT), "show", f"origin/{f['branch']}:{f['dir']}/stats.json"],
                               capture_output=True, text=True).stdout
        if stats:
            (rd / f"{name}.stats.json").write_text(stats)
    (rd / "manifest.json").write_text(json.dumps(manifest, indent=1))
    cmd = [sys.executable, str(ROOT / "scripts/judge.py"), "batch", str(rd / "manifest.json"), "--out", str(rd / "results"),
           "--trials", str(a.trials)]
    if a.skip_done:
        cmd.append("--skip-done")
    subprocess.run(cmd)
    cmd_table(a)


def _stats(rd, bid):
    p = rd / f"{bid}.stats.json"
    if not p.exists():
        return None, None, None
    try:
        s = json.loads(p.read_text())
    except json.JSONDecodeError:
        return None, None, None
    from datetime import datetime
    wall = None
    try:
        t0 = datetime.fromisoformat(s["start_utc"].replace("Z", "+00:00"))
        t1 = datetime.fromisoformat(s["end_utc"].replace("Z", "+00:00"))
        wall = round((t1 - t0).total_seconds() / 60, 1)
    except Exception:
        pass
    rr = [r for r in s.get("renders", []) if isinstance(r, dict)]
    rss = max((r.get("peak_rss_mb", 0) for r in rr), default=None)
    render_s = rr[-1].get("wall_s") if rr else None
    return wall, rss, render_s


def fmt_tok(u):
    return f"{u['tokens'] / 1e6:.1f}" if u.get("tokens") else "?"


def cmd_table(a):
    rd = J / "rounds" / a.round
    rd.mkdir(parents=True, exist_ok=True)
    usage = json.loads((rd / "usage.json").read_text()) if (rd / "usage.json").exists() else {}
    lines = [f"## Round {a.round}", "",
             "| Brief | Category | Verdict | Judged pro | Score (blind) | Refs mean | Δ | Full (+motion/sound) | Agent wall (min) | Tokens (M) | Cost ($) | Final render (s) | Peak RSS render (MB) |",
             "|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    gaps_by = {}
    scores = []
    rows = []
    for b in briefs():
        rows.append((b["id"], b))
        if (rd / "results" / f"hf-{b['id']}").exists() or f"hf-{b['id']}" in usage:
            rows.append((f"hf-{b['id']}", b))
    for name, b in rows:
        p = rd / "results" / name / "result.json"
        wall, rss, render_s = _stats(rd, name)
        u = usage.get(name, {})
        if not p.exists():
            lines.append(f"| {name} | {b.get('category','')} | no output | | | | | | {wall or ''} | {fmt_tok(u)} | {u.get('cost_usd', '')} | {render_s or ''} | {rss or ''} |")
            continue
        r = json.loads(p.read_text())
        c = r["candidate"]
        if not name.startswith("hf-"):
            scores.append(c.get("overall") or 0)
        lines.append(f"| {name} | {b.get('category','')} | {r['verdict']} | {c.get('judged_professional')} | {c.get('overall')} | {r['ref_mean']} | {r['delta']} | "
                     f"{r.get('full_overall', '–')} | {wall or '?'} | {fmt_tok(u)} | {u.get('cost_usd', '?')} | {render_s or '?'} | {rss or '?'} |")
        gaps_by[name] = c.get("gaps", [])[:3] + (r.get("critic", {}).get("answer", {}).get("gaps", [])[:1])
    if scores:
        lines += ["", f"Mean blind score (Anikit rows) {statistics.mean(scores):.2f}. hf-* rows are HyperFrames on the same brief, same refs and seed. Pass = judged professional in ≥ half the trials and Δ ≥ −1.0 (RUBRIC.md)."]
    lines += ["", "Top gaps per brief:", ""]
    for k, g in gaps_by.items():
        lines.append(f"- **{k}**: " + " · ".join(g))
    section = "\n".join(lines) + "\n"
    (rd / "TABLE.md").write_text(section)
    rounds = J / "ROUNDS.md"
    text = rounds.read_text() if rounds.exists() else "# Eval rounds\n\nFixed briefs: `briefs.json`. Procedure: `RUBRIC.md`, `scripts/judge_round.py`.\n\n"
    marker = f"## Round {a.round}\n"
    if marker in text:
        head, rest = text.split(marker, 1)
        nxt = rest.find("\n## Round ")
        text = head + section + (rest[nxt + 1:] if nxt >= 0 else "")
    else:
        text = text.rstrip() + "\n\n" + section
    rounds.write_text(text)
    print(section)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for n in ("prompts", "collect", "table"):
        p = sub.add_parser(n)
        p.add_argument("--round", required=True)
        p.add_argument("--only")
        p.add_argument("--trials", type=int, default=3)
        p.add_argument("--skip-done", action="store_true")
        p.add_argument("--no-sync", action="store_true", help="don't re-fetch REFS's benchmark stills")
    a = ap.parse_args()
    {"prompts": cmd_prompts, "collect": cmd_collect, "table": cmd_table}[a.cmd](a)


if __name__ == "__main__":
    main()
