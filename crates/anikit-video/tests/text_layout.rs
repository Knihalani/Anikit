use anikit_video::text::*;

fn store() -> &'static FontStore {
    static S: std::sync::OnceLock<FontStore> = std::sync::OnceLock::new();
    S.get_or_init(FontStore::new)
}

#[test]
fn families_and_basic_layout() {
    let s = store();
    let names = s.family_names();
    eprintln!("{names:?}");
    assert!(s.resolve_family("inter display").is_some());
    let l = s.layout(&TextSpec::plain("Hello world", TextStyle::default()));
    eprintln!("w {} h {} runs {} chars {} words {} lines {}", l.width, l.height, l.runs.len(), l.chars.len(), l.words.len(), l.lines.len());
    eprintln!("{:?}", l.words);
    assert_eq!(l.chars.len(), 10);
    assert_eq!(l.words.len(), 2);
    assert_eq!(l.lines.len(), 1);
    assert!(l.glyphs.iter().all(|g| l.runs[g.run as usize].glyphs[g.index as usize].id != 0));
}

mod common;
use anikit_video::dl::{DisplayList, Rgba, IDENTITY};

fn hex(s: &str) -> Color {
    Color::hex(s).unwrap()
}

#[test]
fn render_rich_paragraph() {
    let s = store();
    let mut spec = TextSpec {
        spans: vec![
            Span { text: "Ship ".into(), weight: Some(800.0), font: Some("Inter Display".into()), ..Default::default() },
            Span { text: "faster".into(), color: Some(hex("#7c5cff")), italic: Some(true), font: Some("Instrument Serif".into()), size: Some(64.0), ..Default::default() },
            Span::new(" with programmatic video 🚀✨. Agents write a spec; "),
            Span { text: "native Rust".into(), underline: Some(true), font: Some("JetBrains Mono".into()), ..Default::default() },
            Span::new(" renders it on every core, with no browser in the loop at all."),
        ],
        width: Some(900.0),
        align: Align::Left,
        ..Default::default()
    };
    spec.style.size = 48.0;
    spec.style.letter_spacing = -0.01;
    let l = s.layout(&spec);
    eprintln!("lines {} h {} runs {}", l.lines.len(), l.height, l.runs.len());
    for ln in &l.lines {
        assert!(ln.rect[2] <= 900.5, "line too wide {:?}", ln.rect);
    }
    let mut dl = DisplayList { width: 1000, height: 500, background: Rgba([0.05, 0.05, 0.08, 1.0]), ops: vec![] };
    let t = [1.0, 0.0, 0.0, 1.0, 50.0, 50.0];
    for w in &l.words {
        dl.ops.push(anikit_video::dl::Op::RoundRect { x: w.rect[0], y: w.rect[1], w: w.rect[2], h: w.rect[3], radius: 0.0, paint: anikit_video::dl::Paint::Solid(Rgba([1.0, 1.0, 1.0, 0.06])), transform: t });
    }
    l.ops(t, &mut dl.ops);
    let _ = IDENTITY;
    common::save(common::render(s, &dl), "rich_paragraph");
}

const PARA: &str = "Agents are best at writing structured data, so the video is a spec: scenes, text, shapes and timing. A native renderer draws every frame in parallel across all cores, and the encoder knows exactly which blocks changed.";

