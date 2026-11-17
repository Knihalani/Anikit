mod common;
use anikit_video::dl::{DisplayList, Op, Paint, Rgba};
use anikit_video::text::captions::*;
use anikit_video::text::*;

const SRT: &str = "1\n00:00:00,500 --> 00:00:02,500\nAgents write the spec.\n\n2\n00:00:03,000 --> 00:00:05,000\nRust renders every frame in parallel.\n";
const VTT: &str = "WEBVTT\n\n00:01.000 --> 00:03.000 align:center\n<v Ana>Ship <00:01.400>it <00:01.700><c>today</c>\n";

#[test]
fn parse_srt_vtt_and_words() {
    let c = parse(SRT);
    assert_eq!(c.len(), 2);
    assert_eq!((c[0].start, c[0].end), (0.5, 2.5));
    assert_eq!(c[1].text, "Rust renders every frame in parallel.");
    let v = parse(VTT);
    assert_eq!(v[0].text, "Ship it today");
    let w: Vec<(String, f32, f32)> = v[0].words.iter().map(|w| (w.text.clone(), w.start, w.end)).collect();
    assert_eq!(w, [("Ship".into(), 1.0, 1.4), ("it".into(), 1.4, 1.7), ("today".into(), 1.7, 3.0)]);
    let mut e = c.clone();
    estimate_words(&mut e);
    assert_eq!(e[0].words.len(), 4);
    assert!((e[0].words[0].start - 0.5).abs() < 1e-4 && (e[0].words[3].end - 2.5).abs() < 1e-3);
    assert!(e[0].words[1].end - e[0].words[1].start > e[0].words[2].end - e[0].words[2].start, "longer word, longer time");
    let f = from_words("one two three", &[0.0, 0.4, 0.9], 1.5);
    assert_eq!(f[0].words[1].end, 0.9);
}

fn colours(track: &CaptionTrack, t: f32) -> Vec<[f32; 4]> {
    let mut o = vec![];
    track.ops(t, anikit_video::dl::IDENTITY, &mut o);
    o.iter().filter_map(|op| match op { Op::Glyphs { paint: Paint::Solid(c), .. } => Some(c.0), _ => None }).collect()
}

#[test]
fn pages_and_highlight() {
    let fonts = FontStore::new();
    let track = CaptionTrack::new(&fonts, &parse(SRT), &CaptionStyle { max_words: 3, ..Default::default() });
    // 4 + 6 words at 3 per page (a sentence end also breaks).
    assert_eq!(track.page_count(), 4);
    assert_eq!(track.page_text(0.6), Some("Agents write the"));
    assert_eq!(track.page_text(0.0), None);
    assert_eq!(track.page_text(2.8), Some("spec."), "short pause bridged");
    assert_eq!(track.page_text(10.0), None);
    let c = colours(&track, 1.0);
    let hl = CaptionStyle::default().highlight.unwrap().0 .0;
    assert_eq!(c.iter().filter(|x| **x == hl).count(), 1, "exactly one active word");
    // One word at a time.
    let one = CaptionTrack::new(&fonts, &parse(SRT), &CaptionStyle { max_words: 1, upcoming: Upcoming::Hidden, ..Default::default() });
    assert_eq!(one.page_count(), 10);
}

#[test]
fn reading_time() {
    assert!((reading_seconds("one two three four five six seven") - 2.6).abs() < 1e-4);
    assert!(word_count("日本語のテキスト") >= 3.0);
    let long = "Agents are best at writing structured data so the video is a spec of scenes text shapes and timing";
    let f = fit_to_seconds(long, 3.0);
    assert!(f.ends_with('…') && reading_seconds(f.trim_end_matches('…')) <= 3.0 + 1e-3, "{f}");
    assert_eq!(fit_to_seconds("Short", 3.0), "Short");
}

#[test]
fn golden_tiktok_captions() {
    let fonts = FontStore::new();
    let st = CaptionStyle { highlight: Some(Color::hex("#ffffff").unwrap()), highlight_box: Some(Color::hex("#7c5cff").unwrap()), uppercase: true, ..Default::default() };
    let track = CaptionTrack::new(&fonts, &parse(VTT), &st);
    let mut dl = DisplayList { width: 700, height: 200, background: Rgba([0.2, 0.25, 0.3, 1.0]), ops: vec![] };
    track.ops(1.5, [1.0, 0.0, 0.0, 1.0, 350.0, 160.0], &mut dl.ops);
    let pm = common::render(&fonts, &dl);
    let d = common::golden(&pm, "captions_tiktok");
    assert!(d < 0.5, "golden diff {d}");
    common::save(pm, "captions_tiktok");
}
