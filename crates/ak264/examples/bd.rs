//! Rate-quality runs for BD-rate comparisons.
//! CLIPS="name=path.yuv:frames,..." QPS="20,23,26" OUT=dir cargo run --release -p ak264 --example bd
//! Writes `{OUT}/{name}-{qp}.h264` and prints `name qp bytes seconds`. 1920×1080 I420 input,
//! the anikit-video defaults (CABAC, deblock, quarter-pel, AQ off), SSIM floor off, 60-frame chunks.
use ak264::*;
use std::time::Instant;

fn main() {
    let clips = std::env::var("CLIPS").expect("CLIPS");
    let qps: Vec<u8> = std::env::var("QPS").unwrap_or("20,23,26".into()).split(',').map(|s| s.parse().unwrap()).collect();
    let out = std::env::var("OUT").unwrap_or("/tmp".into());
    let env = |k: &str, d: f32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let (w, h) = (1920u32, 1080u32);
    let fs = (w * h * 3 / 2) as usize;
    for c in clips.split(',') {
        let (name, rest) = c.split_once('=').unwrap();
        let (path, n) = rest.rsplit_once(':').unwrap();
        let n: usize = n.parse().unwrap();
        let mut f = std::fs::File::open(path).unwrap();
        let mut data = vec![0u8; fs * n];
        use std::io::Read;
        let got = f.read(&mut data).unwrap();
        let mut got = got;
        while got < data.len() {
            let k = f.read(&mut data[got..]).unwrap();
            if k == 0 {
                break;
            }
            got += k;
        }
        let n = got / fs;
        let (ys, cs) = ((w * h) as usize, (w * h / 4) as usize);
        let frames: Vec<YuvFrame> = (0..n)
            .map(|i| {
                let b = &data[i * fs..(i + 1) * fs];
                YuvFrame { width: w, height: h, y: &b[..ys], u: &b[ys..ys + cs], v: &b[ys + cs..], y_stride: w as usize, c_stride: w as usize / 2 }
            })
            .collect();
        for &qp in &qps {
            let cfg = EncoderConfig {
                width: w,
                height: h,
                qp,
                gop: env("GOP", 60.0) as u32,
                deblock: env("DEBLOCK", 1.0) > 0.0,
                aq_strength: env("AQ", 0.0),
                ssim_floor: env("SSIM", 0.0),
                ssim_max_drop: env("DROP", 8.0) as u8,
                bframes: env("BF", 0.0) as u8,
                transform8: env("T8", 1.0) > 0.0,
                partitions: env("PART", 0.0) > 0.0,
                subpel: env("SUBPEL", 1.0) > 0.0,
                ..Default::default()
            };
            let t = Instant::now();
            let s = encode_chunks(&cfg, &frames, &[]);
            let dt = t.elapsed().as_secs_f64();
            std::fs::write(format!("{out}/{name}-{qp}.h264"), &s).unwrap();
            println!("{name} {qp} {} {dt:.2}", s.len());
        }
    }
}
