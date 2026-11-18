# Judging a demo rebuild so it pairs with r1 and r2

Each eval brief is pinned to the **same 3 references** that r1 and r2 were judged against (`pins.json`), so a rebuild's score is directly comparable. Run this from a checkout that has `claude/epic-ramanujan-qe7x4w-v-judge`'s `scripts/judge.py` and `docs/video/judge/` (fetch that branch, or `git checkout origin/claude/epic-ramanujan-qe7x4w-v-judge -- scripts/judge.py docs/video/judge`):

```bash
python3 scripts/judge.py brief <brief-id> path/to/final.mp4 --out deliverables/demo/<cat>/judge   # 3 trials + critic, ~3 min, ~$0.65
```

- The first run in a container fetches REFS's reference stills into `.cache/` (`sync-bench`, about 1 min). The `claude` CLI must be on PATH (`JUDGE_MODEL` defaults to claude-opus-5-5).
- The command prints the new score next to r1 and r2. The full answer (tells, 3 gaps per trial, critic, the panels the jury saw) is in `<out>/<brief-id>/result.json` and `trial*/`, `critic/`.
- **Noise:** about ±0.4 per piece between identical runs. A rebuild is only clearly better if it gains more than 0.5. Use `--trials 6` for a decision run.
- Verdicts: PASS = judged pro in ≥ 2/3 trials, within 1.0 of the refs and motion ≥ 4; PRO = judged pro, score ≥ 5 and motion ≥ 4; otherwise FAIL (`RUBRIC.md`).

| Brief | Rebuild owner | Command | Pinned refs | r1 | r2 |
|---|---|---|---|---|---|
| a-social (9:16 social ad) | ASSETS | `python3 scripts/judge.py brief a-social final.mp4` | kurzgesagt-short, new-yorker-festival, hermes-buck | 3.25 | 3.38 |
| b-launch (product launch) | CEILING-2 | `python3 scripts/judge.py brief b-launch final.mp4` | coinbase-system-update, jpmorgan-payments, morph-studio-xreal-1s-2025 | 3.75 | 3.62 |
| c-event (event promo) | CEILING-1 | `python3 scripts/judge.py brief c-event final.mp4` | kurzgesagt-short, xk-studio-offfmx-2025-main-titles, studio-river-motion-plus-design-tokyo-2024 | 3.75 | 2.5 |
| d-illustration (illustration-led) | COMPONENTS | `python3 scripts/judge.py brief d-illustration final.mp4` | osko-launch, morph-studio-xreal-1s-2025, found-devialet-opera-de-paris-2022 | 3.38 | 4.38 |
| e-data (data story) | GRAPHIC | `python3 scripts/judge.py brief e-data final.mp4` | kurzgesagt-climate, kurzgesagt-short, cosmos-spacetime-odyssey | 3.62 | 3.38 |
| f-3d (3D/CG) | 3D | `python3 scripts/judge.py brief f-3d final.mp4` | morph-studio-xreal-1s-2025, found-devialet-opera-de-paris-2022, osko-launch | 3.5 | 3.62 |
| g-titles (kinetic type / titles) | TYPE | `python3 scripts/judge.py brief g-titles final.mp4` | game-of-thrones, thank-you-for-smoking, xk-studio-offfmx-2025-main-titles | 3.62 | 3.88 |
| h-ident (brand ident) | TASTE | `python3 scripts/judge.py brief h-ident final.mp4` | game-of-thrones, fitc-tokyo-2015, mad-men | 2.75 | 2.88 |
| i-explainer (2D explainer) | SOUND | `python3 scripts/judge.py brief i-explainer final.mp4` | kurzgesagt-climate, kurzgesagt-short, cosmos-spacetime-odyssey | 2.75 | 2.88 |
| j-music (music / lyric video) | MUSIC | `python3 scripts/judge.py brief j-music final.mp4` | weirdcore-aphex-twin-t69-collapse-2018, blanc-festival-2015, raven-kwok-skyline-2015 | 3.12 | 3.75 |
| k-sports (sports / broadcast) | MOGRAPH | `python3 scripts/judge.py brief k-sports final.mp4` | game-of-thrones, drive-studio-fox-world-cup-2026, xk-studio-offfmx-2025-main-titles | 4.5 | 3.12 |
| l-generative (generative / abstract) | SHADER | `python3 scripts/judge.py brief l-generative final.mp4` | weirdcore-aphex-twin-t69-collapse-2018, game-of-thrones, netflix-ribbon-logo-2019 | 3.12 | 3.62 |

Send the final MP4's branch and path to JUDGE as well: it's re-judged with 6 trials and added to `DEMO-CANDIDATES.md`, where the 05:45 demo set is picked.
