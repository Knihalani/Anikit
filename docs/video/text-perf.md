# Text cost per frame

Measured with `cargo run --release -p anikit-video --example text_cost` on the 4-core dev machine, single thread. Layouts, kinetic plans, caption tracks and code highlighting are built once at preprocessing (as `eval` does). The numbers are only the per-frame op generation, averaged over 300 frames across each animation.

| case | before µs/frame | before allocs | after µs/frame | after allocs | ops after |
|---|---:|---:|---:|---:|---:|
| title, static | 0.08 | 3 | 0.05 | 0 | 2 |
| paragraph 200 words, static | 0.13 | 1 | 0.10 | 0 | 1 |
| title, pop by char | 0.78 | 15.3 | 0.49 | 0 | 3 |
| paragraph, rise by word | 2.83 | 34 | 2.19 | 0 | 2 |
| title, wave (every char moves) | 1.70 | 36 | 1.14–1.70 | 0 | 35 |
| captions, TikTok pop + shadow | 0.21 | 4.7 | 0.22 | 1 ¹ | 5 |
| code block 16 lines, static | 7.11 | 220 | 0.22 | 0 | 9 (was 220) |
| code block, typewriter | 9.36 | 143 | 2.35 | 0 | 10 |
| counter | 0.24 | 3 | 0.26 | 0 | 1 |

¹ The shadow layer's `Vec<Filter>` in `Op::PushLayer`.

## What changed

- **Runs merged per (face, size, colour)** across the block. Syntax-highlighted code alternates colours, so it went from 220 `Op::Glyphs` to 9. That is 32× less op generation and far fewer ops for the renderer.
- **Glyph buffer pool** (`text::pool`, `text::recycle(&mut ops)`). Text ops take their `Vec<Glyph>` from a per-thread pool. When `eval` hands the previous frame's ops back with `text::recycle`, steady state allocates nothing. Without recycling, the cost is one allocation per glyph op, as before.
- **Kinetic rest batching** uses pooled per-run scratch, and counters format into a reused buffer.
- **Layout cache:** shaped layouts are cached by content (text, style, width) behind an `RwLock` with allocation-free keys (1.0–1.4 µs per hit for 200 words), bounded at 8192 entries. Kinetic styles only change transforms, paint alpha or clips; they never re-shape.
- **Outline cache:** glyph paths are cached per (FontId, glyph, size). A FontId is one face *instance* (font + variation coords + synthesis), so coords are part of the key.

## Preprocessing costs (once per text block)

From `examples/text_bench.rs`: font store 0.12 ms, 200-word layout 0.28 ms, auto-fit 0.8 ms, ellipsis 0.4 ms, code highlight 11 ms cold (first use of a grammar) and then about 1 ms per 40 lines.
