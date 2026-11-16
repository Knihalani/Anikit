//! ak264 benchmark vs x264 (via ffmpeg) on synthetic motion graphics.
//!
//! cargo run --release -p ak264 --example bench -- [scene] [frames] [qp...]
//! scenes: typical | hold | full
use ak264::synth::Scene;
use ak264::*;
use std::process::Command;
use std::time::Instant;

fn psnr_y(a: &[u8], b: &[u8], fsz: usize, ysz: usize) -> (f64, f64) {
    let (mut sy, mut ny, mut sa, mut na) = (0f64, 0usize, 0f64, 0usize);
    for (fa, fb) in a.chunks(fsz).zip(b.chunks(fsz)) {
        for (i, (&x, &y)) in fa.iter().zip(fb).enumerate() {
            let d = (x as f64 - y as f64).powi(2);
            if i < ysz {
                sy += d;
                ny += 1;
            }
            sa += d;
            na += 1;
        }
    }
    let p = |s: f64, n: usize| if s == 0.0 { 99.0 } else { 10.0 * (255.0f64 * 255.0 / (s / n as f64)).log10() };
    (p(sy, ny), p(sa, na))
}

fn decode(path: &str) -> Vec<u8> {
    let o = Command::new("ffmpeg")
        .args(["-v", "error", "-i", path, "-f", "rawvideo", "-pix_fmt", "yuv420p", "-"])
        .output()
        .unwrap();
    assert!(o.stderr.is_empty(), "{}", String::from_utf8_lossy(&o.stderr));
    o.stdout
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scene = args.get(1).map(|s| s.as_str()).unwrap_or("typical").to_string();
    let n: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(120);
    let qps: Vec<u8> = if args.len() > 3 { args[3..].iter().map(|s| s.parse().unwrap()).collect() } else { vec![22] };
    let x264: bool = std::env::var("X264").map(|v| v != "0").unwrap_or(true);
    let (w, h) = (1920u32, 1080u32);
    let s = Scene::preset(&scene, w, h);
    let t0 = Instant::now();
    let bufs: Vec<YuvBuffer> = (0..n).map(|t| s.frame(t)).collect();
    eprintln!("generated {n} frames in {:.2}s", t0.elapsed().as_secs_f64());
    let src: Vec<u8> = bufs.iter().flat_map(|b| b.to_bytes()).collect();
    let fsz = bufs[0].to_bytes().len();
    let ysz = (w * h) as usize;
    let dir = std::env::var("BENCH_DIR").unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
    let srcp = format!("{dir}/ak264-src-{scene}.yuv");
    std::fs::write(&srcp, &src).unwrap();
    let frames: Vec<YuvFrame> = bufs.iter().map(|b| b.frame()).collect();
    println!("scene={scene} {w}x{h} frames={n}");
    println!("| encoder | settings | fps | size KB | PSNR-Y | PSNR-all |");
    println!("|---|---|---|---|---|---|");
    for &qp in &qps {
        for (label, threads, chunked) in [("ak264 1 thread", 1usize, false), ("ak264 4 slices", 4, false), ("ak264 chunks", 1, true)]
        {
            let cfg = EncoderConfig { width: w, height: h, qp, gop: 60, fps: 30.0, threads, ..Default::default() };
            // warm-up
            let _ = encode_chunks(&cfg, &frames[..2.min(frames.len())], &[]);
            let t = Instant::now();
            let stream = if chunked {
                encode_chunks(&cfg, &frames, &[])
            } else {
                let mut e = Encoder::new(cfg.clone());
                let mut out = Vec::new();
                for f in &frames {
                    out.extend_from_slice(e.encode(f, &FrameHints::default()));
                }
                out
            };
            let dt = t.elapsed().as_secs_f64();
            let p = format!("{dir}/ak264-{scene}-{qp}.h264");
            std::fs::write(&p, &stream).unwrap();
            let dec = decode(&p);
            let (py, pa) = psnr_y(&dec, &src, fsz, ysz);
            println!("| {label} | qp {qp} | {:.0} | {} | {py:.2} | {pa:.2} |", n as f64 / dt, stream.len() / 1024);
        }
    }
    if !x264 {
        return;
    }
    let crfs: Vec<String> = std::env::var("CRF").unwrap_or_else(|_| "23".into()).split(',').map(|s| s.to_string()).collect();
    for preset in ["ultrafast", "veryfast", "medium"] {
        for crf in &crfs {
            let out = format!("{dir}/x264-{scene}-{preset}-{crf}.h264");
            let t = Instant::now();
            let o = Command::new("ffmpeg")
                .args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "yuv420p", "-s", &format!("{w}x{h}"), "-r", "30"])
                .args(["-i", &srcp, "-c:v", "libx264", "-preset", preset, "-crf", crf, "-g", "60", &out])
                .output()
                .unwrap();
            let dt = t.elapsed().as_secs_f64();
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
            let size = std::fs::metadata(&out).unwrap().len();
            let dec = decode(&out);
            let (py, pa) = psnr_y(&dec, &src, fsz, ysz);
            println!("| x264 {preset} | crf {crf} | {:.0} | {} | {py:.2} | {pa:.2} |", n as f64 / dt, size / 1024);
        }
    }
}
