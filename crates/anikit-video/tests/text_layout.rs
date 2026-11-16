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
