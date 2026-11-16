//! OpenH264 (Cisco, via the `openh264` crate) on the same synthetic scenes as
//! ak264's bench. Frames are encoded on one thread per GOP chunk (like ak264's
//! chunk mode) to give it the same parallelism.
use ak264::synth::Scene;
use openh264::encoder::{Complexity, Encoder, EncoderConfig, FrameRate, IntraFramePeriod, QpRange, RateControlMode, UsageType};
use openh264::formats::YUVSlices;
use std::process::Command;
use std::time::Instant;

/// Decode an ak264 stream with OpenH264's decoder and compare with ffmpeg's output.
fn crosscheck(cabac: bool) {
    use openh264::decoder::Decoder;
    use openh264::formats::YUVSource;
    let (w, h) = (320usize, 192usize);
    let s = Scene::preset("typical", w as u32, h as u32);
    let bufs: Vec<ak264::YuvBuffer> = (0..30).map(|t| s.frame(t)).collect();
    let frames: Vec<ak264::YuvFrame> = bufs.iter().map(|b| b.frame()).collect();
    let cfg = ak264::EncoderConfig { width: w as u32, height: h as u32, gop: 10, cabac, ..Default::default() };
    let stream = ak264::encode_chunks(&cfg, &frames, &[]);
    let mut dec = Decoder::new().unwrap();
    let mut ours = Vec::new();
    let mut push = |y: &dyn YUVSource| {
        let (ys, us, vs) = y.strides();
        for r in 0..h {
            ours.extend_from_slice(&y.y()[r * ys..r * ys + w]);
        }
        for r in 0..h / 2 {
            ours.extend_from_slice(&y.u()[r * us..r * us + w / 2]);
        }
        for r in 0..h / 2 {
            ours.extend_from_slice(&y.v()[r * vs..r * vs + w / 2]);
        }
    };
    for nal in openh264::nal_units(&stream) {
        if let Ok(Some(f)) = dec.decode(nal) {
            push(&f);
        }
    }
    for f in dec.flush_remaining().unwrap() {
        push(&f);
    }
    let p = std::env::temp_dir().join("xchk.h264");
    std::fs::write(&p, &stream).unwrap();
    let o = Command::new("ffmpeg").args(["-v", "error", "-i"]).arg(&p).args(["-f", "rawvideo", "-pix_fmt", "yuv420p", "-"]).output().unwrap();
    println!(
        "cross-check {}: OpenH264 decoded {} frames, ffmpeg {} frames, identical: {}",
        if cabac { "CABAC/Main" } else { "CAVLC/Baseline" },
        ours.len() / (w * h * 3 / 2),
        o.stdout.len() / (w * h * 3 / 2),
        ours == o.stdout
    );
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.get(1).map(|s| s == "crosscheck").unwrap_or(false) {
        crosscheck(false);
        crosscheck(true);
        return;
    }
    let scene = a.get(1).cloned().unwrap_or("typical".into());
    let n: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(240);
    let qps: Vec<u8> = if a.len() > 3 { a[3..].iter().map(|s| s.parse().unwrap()).collect() } else { vec![26] };
    let screen = std::env::var("USAGE").map(|u| u == "screen").unwrap_or(false);
    let (w, h) = (1920usize, 1080usize);
    let s = Scene::preset(&scene, w as u32, h as u32);
    let bufs: Vec<ak264::YuvBuffer> = (0..n as u32).map(|t| s.frame(t)).collect();
    let src: Vec<u8> = bufs.iter().flat_map(|b| b.to_bytes()).collect();
    let fsz = src.len() / n;
    for &qp in &qps {
      for cx in [Complexity::Low, Complexity::Medium] {
        let gop = 60usize;
        let t = Instant::now();
        let chunks: Vec<Vec<u8>> = std::thread::scope(|sc| {
            let hs: Vec<_> = bufs
                .chunks(gop)
                .map(|chunk| {
                    sc.spawn(move || {
                        let cfg = EncoderConfig::new()
                            .max_frame_rate(FrameRate::from_hz(30.0))
                            .rate_control_mode(RateControlMode::Off)
                            .qp(QpRange::new(qp, qp))
                            .skip_frames(false)
                            .complexity(cx)
                            .usage_type(if screen { UsageType::ScreenContentRealTime } else { UsageType::CameraVideoRealTime })
                            .intra_frame_period(IntraFramePeriod::from_num_frames(gop as u32));
                        let mut e = Encoder::with_api_config(openh264::OpenH264API::from_source(), cfg).unwrap();
                        let mut out = Vec::new();
                        for b in chunk {
                            let yuv = YUVSlices::new((&b.y, &b.u, &b.v), (w, h), (w, w / 2, w / 2));
                            let bs = e.encode(&yuv).unwrap();
                            bs.write_vec(&mut out);
                        }
                        out
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let dt = t.elapsed().as_secs_f64();
        let stream: Vec<u8> = chunks.concat();
        let p = std::env::temp_dir().join("oh264.h264");
        std::fs::write(&p, &stream).unwrap();
        let o = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&p)
            .args(["-f", "rawvideo", "-pix_fmt", "yuv420p", "-"])
            .output()
            .unwrap();
        let dec = o.stdout;
        let (mut sy, mut ny, mut sa) = (0f64, 0usize, 0f64);
        for (fa, fb) in dec.chunks(fsz).zip(src.chunks(fsz)) {
            for (i, (&x, &y)) in fa.iter().zip(fb).enumerate() {
                let d = (x as f64 - y as f64).powi(2);
                if i < w * h {
                    sy += d;
                    ny += 1;
                }
                sa += d;
            }
        }
        let p = |s: f64, n: usize| 10.0 * (65025.0 / (s / n as f64)).log10();
        println!(
            "| OpenH264 {:?} (GOP chunks, {} frames decoded) | qp {qp} | {:.0} | {} | {:.2} | {:.2} |",
            cx,
            dec.len() / fsz,
            n as f64 / dt,
            stream.len() / 1024,
            p(sy, ny),
            p(sa, dec.len().min(src.len()))
        );
      }
    }
}
