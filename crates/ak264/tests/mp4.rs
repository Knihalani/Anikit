mod common;
use ak264::synth::Scene;
use ak264::*;
use common::*;
use std::process::Command;

#[test]
fn mp4_decodes_like_annexb() {
    for bf in [0u8, 1, 2] {
        mp4_case(bf);
    }
}

fn mp4_case(bf: u8) {
    let s = Scene::new(192, 108);
    let bufs: Vec<_> = (0..30).map(|t| s.frame(t)).collect();
    let frames: Vec<_> = bufs.iter().map(|b| b.frame()).collect();
    let cfg = EncoderConfig { width: 192, height: 108, gop: 10, fps: 30.0, bframes: bf, ..Default::default() };
    let stream = encode_chunks(&cfg, &frames, &[]);
    let mp4 = mp4::mux(&stream, 192, 108, 30.0);
    if !have_ffmpeg() {
        return;
    }
    let p = tmp("t.mp4");
    std::fs::write(&p, &mp4).unwrap();
    let raw = tmp("t.yuv");
    let o = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(&p)
        .args(["-f", "rawvideo", "-pix_fmt", "yuv420p"])
        .arg(&raw)
        .output()
        .unwrap();
    assert!(o.stderr.is_empty(), "{}", String::from_utf8_lossy(&o.stderr));
    let dec = std::fs::read(&raw).unwrap();
    let (_, dec2) = decode(&stream, "mp4ref");
    assert_eq!(dec, dec2);
    let probe = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "stream=nb_frames,r_frame_rate,width,height", "-of", "csv=p=0"])
        .arg(&p)
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&probe.stdout);
    assert!(out.contains("192,108,30/1,30"), "ffprobe: {out}");
    // presentation times: 0, 1/30, 2/30, … (B pictures reordered by ctts, the
    // edit list removes the reordering delay)
    let probe = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v", "-show_entries", "frame=pts_time", "-of", "csv=p=0"])
        .arg(&p)
        .output()
        .unwrap();
    let pts: Vec<f64> = String::from_utf8_lossy(&probe.stdout).lines().filter_map(|l| l.trim().trim_end_matches(',').parse().ok()).collect();
    assert_eq!(pts.len(), 30, "bframes {bf}: {pts:?}");
    for (i, t) in pts.iter().enumerate() {
        assert!((t - i as f64 / 30.0).abs() < 1e-3, "bframes {bf}: frame {i} at {t}");
    }
    let _ = std::fs::remove_file(&p);
    let _ = std::fs::remove_file(&raw);
}

#[test]
fn parallel_render_encode_matches_chunks() {
    let s = Scene::new(128, 80);
    let bufs: Vec<_> = (0..17).map(|t| s.frame(t)).collect();
    let frames: Vec<_> = bufs.iter().map(|b| b.frame()).collect();
    let cfg = EncoderConfig { width: 128, height: 80, gop: 5, ..Default::default() };
    let a = encode_chunks(&cfg, &frames, &[]);
    let b = encode_parallel(&cfg, 17, |t, buf, _| *buf = s.frame(t as u32));
    assert_eq!(a, b);
}
