mod common;
use anikit_video::dl::{DisplayList, Op, Rgba};
use anikit_video::text::kinetic::*;
use anikit_video::text::*;

fn store() -> &'static FontStore {
    static S: std::sync::OnceLock<FontStore> = std::sync::OnceLock::new();
    S.get_or_init(FontStore::new)
}

fn styles() -> Vec<(&'static str, Kinetic)> {
    let k = |style, unit| Kinetic { style, unit, ..Default::default() };
    vec![
        ("rise", k(KineticStyle::Rise { distance: 40.0 }, UnitKind::Word)),
        ("fade", k(KineticStyle::Fade, UnitKind::Char)),
        ("blur", k(KineticStyle::BlurIn { radius: 12.0 }, UnitKind::Word)),
        ("pop", k(KineticStyle::ScalePop { from: 0.0 }, UnitKind::Char)),
        ("type", Kinetic { stagger: 0.04, duration: 0.0, ..k(KineticStyle::Typewriter { caret: true, caret_color: None, blink: 1.0 }, UnitKind::Char) }),
        ("scramble", Kinetic { stagger: 0.04, ..k(KineticStyle::Scramble { charset: "ABCDEFGHIJKLMNOPQRSTUVWXYZ#%&".into(), rate: 20.0, seed: 1 }, UnitKind::Char) }),
        ("wave", k(KineticStyle::Wave { amplitude: 10.0, wavelength: 8.0, speed: 0.8 }, UnitKind::Char)),
        ("highlight", Kinetic { stagger: 0.25, ..k(KineticStyle::HighlightSweep { color: Color::rgba(0.49, 0.36, 1.0, 0.55), padding: 4.0, radius: 6.0 }, UnitKind::Word) }),
        ("mask", k(KineticStyle::MaskReveal { direction: Direction::Up }, UnitKind::Word)),
    ]
}

fn spec() -> TextSpec {
    let mut s = TextSpec::plain("Make videos in code", TextStyle { size: 44.0, weight: 700.0, font: "Inter Display".into(), ..Default::default() });
    s.style.underline = false;
    s
}

fn glyph_alpha_sum(ops: &[Op]) -> (usize, f32) {
    let mut n = 0;
    let mut a = 0.0;
    for op in ops {
        if let Op::Glyphs { glyphs, paint: anikit_video::dl::Paint::Solid(c), .. } = op {
            n += glyphs.len();
            a += c.0[3] * glyphs.len() as f32;
        }
    }
    (n, a)
}

#[test]
fn kinetic_start_mid_end() {
    let fonts = store();
    let l = fonts.layout(&spec());
    let total = l.chars.len();
    for (name, k) in styles() {
        let plan = KineticPlan::new(fonts, l.clone(), &k);
        let end = plan.duration() + 0.01;
        let mut o0 = vec![];
        plan.ops(0.0, anikit_video::dl::IDENTITY, &mut o0);
        let mut om = vec![];
        plan.ops(end * 0.5, anikit_video::dl::IDENTITY, &mut om);
        let mut oe = vec![];
        plan.ops(end + 5.0, anikit_video::dl::IDENTITY, &mut oe);
        let (n0, _) = glyph_alpha_sum(&o0);
        let (nm, am) = glyph_alpha_sum(&om);
        let (ne, ae) = glyph_alpha_sum(&oe);
        eprintln!("{name}: t0 {n0} glyphs, mid {nm} ({am:.1}), end {ne} ({ae:.1}) ops {}", oe.len());
        // End: exactly the text, fully opaque, few ops.
        assert_eq!(ne, total, "{name} end glyphs");
        assert!((ae - total as f32).abs() < 1e-3, "{name} end alpha");
        // Start: nothing visible, except continuous styles.
        match name {
            "wave" | "highlight" => assert_eq!(n0, total),
            _ => assert!(n0 <= 1, "{name} start shows {n0}"),
        }
        // Mid: partially there.
        if name == "blur" || name == "mask" {
            assert!(om.iter().any(|o| matches!(o, Op::PushLayer { .. })), "{name} mid has no layer");
        } else if !matches!(name, "wave" | "highlight" | "type" | "scramble") {
            assert!(am > 0.0 && am < total as f32, "{name} mid alpha {am}");
        }
    }
}

#[test]
fn exit_reverses() {
    let fonts = store();
    let l = fonts.layout(&spec());
    let plan = KineticPlan::new(fonts, l.clone(), &Kinetic { exit: true, style: KineticStyle::Fade, ..Default::default() });
    let mut o = vec![];
    plan.ops(0.0, anikit_video::dl::IDENTITY, &mut o);
    assert_eq!(glyph_alpha_sum(&o).0, l.chars.len());
    o.clear();
    plan.ops(plan.duration() + 1.0, anikit_video::dl::IDENTITY, &mut o);
    assert_eq!(glyph_alpha_sum(&o).0, 0);
}

#[test]
fn counter_formats() {
    assert_eq!(format_number(1234567.891, 2, ",", "$", ""), "$1,234,567.89");
    assert_eq!(format_number(-42.0, 0, ",", "", "%"), "-42%");
    assert_eq!(format_number(999.0, 0, "", "", "k"), "999k");
    let c = CounterPlan::new(store(), &Counter { from: 0.0, to: 12500.0, prefix: "$".into(), ..Default::default() });
    assert_eq!(c.text(0.0), "$0");
    assert_eq!(c.text(10.0), "$12,500");
    let mut o = vec![];
    c.ops(10.0, anikit_video::dl::IDENTITY, &mut o);
    assert_eq!(glyph_alpha_sum(&o).0, 7);
}

#[test]
fn contact_sheet() {
    let fonts = store();
    let l = fonts.layout(&spec());
    let cols = [0.25f32, 0.5, 1.0];
    let (cw, rh) = (520.0, 80.0);
    let st = styles();
    let mut dl = DisplayList { width: (cw * 3.0) as u32, height: (rh * st.len() as f32 + 100.0) as u32, background: Rgba([0.04, 0.04, 0.07, 1.0]), ops: vec![] };
    for (row, (_, k)) in st.iter().enumerate() {
        let plan = KineticPlan::new(fonts, l.clone(), k);
        let d = plan.duration();
        for (c, f) in cols.iter().enumerate() {
            let t = [1.0, 0.0, 0.0, 1.0, 20.0 + c as f32 * cw, 15.0 + row as f32 * rh];
            plan.ops(d * f + if *f >= 1.0 { 0.01 } else { 0.0 }, t, &mut dl.ops);
        }
    }
    let c = CounterPlan::new(fonts, &Counter { to: 1284503.0, prefix: "$".into(), duration: 2.0, ..Default::default() });
    for (i, f) in cols.iter().enumerate() {
        c.ops(2.0 * f, [1.0, 0.0, 0.0, 1.0, 20.0 + i as f32 * cw, 15.0 + rh * st.len() as f32 - 20.0], &mut dl.ops);
    }
    let pm = common::render(fonts, &dl);
    common::save(pm, "kinetic_sheet");
}
