# Overnight push (21:30 → 07:00 UTC): shared brief for every session

## The user's verdict and goal (verbatim)
"Literally none of those were impressive at all, and still had super boring and AI slop like typography, they were insanely boring. They have to be SO much more impressive. … 1. Make the output videos genuinely incredible, visually stunning, like it should feel RIDICULOUS how good they are 2. Make it loads faster and more efficient both token and compute/memory wise and 3. Just overall make it far and away the best option out there, it should be able to make stuff in any style and no matter the style should be as good as if a full human design team had made it and spent ages on it but should still be fast."
Also: "There's not enough stuff on research on taste, design, sound design, visual design, 2d design, graphic design, motion design, 3d design etc, not enough feature building, not nearly enough optimisation."

## Read first
- `docs/video/quality/BAR.md`: every rejection so far, with the reasons.
- `docs/video/taste/TASTE.md`: the taste canon (advice, not rules).
- The latest rejected outputs: branches `claude/epic-ramanujan-qe7x4w-v-div-{a..g}` and `-v-blind-ak2` (`deliverables/*/`).
- Known root cause: the engine ships only 3 typefaces (Inter, Instrument Serif, JetBrains Mono).

## Rules
- Outside skills, guidance is advice (what works, what to avoid, why). Never force a style; engine defaults stay neutral.
- Deterministic output (byte-identical across thread counts). No quality regression (PERF's `qbench --gate`). Measured numbers only.
- Merge `origin/claude/epic-ramanujan-qe7x4w` into your branch before every push; push small increments often.
- Every feature ships with docs (a short `anikit-video docs <topic>` page or section), a test, and a proof still/clip at full size.
- Research goes in `docs/video/craft/<domain>.md` (sources linked). Distil it into principles, techniques with numbers, what separates studio work from amateur work, and a gap list against Anikit.
- Report to the parent (send_message "@parent") about every 90 minutes: what changed, measured numbers, proof paths.
- Commit trailer:
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01YBqayaET7JSh2nx9HSb69L
- Never put model names in code or commits. No PRs.

## Schedule
- Eval rounds (JUDGE runs fixed briefs with fresh agents on current main) at 23:30, 01:30, 03:30, 05:30. The parent merges green branches at :00 and :30.
- 06:15: final merge. 07:00: report to the user.