#[test]
fn wrapping_and_alignment() {
    let s = store();
    let mk = |align| TextSpec { text: PARA.into(), width: Some(600.0), align, style: TextStyle { size: 32.0, ..Default::default() }, ..Default::default() };
    let left = s.layout(&mk(Align::Left));
    assert!(left.lines.len() >= 4);
    for l in &left.lines {
        assert!(l.rect[2] <= 600.5);
        assert!(l.rect[0].abs() < 0.01, "left aligned starts at 0");
    }
    // Lines stack at the line height (32 * 1.2).
    for w in left.lines.windows(2) {
        assert!((w[1].baseline - w[0].baseline - 38.4).abs() <= 1.0, "{} {}", w[0].baseline, w[1].baseline);
    }
    let right = s.layout(&mk(Align::Right));
    for l in &right.lines {
        assert!((l.rect[0] + l.rect[2] - 600.0).abs() < 0.6, "{:?}", l.rect);
    }
    let center = s.layout(&mk(Align::Center));
    for l in &center.lines {
        assert!((l.rect[0] + l.rect[2] * 0.5 - 300.0).abs() < 0.6);
    }
    let just = s.layout(&mk(Align::Justify));
    for l in &just.lines[..just.lines.len() - 1] {
        let last = just.chars.iter().rfind(|c| c.text.end <= l.text.end).unwrap();
        assert!((last.rect[0] + last.rect[2] - 600.0).abs() < 1.0, "justified line ends at the edge");
    }
    // Every word sits inside its line.
    for w in &left.words {
        let g = left.glyphs[w.glyphs.start as usize];
        let line = &left.lines[g.line as usize];
        assert!(w.rect[0] >= line.rect[0] - 0.01 && w.rect[0] + w.rect[2] <= line.rect[0] + line.rect[2] + 0.01);
    }
    // Word text ranges round-trip.
    let words: Vec<&str> = left.words.iter().map(|w| &left.text[w.text.clone()]).collect();
    assert_eq!(words[0], "Agents");
    assert_eq!(words.len(), PARA.split_whitespace().count());
}

#[test]
fn max_lines_ellipsis() {
    let s = store();
    let spec = TextSpec { text: PARA.into(), width: Some(500.0), max_lines: Some(2), style: TextStyle { size: 32.0, ..Default::default() }, ..Default::default() };
    let l = s.layout(&spec);
    assert_eq!(l.lines.len(), 2);
    assert!(l.truncated);
    assert!(l.text.ends_with('…'), "{}", l.text);
    // Fills the second line reasonably (not cut absurdly early).
    assert!(l.lines[1].rect[2] > 300.0);
    let no = s.layout(&TextSpec { ellipsis: Some(false), ..spec.clone() });
    assert!(!no.text.ends_with('…'));
    let short = s.layout(&TextSpec { text: "Short".into(), ..spec });
    assert!(!short.truncated);
}

#[test]
fn auto_fit_binary_search() {
    let s = store();
    let fit = |w: f32, h: f32| TextSpec {
        text: PARA.into(),
        style: TextStyle { size: 120.0, ..Default::default() },
        fit: Some(Fit { width: w, height: h, min_size: 8.0, max_size: None }),
        ..Default::default()
    };
    let a = s.layout(&fit(800.0, 300.0));
    assert!(a.height <= 300.0 + 0.01 && a.width <= 800.0 + 0.01, "{} {}", a.width, a.height);
    assert!(a.font_size < 120.0 && a.font_size > 12.0);
    // The next size up would not fit: the search is tight.
    let bigger = s.layout(&TextSpec { fit: None, width: Some(800.0), style: TextStyle { size: a.font_size + 1.0, ..Default::default() }, ..fit(800.0, 300.0) });
    assert!(bigger.height > 300.0, "size {} not tight", a.font_size);
    // Short text grows only up to the max size.
    let short = s.layout(&TextSpec { text: "Hi".into(), ..fit(800.0, 300.0) });
    assert_eq!(short.font_size, 120.0);
    // Huge text clamps at the minimum.
    let tiny = s.layout(&TextSpec { text: PARA.repeat(20), ..fit(200.0, 50.0) });
    assert_eq!(tiny.font_size, 8.0);
}

#[test]
fn cache_hits_and_fonts() {
    let s = store();
    let spec = TextSpec::plain("cached", TextStyle::default());
    let a = s.layout(&spec);
    let b = s.layout(&spec);
    assert!(std::sync::Arc::ptr_eq(&a, &b));
    // Different families intern different faces; weights on a variable font are distinct instances.
    let f = |font: &str, weight| s.layout(&TextSpec::plain("Aa", TextStyle { font: font.into(), weight, ..Default::default() })).runs[0].font;
    assert_ne!(f("Inter", 400.0), f("Inter", 800.0));
    assert_ne!(f("Inter", 400.0), f("Inter Display", 400.0));
    assert_ne!(f("Inter", 400.0), f("JetBrains Mono", 400.0));
    assert_ne!(f("serif", 400.0), f("mono", 400.0));
    assert_eq!(f("Inter", 400.0), f("inter", 400.0));
    let face = s.face(f("Inter", 800.0));
    assert!(!face.coords.is_empty(), "variable instance carries coords");
    // Unknown family falls back to the default sans.
    assert_eq!(f("Nope Sans", 400.0), f("Inter", 400.0));
}

