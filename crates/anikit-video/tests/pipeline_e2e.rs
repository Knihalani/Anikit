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
    assert_eq!(ev.frames(), 20, "a transition never shortens the total (1 s + 1 s)");
    let dir = std::env::temp_dir().join(format!("akv-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("t.mp4");
    let mut enc = FfmpegEncoder::new(&out, 160, 90, 10.0, Codec::H264, Quality::Draft, None).unwrap();
    let s = pipeline::render_video(&ev, &RenderOptions::default(), &mut enc).unwrap();
    assert_eq!(s.frames, 20);
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
    assert_eq!(o.stdout.len(), fsz * 20, "decoded frame count");
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

#[test]
fn fused_odd_size_and_range() {
    if !ffmpeg() {
        return;
    }
    let src = r#"{ "size": [161, 91], "fps": 24, "scenes": [ { "duration": 2, "layers": [ { "shape": "circle", "width": 30, "x": "=20 + 120 * t / 2" } ] } ] }"#;
    let ev = Evaluator::new(&parse_json(src).unwrap().spec, Options::default()).unwrap();
    let dir = std::env::temp_dir().join(format!("akv-odd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("o.mp4");
    let opts = RenderOptions { from: Some(0.5), to: Some(1.5), ..Default::default() };
    let s = pipeline::render_ak264(&ev, &opts, &out, None).unwrap();
    assert_eq!((s.width, s.height, s.frames), (160, 90, 24));
    let o = Command::new("ffprobe").args(["-v", "error", "-count_frames", "-show_entries", "stream=nb_read_frames,width,height", "-of", "csv=p=0"]).arg(&out).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "160,90,24");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn colour_round_trip_every_codec() {
    if !ffmpeg() {
        return;
    }
    let src = r##"{ "size": [96, 64], "fps": 10, "scenes": [ { "duration": 0.5, "background": "#7c5cff", "layers": [] } ] }"##;
    let ev = Evaluator::new(&parse_json(src).unwrap().spec, Options::default()).unwrap();
    let dir = std::env::temp_dir().join(format!("akv-colour-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let decode = |path: &std::path::Path| {
        let o = Command::new("ffmpeg").args(["-v", "error", "-i"]).arg(path).args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]).output().unwrap();
        let i = (32 * 96 + 48) * 3;
        [o.stdout[i] as i32, o.stdout[i + 1] as i32, o.stdout[i + 2] as i32]
    };
    for (codec, file, tol) in [
        (Codec::H264, "c.mp4", 4),
        (Codec::H265, "c265.mp4", 4),
        (Codec::Vp9, "c.webm", 4),
        (Codec::Av1, "c.mkv", 5),
        (Codec::ProRes, "c.mov", 4),
        (Codec::Gif, "c.gif", 6),
        (Codec::Png, "c.png", 0),
    ] {
        let out = dir.join(file);
        let mut enc = FfmpegEncoder::new(&out, 96, 64, 10.0, codec, Quality::Best, None).unwrap();
        pipeline::render_video(&ev, &RenderOptions::default(), &mut enc).unwrap();
        let read = if codec == Codec::Png { dir.join("c_00001.png") } else { out.clone() };
        let c = decode(&read);
        let want = [0x7c, 0x5c, 0xff];
        let err = (0..3).map(|k| (c[k] - want[k]).abs()).max().unwrap();
        assert!(err <= tol, "{codec:?}: decoded {c:?}, want {want:?}");
    }
    let out = dir.join("ak.mp4");
    pipeline::render_ak264(&ev, &RenderOptions::default(), &out, None).unwrap();
    let c = decode(&out);
    assert!((c[0] - 0x7c).abs() <= 4 && (c[1] - 0x5c).abs() <= 4 && (c[2] - 0xff).abs() <= 4, "ak264: {c:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn long_render_with_audio_does_not_hang() {
    if !ffmpeg() {
        return;
    }
    // 60 s with audio through the fused ak264 path (this used to stall on a raw h264 pipe).
    let src = r#"{ "size": [160, 90], "fps": 30, "scenes": [ { "duration": 60, "layers": [ { "shape": "circle", "width": 20, "x": "=10 + 140 * fract(t / 5)" } ] } ],
        "audio": [ { "sfx": "riser", "at": 0, "duration": 3 }, { "sfx": "impact", "at": 58 } ] }"#;
    let ev = Evaluator::new(&parse_json(src).unwrap().spec, Options::default()).unwrap();
    let dir = std::env::temp_dir().join(format!("akv-long-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = pipeline::mix_audio(&ev, 0.0, ev.duration, &dir.join("a.wav")).unwrap();
    let out = dir.join("long.mp4");
    let t = std::time::Instant::now();
    pipeline::render_ak264(&ev, &RenderOptions::default(), &out, wav.as_deref()).unwrap();
    assert!(t.elapsed().as_secs() < 60, "too slow / stalled");
    let o = Command::new("ffprobe").args(["-v", "error", "-show_entries", "stream=codec_name,duration", "-of", "csv=p=0"]).arg(&out).output().unwrap();
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(s.contains("h264,60.0") && s.contains("aac,60.0"), "{s}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Output must be byte-identical across runs and thread counts. Text faces that are
/// first used mid-render (an emoji, a counter) used to recreate a worker's renderer at
/// a moment set by other threads, and chunk sizes depended on the core count.
#[test]
fn render_is_byte_identical_across_runs_and_thread_counts() {
    let src = r##"{ "size": [320, 180], "fps": 30, "scenes": [
        { "duration": 2, "layers": [ { "text": "Launch day", "style": "title", "enter": "wave by char" },
            { "text": "=floor(t * 40) + 1000", "y": 140, "size": 20 },
            { "shape": "circle", "width": 30, "fill": "$accent", "x": "=40 + 240 * fract(t / 1.5)", "y": 40 } ] },
        { "duration": 2, "transition": "push", "layers": [ { "text": "Ship it 🚀", "style": "heading", "enter": "pop by word" },
            { "component": "bar-chart", "props": { "values": [3, 7, 5, 9] }, "width": 200, "height": 60, "y": 140 } ] } ] }"##;
    let dir = std::env::temp_dir().join(format!("akv-det-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let render = |threads: usize, k: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            let ev = Evaluator::new(&parse_json(src).unwrap().spec, Options::default()).unwrap();
            let out = dir.join(format!("d{threads}-{k}.mp4"));
            pipeline::render_ak264(&ev, &RenderOptions::default(), &out, None).unwrap();
            std::fs::read(&out).unwrap()
        })
    };
    let a = render(4, 0);
    assert!(a.len() > 1000);
    assert!(a == render(4, 1), "two 4-thread renders differ");
    assert!(a == render(1, 0), "1-thread and 4-thread renders differ");
    assert!(a == render(3, 0), "3-thread and 4-thread renders differ");
    let _ = std::fs::remove_dir_all(&dir);
}
