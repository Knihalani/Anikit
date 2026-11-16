mod common;
use anikit_video::dl::{DisplayList, Op, Paint, Rgba};
use anikit_video::text::code::*;
use anikit_video::text::*;

const SRC: &str = r#"import { video } from "anikit/video";

// A launch video in twelve lines.
export default video({ fps: 60, size: [1920, 1080] })
  .scene("intro", (s) => {
    s.text("Ship faster", { font: "Inter Display", weight: 800 });
    s.counter({ from: 0, to: 12_500, prefix: "$" });
  })
  .render();
"#;

#[test]
fn highlights_typescript_and_ranges() {
    let h = highlight(&CodeSpec { code: SRC.into(), language: "ts".into(), highlight: vec![[6, 7]], ..Default::default() });
    assert_eq!(h.syntax, "TypeScript");
    assert_eq!(h.lines.len(), 9);
    assert!(h.lines[5].highlighted && h.lines[6].highlighted && !h.lines[4].highlighted);
    // Keywords, strings and comments get different colours.
    let colours: std::collections::HashSet<String> = h.lines.iter().flat_map(|l| l.spans.iter().map(|s| format!("{:?}", s.color))).collect();
    assert!(colours.len() >= 5, "{colours:?}");
    assert!(h.lines[2].spans[0].italic, "comment italic");
    // Unknown language falls back to plain text.
    let p = highlight(&CodeSpec { code: "x".into(), language: "nope".into(), ..Default::default() });
    assert_eq!(p.syntax, "Plain Text");
    for lang in ["rust", "py", "json", "bash", "tsx", "go", "yaml", "toml", "diff"] {
        let s = highlight(&CodeSpec { code: "a = 1".into(), language: lang.into(), ..Default::default() });
        assert_ne!(s.syntax, "Plain Text", "{lang}");
    }
}

#[test]
fn render_code_block() {
    let fonts = FontStore::new();
    for th in ["night", "day"] {
        let h = highlight(&CodeSpec { code: SRC.into(), language: "ts".into(), theme: th.into(), highlight: vec![[6, 7]], ..Default::default() });
        let code = fonts.layout(&h.text_spec(22.0, 1.6));
        let gutter = fonts.layout(&h.gutter_spec(22.0, 1.6));
        let mut dl = DisplayList { width: 1100, height: 400, background: Rgba([0.2, 0.2, 0.25, 1.0]), ops: vec![] };
        dl.ops.push(Op::RoundRect { x: 20.0, y: 20.0, w: 1060.0, h: 360.0, radius: 16.0, paint: Paint::Solid(h.theme.background.0), transform: anikit_video::dl::IDENTITY });
        let ox = 60.0 + gutter.width + 24.0;
        for (i, l) in h.lines.iter().enumerate() {
            if l.highlighted {
                let r = code.lines[i].rect;
                dl.ops.push(Op::RoundRect { x: 20.0, y: 40.0 + r[1], w: 1060.0, h: r[3], radius: 0.0, paint: Paint::Solid(h.theme.line_highlight.0), transform: anikit_video::dl::IDENTITY });
            }
        }
        gutter.ops([1.0, 0.0, 0.0, 1.0, 60.0, 40.0], &mut dl.ops);
        code.ops([1.0, 0.0, 0.0, 1.0, ox, 40.0], &mut dl.ops);
        let pm = common::render(&fonts, &dl);
        let d = common::golden(&pm, &format!("code_{th}"));
        assert!(d < 0.5, "golden diff {d}");
        common::save(pm, &format!("code_{th}"));
    }
}
