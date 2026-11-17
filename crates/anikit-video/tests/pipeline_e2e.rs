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
    let mut enc = FfmpegEncoder::new(&out, 160, 90, 10.0, Codec::H264, Quality::Draft, None).unwrap();
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

#[test]
fn ak264_with_sfx_audio() {
    if !ffmpeg() {
        return;
    }
    let src = r#"{ "size": [128, 72], "fps": 15, "scenes": [ { "duration": 1, "layers": [ { "text": "x", "enter": "pop" } ],
        "audio": [ { "sfx": "whoosh", "at": 0.1 }, { "sfx": "impact", "at": 0.6, "volume": "-3dB" } ] } ] }"#;
    let ev = Evaluator::new(&parse_json(src).unwrap().spec, Options::default()).unwrap();
    assert_eq!(ev.audio.len(), 2);
    let dir = std::env::temp_dir().join(format!("akv-e2e-a-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = pipeline::mix_audio(&ev, 0.0, ev.duration, &dir.join("a.wav")).unwrap();
    let out = dir.join("a.mp4");
    let mut enc = FfmpegEncoder::new(&out, 128, 72, 15.0, Codec::Ak264, Quality::Good, wav.as_deref()).unwrap();
    let s = pipeline::render_video(&ev, &RenderOptions::default(), &mut enc).unwrap();
    assert_eq!(s.frames, 15);
    let o = Command::new("ffprobe").args(["-v", "error", "-show_entries", "stream=codec_name", "-of", "csv=p=0"]).arg(&out).output().unwrap();
    let streams = String::from_utf8_lossy(&o.stdout);
    assert!(streams.contains("h264") && streams.contains("aac"), "{streams}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fused_ak264_decodes_and_matches() {
    if !ffmpeg() {
        return;
    }
    // Holds (static spans), a moving shape and a cut: exercises held, partial and full frames across GOP chunks.
    let src = r##"{ "size": [192, 108], "fps": 30, "scenes": [
        { "duration": 2, "background": "#204080", "layers": [ { "shape": "rect", "width": 30, "height": 30, "fill": "#ff0000", "x": "=20 + 60 * clamp(t - 1, 0, 1)" } ] },
        { "duration": 1.5, "transition": "cut", "background": "#103010", "layers": [ { "shape": "circle", "width": 40, "fill": "#ffffff" } ] } ] }"##;
    let ev = Evaluator::new(&parse_json(src).unwrap().spec, Options::default()).unwrap();
    let dir = std::env::temp_dir().join(format!("akv-fused-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("f.mp4");
    let s = pipeline::render_ak264(&ev, &RenderOptions::default(), &out, None).unwrap();
    assert_eq!(s.frames, 105);
    assert!(s.deduped > 30, "holds should be skipped: {}", s.deduped);
    let o = Command::new("ffmpeg").args(["-v", "error", "-i"]).arg(&out).args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-"]).output().unwrap();
    assert!(o.status.success());
    let fsz = 192 * 108 * 3;
    assert_eq!(o.stdout.len(), fsz * 105);
    let px = |f: usize, x: usize, y: usize| {
        let i = f * fsz + (y * 192 + x) * 3;
        [o.stdout[i] as i32, o.stdout[i + 1] as i32, o.stdout[i + 2] as i32]
    };
    // Frame 15 (held): square at x = 20 (centre), background elsewhere.
    assert!(px(15, 20, 54)[0] > 200, "{:?}", px(15, 20, 54));
    assert!((px(15, 150, 20)[2] - 0x80).abs() < 14, "{:?}", px(15, 150, 20));
    // Frame 59 (after the move): square at x = 80.
    assert!(px(59, 80, 54)[0] > 200 && px(59, 20, 54)[0] < 80, "{:?} {:?}", px(59, 80, 54), px(59, 20, 54));
    // Frame 90 (second scene): white circle in the middle, green background.
    assert!(px(90, 96, 54)[1] > 220 && (px(90, 5, 5)[1] - 0x30).abs() < 14, "{:?} {:?}", px(90, 96, 54), px(90, 5, 5));
    // Every frame must match the plain (non-fused) path: catches chunk-boundary and change-map bugs.
    let refp = dir.join("ref.mp4");
    let mut enc = FfmpegEncoder::new(&refp, 192, 108, 30.0, Codec::H264, Quality::Best, None).unwrap();
    pipeline::render_video(&ev, &RenderOptions::default(), &mut enc).unwrap();
    let r = Command::new("ffmpeg").args(["-v", "error", "-i"]).arg(&refp).args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-"]).output().unwrap();
    assert_eq!(r.stdout.len(), o.stdout.len());
    for f in 0..105 {
        let a = &o.stdout[f * fsz..(f + 1) * fsz];
        let b = &r.stdout[f * fsz..(f + 1) * fsz];
        let mad = a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as u64).sum::<u64>() as f64 / fsz as f64;
        assert!(mad < 3.0, "frame {f}: mean abs diff {mad:.2} vs the plain path");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fused_many_chunks_match_plain_path() {
    if !ffmpeg() {
        return;
    }
    // Something moves every frame, so a wrong partial copy at any chunk start shows.
    let src = r##"{ "size": [160, 96], "fps": 30, "scenes": [ { "duration": 4, "background": "#202020", "layers": [
        { "shape": "rect", "width": 24, "height": 24, "fill": "#ff4040", "x": "=12 + 136 * fract(t / 2)", "y": 30 },
        { "shape": "circle", "width": 20, "fill": "#40ff40", "x": 80, "y": "=20 + 56 * fract(t / 1.3)" } ] } ] }"##;
    let ev = Evaluator::new(&parse_json(src).unwrap().spec, Options::default()).unwrap();
    let n = ev.frames() as usize;
    let cfg = ak264::EncoderConfig { width: 160, height: 96, fps: 30.0, qp: 18, gop: 7, deblock: false, threads: 1, full_range: false, ..Default::default() };
    let annexb = pipeline::render_fused(&ev, &RenderOptions::default(), &cfg, &pipeline::Profile::default());
    let dec = |args: &[&str], input: Option<&[u8]>| {
        use std::io::Write;
        let mut c = Command::new("ffmpeg").args(["-v", "error"]).args(args).args(["-f", "rawvideo", "-pix_fmt", "gray", "-"]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
        let mut sin = c.stdin.take().unwrap();
        if let Some(b) = input {
            sin.write_all(b).unwrap();
        }
        drop(sin);
        c.wait_with_output().unwrap().stdout
    };
    let got = dec(&["-f", "h264", "-i", "-"], Some(&annexb));
    let dir = std::env::temp_dir().join(format!("akv-chunks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let refp = dir.join("ref.mp4");
    let mut enc = FfmpegEncoder::new(&refp, 160, 96, 30.0, Codec::H264, Quality::Best, None).unwrap();
    pipeline::render_video(&ev, &RenderOptions::default(), &mut enc).unwrap();
    let want = dec(&["-i", refp.to_str().unwrap()], None);
    let fsz = 160 * 96;
    assert_eq!(got.len(), n * fsz);
    assert_eq!(want.len(), n * fsz);
    for f in 0..n {
        let mad = got[f * fsz..(f + 1) * fsz].iter().zip(&want[f * fsz..(f + 1) * fsz]).map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as u64).sum::<u64>() as f64 / fsz as f64;
        assert!(mad < 2.5, "frame {f}: mean abs diff {mad:.2} vs the plain path");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
