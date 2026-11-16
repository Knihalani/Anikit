//! A 6-second text showcase rendered with the test renderer and piped to ffmpeg:
//! `cargo run --release -p anikit-video --example text_reel -- out.mp4`.
#[path = "../tests/common/mod.rs"]
mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use anikit_video::dl::{DisplayList, Op, Paint, Rgba, IDENTITY};
use anikit_video::layout::{find, layout_boxes, BoxNode};
use anikit_video::text::code::{highlight, CodeSpec};
use anikit_video::text::kinetic::*;
use anikit_video::text::*;

fn tr(x: f32, y: f32) -> [f32; 6] {
    [1.0, 0.0, 0.0, 1.0, x, y]
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "text_reel.mp4".into());
    let (w, h, fps, secs) = (1920u32, 1080u32, 30u32, 6.0f32);
    let fonts = FontStore::new();
    let tree: BoxNode = serde_json::from_str(
        r##"{ "style": { "display": "flex", "flexDirection": "column", "padding": "120px 140px", "gap": 36 },
              "children": [
                { "id": "kicker", "text": { "text": "ANIKIT VIDEO", "style": { "font": "700 26px Inter", "letterSpacing": 0.3, "color": "#8b7bff" } } },
                { "id": "title", "text": { "text": "Programmatic video, ==rendered natively==.", "style": { "font": "800 104px/1.05 Inter Display", "letterSpacing": -0.03, "accent": "#ff6ad5" } } },
                { "id": "row", "style": { "display": "grid", "gridTemplateColumns": "1.4fr 1fr", "gap": 48, "flexGrow": 1 },
                  "children": [ { "id": "code", "style": { "padding": 36 } }, { "id": "stats", "style": { "display": "flex", "flexDirection": "column", "justifyContent": "center", "gap": 8 },
                    "children": [ { "id": "num", "style": { "height": 150 } }, { "id": "cap", "text": { "text": "frames per second, end to end", "style": { "font": "500 32px Inter", "color": "#a6accd" } } } ] } ] }
              ] }"##,
    )
    .unwrap();
    let boxes = layout_boxes(&tree, &fonts, [w as f32, h as f32]).unwrap();
    let at = |id: &str| find(&boxes, id).unwrap().content;
    let text_of = |n: &BoxNode, id: &str| -> TextSpec {
        fn walk<'a>(n: &'a BoxNode, id: &str) -> Option<&'a BoxNode> {
            if n.id.as_deref() == Some(id) {
                return Some(n);
            }
            n.children.iter().find_map(|c| walk(c, id))
        }
        let mut s = walk(n, id).unwrap().text.clone().unwrap();
        s.width = Some(at(id)[2] + 1.0);
        s
    };
    let kicker = TextBlock::new(&fonts, &text_of(&tree, "kicker"), Some(&"tracking 1s".parse().unwrap()), None, [0.0, 0.0]);
    let title = TextBlock::new(&fonts, &text_of(&tree, "title"), Some(&"rise by word 0.8s 70ms".parse().unwrap()), None, [0.0, 0.0]);
    let cap = TextBlock::new(&fonts, &text_of(&tree, "cap"), Some(&"fade by word".parse().unwrap()), None, [0.0, 0.0]);
    let code_box = find(&boxes, "code").unwrap().rect;
    let hl = highlight(&CodeSpec {
        code: "const film = video({ size: \"1080p\", fps: 60 })\n  .scene(\"intro\", s => s.text(\"Ship **faster**\", { animate: \"rise\" }))\n  .render(\"launch.mp4\");".into(),
        language: "ts".into(),
        highlight: vec![[2, 2]],
        ..Default::default()
    });
    let code = KineticPlan::new(&fonts, fonts.layout(&TextSpec { width: None, ..hl.text_spec(24.0, 1.7) }), &"typewriter 25ms".parse().unwrap());
    let counter = CounterPlan::new(&fonts, &Counter { from: 0.0, to: 1240.0, duration: 2.0, style: TextStyle { size: 120.0, weight: 800.0, font: "Inter Display".into(), gradient: Some(Gradient { colors: vec![Color::hex("#7c5cff").unwrap(), Color::hex("#ff6ad5").unwrap()], angle: 90.0 }), ..Default::default() }, ..Default::default() });

    let mut ff = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-r", &fps.to_string(), "-i", "-", "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p", &out])
        .stdin(Stdio::piped())
        .spawn()
        .expect("ffmpeg");
    let stdin = ff.stdin.as_mut().unwrap();
    let t0 = std::time::Instant::now();
    let mut eval_us = 0.0;
    let n = (secs * fps as f32) as u32;
    for f in 0..n {
        let t = f as f32 / fps as f32;
        let e0 = std::time::Instant::now();
        let mut dl = DisplayList { width: w, height: h, background: Rgba([0.035, 0.035, 0.06, 1.0]), ops: Vec::with_capacity(256) };
        let k = at("kicker");
        kicker.ops(t, tr(k[0], k[1]), &mut dl.ops);
        let ti = at("title");
        title.ops(t - 0.3, tr(ti[0], ti[1]), &mut dl.ops);
        // Code card.
        let a = ((t - 1.0) / 0.4).clamp(0.0, 1.0);
        if a > 0.0 {
            let mut bg = hl.theme.background.0;
            bg.0[3] = a;
            dl.ops.push(Op::RoundRect { x: code_box[0], y: code_box[1] + (1.0 - a) * 30.0, w: code_box[2], h: code_box[3], radius: 24.0, paint: Paint::Solid(bg), transform: IDENTITY });
            let c = at("code");
            code.ops(t - 1.3, tr(c[0], c[1] + (1.0 - a) * 30.0), &mut dl.ops);
        }
        let nb = at("num");
        if t > 1.5 {
            counter.ops(t - 1.5, tr(nb[0], nb[1]), &mut dl.ops);
        }
        let cb = at("cap");
        cap.ops(t - 1.8, tr(cb[0], cb[1]), &mut dl.ops);
        eval_us += e0.elapsed().as_secs_f64() * 1e6;
        let pm = common::render(&fonts, &dl);
        stdin.write_all(pm.data_as_u8_slice()).unwrap();
        if f == n / 2 {
            common::save(pm, "reel_mid");
        } else if f == 18 {
            common::save(pm, "reel_early");
        } else {
            stdin.flush().ok();
        }
    }
    drop(ff.stdin.take());
    ff.wait().unwrap();
    println!("{n} frames in {:.2}s; text eval {:.1} µs/frame avg -> {out}", t0.elapsed().as_secs_f64(), eval_us / n as f64);
}
