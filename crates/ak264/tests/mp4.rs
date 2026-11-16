mod common;
use ak264::synth::Scene;
use ak264::*;
use common::*;
use std::process::Command;

#[test]
fn mp4_decodes_like_annexb() {
    let s = Scene::new(192, 108);
    let bufs: Vec<_> = (0..30).map(|t| s.frame(t)).collect();
    let frames: Vec<_> = bufs.iter().map(|b| b.frame()).collect();
    let cfg = EncoderConfig { width: 192, height: 108, gop: 10, fps: 30.0, ..Default::default() };
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
    let _ = std::fs::remove_file(&p);
    let _ = std::fs::remove_file(&raw);
}