#[test]
fn golden_rich_paragraph() {
    let s = store();
    let spec = TextSpec {
        spans: vec![
            Span { text: "Golden ".into(), weight: Some(800.0), ..Default::default() },
            Span { text: "type".into(), font: Some("Instrument Serif".into()), italic: Some(true), color: Some(hex("#ffcb6b")), ..Default::default() },
            Span::new(" stays put. "),
            Span { text: "mono 0123".into(), font: Some("mono".into()), strikethrough: Some(true), ..Default::default() },
        ],
        width: Some(560.0),
        align: Align::Center,
        ..Default::default()
    };
    let l = s.layout(&spec);
    let mut dl = DisplayList { width: 600, height: 160, background: Rgba([0.06, 0.06, 0.1, 1.0]), ops: vec![] };
    l.ops([1.0, 0.0, 0.0, 1.0, 20.0, 20.0], &mut dl.ops);
    let pm = common::render(s, &dl);
    let d = common::golden(&pm, "rich_paragraph");
    assert!(d < 0.5, "golden diff {d}");
}

#[test]
fn font_shorthand_and_string_specs() {
    let f = parse_font("italic 800 72px/1.1 Inter Display").unwrap();
    assert_eq!((f.family.as_str(), f.weight, f.size, f.line_height, f.italic), ("Inter Display", Some(800.0), Some(72.0), Some(1.1), true));
    assert_eq!(parse_font("JetBrains Mono"), None);
    assert_eq!(parse_font("bold 32px mono").unwrap().weight, Some(700.0));
    let spec: TextSpec = serde_json::from_str(r#""Hello""#).unwrap();
    assert_eq!(spec.text, "Hello");
    let spec: TextSpec = serde_json::from_str(r##"{"text": "Hi", "style": {"font": "black 100px serif", "color": "#ff0", "gradient": ["#7c5cff", "#ff6ad5"]}}"##).unwrap();
    let l = store().layout(&spec);
    assert_eq!(l.font_size, 100.0);
    assert!(l.fill.is_some());
    assert!(matches!(l.paint(&l.runs[0], 1.0), anikit_video::dl::Paint::Linear { .. }));
}

#[test]
fn golden_gradient_title() {
    let s = store();
    let spec: TextSpec = serde_json::from_str(r##"{"text": "Gradient titles ✨", "style": {"font": "900 64px Inter Display", "letterSpacing": -0.02, "gradient": {"colors": ["#7c5cff", "#ff6ad5", "#ffcb6b"], "angle": 100}}}"##).unwrap();
    let l = s.layout(&spec);
    let mut dl = DisplayList { width: 640, height: 110, background: Rgba([0.04, 0.04, 0.07, 1.0]), ops: vec![] };
    l.ops([1.0, 0.0, 0.0, 1.0, 20.0, 15.0], &mut dl.ops);
    let pm = common::render(s, &dl);
    let d = common::golden(&pm, "gradient_title");
    assert!(d < 0.5, "golden diff {d}");
    common::save(pm, "gradient_title");
}

#[test]
fn inline_markup() {
    let spec: TextSpec = serde_json::from_str(r#""Ship **faster** with `code` and ==style==, *really*. 2 * 3 = 6 \\*x""#).unwrap();
    let n = spec.normalized();
    let texts: Vec<&str> = n.spans.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(texts, ["Ship ", "faster", " with ", "code", " and ", "style", ", ", "really", ". 2 * 3 = 6 *x"]);
    assert_eq!(n.spans[1].weight, Some(700.0));
    assert_eq!(n.spans[3].font.as_deref(), Some("JetBrains Mono"));
    assert_eq!(n.spans[5].color, Some(TextStyle::default().accent));
    assert_eq!(n.spans[7].italic, Some(true));
    let off = TextSpec { markup: Some(false), ..spec.clone() }.normalized();
    assert!(off.spans.is_empty());
    let l = store().layout(&spec);
    assert_eq!(l.text, "Ship faster with code and style, really. 2 * 3 = 6 *x");
}

#[test]
fn system_font_fallback_cjk() {
    let s = FontStore::with_system_fonts();
    let l = s.layout(&TextSpec::plain("Hello 日本語", TextStyle::default()));
    // Every glyph resolved (no .notdef) when the system has a CJK font.
    let has_cjk = s.family_names().iter().any(|n| n.contains("IPA") || n.contains("Noto Sans CJK") || n.contains("Gothic"));
    eprintln!("cjk font present: {has_cjk}, runs: {}", l.runs.len());
    if has_cjk {
        assert!(l.glyphs.iter().all(|g| l.runs[g.run as usize].glyphs[g.index as usize].id != 0));
        assert!(l.runs.len() >= 2, "fallback face used");
    }
    // Bundled-only store keeps Inter for Latin.
    let b = store().layout(&TextSpec::plain("Hello", TextStyle::default()));
    assert!(s.face(l.runs[0].font).data.data.len() == store().face(b.runs[0].font).data.data.len());
}

#[test]
fn golden_outline_text() {
    let s = store();
    let spec: TextSpec = serde_json::from_str(r##"{"text": "OUTLINE *type*", "style": {"font": "900 72px Inter Display", "color": "#ffffff", "outline": {"width": 0.02}}}"##).unwrap();
    let l = s.layout(&spec);
    let ops = l.to_ops(anikit_video::dl::IDENTITY);
    assert!(ops.iter().all(|o| matches!(o, anikit_video::dl::Op::Stroke { .. })), "hollow: strokes only");
    let mut dl = DisplayList { width: 640, height: 110, background: Rgba([0.04, 0.04, 0.07, 1.0]), ops: vec![] };
    l.ops([1.0, 0.0, 0.0, 1.0, 20.0, 12.0], &mut dl.ops);
    let pm = common::render(s, &dl);
    let d = common::golden(&pm, "outline_text");
    assert!(d < 0.5, "golden diff {d}");
    common::save(pm, "outline_text");
}

#[test]
fn caption_background_and_effects() {
    use anikit_video::dl::Op;
    let s = store();
    let spec: TextSpec = serde_json::from_str(r##"{"text": "Captions get a box per line, like social video.", "width": 560, "align": "center",
        "style": {"font": "700 40px Inter", "background": {"color": "#000c"}, "shadow": {}, "glow": {"radius": 10}}}"##).unwrap();
    let l = s.layout(&spec);
    assert_eq!(l.backgrounds.len(), l.lines.len());
    assert_eq!(l.filters.len(), 2);
    let ops = l.to_ops(anikit_video::dl::IDENTITY);
    assert!(matches!(ops[0], Op::RoundRect { .. }));
    assert!(ops.iter().any(|o| matches!(o, Op::PushLayer { .. })));
    assert!(matches!(ops.last(), Some(Op::PopLayer)));
    // Kinetic output keeps the backgrounds and the layer balanced.
    let plan = anikit_video::text::kinetic::KineticPlan::new(s, l.clone(), &"karaoke".parse().unwrap());
    let mut o = vec![];
    plan.ops(0.5, anikit_video::dl::IDENTITY, &mut o);
    let push = o.iter().filter(|x| matches!(x, Op::PushLayer { .. })).count();
    let pop = o.iter().filter(|x| matches!(x, Op::PopLayer)).count();
    assert_eq!(push, pop);
    assert!(matches!(o[0], Op::RoundRect { .. }));
    let mut dl = DisplayList { width: 600, height: 200, background: Rgba([0.3, 0.4, 0.5, 1.0]), ops: vec![] };
    plan.ops(0.9, [1.0, 0.0, 0.0, 1.0, 20.0, 30.0], &mut dl.ops);
    let pm = common::render(s, &dl);
    let d = common::golden(&pm, "caption");
    assert!(d < 0.5, "golden diff {d}");
    common::save(pm, "caption");
}

#[test]
fn rtl_arabic_hebrew_and_mixed() {
    use anikit_video::text::kinetic::{Kinetic, KineticPlan};
    let s = store();
    for (text, words) in [("مرحبا بالعالم الجميل", 3), ("שלום עולם יפה", 3), ("Hello שלום world", 3)] {
        let l = s.layout(&TextSpec::plain(text, TextStyle::default()));
        assert_eq!(l.words.len(), words, "{text}");
        assert!(l.glyphs.iter().all(|g| l.runs[g.run as usize].glyphs[g.index as usize].id != 0), "no tofu in {text}");
        // Reading order: the first logical word of an RTL text sits rightmost.
        let ord = l.logical_order(UnitKind::Word);
        let first = &l.words[ord[0]];
        if !text.starts_with('H') {
            assert!(l.words.iter().all(|w| w.rect[0] <= first.rect[0] + 0.01), "{text}: first word is rightmost");
            // Kinetic stagger starts at the reading start.
            let plan = KineticPlan::new(s, l.clone(), &"fade by word 0.1s 1s".parse::<Kinetic>().unwrap());
            let mut o = vec![];
            plan.ops(0.05, anikit_video::dl::IDENTITY, &mut o);
            let xs: Vec<f32> = o.iter().filter_map(|op| match op { anikit_video::dl::Op::Glyphs { glyphs, .. } => glyphs.first().map(|g| g.x), _ => None }).collect();
            assert!(!xs.is_empty() && xs.iter().all(|x| *x >= first.rect[0] - 1.0), "{text}: rightmost word animates first");
        }
    }
    // Arabic shaping joins letters: fewer glyphs than a naive one-per-char would mean no shaping only for isolated forms;
    // check the run uses the bundled Arabic face.
    let l = s.layout(&TextSpec::plain("سلام", TextStyle::default()));
    assert_eq!(l.chars.len(), 4);
    assert!(s.face(l.runs[0].font).data.data.len() > 500_000, "Noto Sans Arabic");
}

#[test]
fn emoji_sequences_in_titles() {
    let s = store();
    let l = s.layout(&TextSpec::plain("Ship it 🚀 👩‍💻 👍🏽 🇯🇵", TextStyle { font: "800 64px Inter Display".into(), ..Default::default() }));
    assert!(l.glyphs.iter().all(|g| l.runs[g.run as usize].glyphs[g.index as usize].id != 0), "no tofu");
    // ZWJ sequence, skin tone and flag are each one grapheme unit (one char unit).
    let emoji_units = l.chars.iter().filter(|c| l.text[c.text.clone()].chars().count() > 1).count();
    assert_eq!(emoji_units, 3, "{:?}", l.chars.iter().map(|c| &l.text[c.text.clone()]).collect::<Vec<_>>());
    let mut dl = DisplayList { width: 640, height: 100, background: Rgba([0.04, 0.04, 0.07, 1.0]), ops: vec![] };
    l.ops([1.0, 0.0, 0.0, 1.0, 20.0, 10.0], &mut dl.ops);
    let pm = common::render(s, &dl);
    let d = common::golden(&pm, "emoji_title");
    assert!(d < 0.5, "golden diff {d}");
    common::save(pm, "emoji_title");
}

#[test]
fn golden_rtl() {
    let s = store();
    let spec = TextSpec { text: "مرحبا بالعالم\nשלום עולם — Hello 123".into(), align: Align::Right, width: Some(560.0), markup: Some(false), style: TextStyle { font: "600 48px Inter".into(), ..Default::default() }, ..Default::default() };
    let l = s.layout(&spec);
    let mut dl = DisplayList { width: 600, height: 150, background: Rgba([0.04, 0.04, 0.07, 1.0]), ops: vec![] };
    l.ops([1.0, 0.0, 0.0, 1.0, 20.0, 10.0], &mut dl.ops);
    let pm = common::render(s, &dl);
    let d = common::golden(&pm, "rtl");
    assert!(d < 0.5, "golden diff {d}");
}
