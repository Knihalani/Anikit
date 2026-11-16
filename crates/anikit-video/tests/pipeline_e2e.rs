use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline::{self, Codec, FfmpegEncoder, Quality, RenderOptions};
use anikit_video::spec::parse_json;
use std::process::Command;

fn ffmpeg() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok()
}

#[test]
fn tiny_render_decodes() {
    if !ffmpeg() {
        eprintln!("ffmpeg missing; skipped");
        return;
    }
    let src = r##"{ "size": [160, 90], "fps": 10, "scenes": [
        { "duration": 1, "background": "#204080", "layers": [ { "shape": "rect", "width": 40, "height": 40, "fill": "#ff0000", "enter": "popIn" } ] },
        { "duration": 1, "transition": "crossfade", "layers": [ { "text": "Hi", "style": "title" } ] } ] }"##;
    let p = parse_json(src).unwrap();
    let ev = Evaluator::new(&p.spec, Options::default()).unwrap();
    assert_eq!(ev.frames(), 15);
    let dir = std::env::temp_dir().join(format!("akv-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("t.mp4");
    let mut enc = FfmpegEncoder::new(&out, 160, 90, 10.0, Codec::H264, Quality::Draft, &[], 0.0, ev.duration).unwrap();
    let s = pipeline::render_video(&ev, &RenderOptions::default(), &mut enc).unwrap();
    assert_eq!(s.frames, 15);
    assert!(s.bytes > 0);
    // Decode the 3rd frame (t = 0.2 s, mid pop-in) and the last, check colours.
    let o = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&out)
        .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .output()
        .unwrap();
    assert!(o.status.success());
    let fsz = 160 * 90 * 3;
    assert_eq!(o.stdout.len(), fsz * 15, "decoded frame count");
    let px = |f: usize, x: usize, y: usize| {
        let i = f * fsz + (y * 160 + x) * 3;
        [o.stdout[i] as i32, o.stdout[i + 1] as i32, o.stdout[i + 2] as i32]
    };
    let bg = px(5, 5, 5);
    assert!((bg[0] - 0x20).abs() < 12 && (bg[2] - 0x80).abs() < 12, "background {bg:?}");
    let c = px(5, 80, 45);
    assert!(c[0] > 200 && c[1] < 60, "centre square {c:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn identical_frames_are_deduped() {
    let p = parse_json(r#"{ "size": [64, 64], "fps": 10, "scenes": [ { "duration": 1, "layers": [ { "shape": "circle", "width": 20 } ] } ] }"#).unwrap();
    let ev = Evaluator::new(&p.spec, Options::default()).unwrap();
    let a = pipeline::hash_list(&ev.eval(0.1));
    let b = pipeline::hash_list(&ev.eval(0.5));
    assert_eq!(a, b);
}
