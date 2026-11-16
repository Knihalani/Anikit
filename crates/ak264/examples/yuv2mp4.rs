//! Encode raw yuv420p from a file (or `-` for stdin) to MP4 or Annex-B.
//! cargo run --release -p ak264 --example yuv2mp4 -- in.yuv 1920x1080 30 out.mp4 [qp] [gop]
use ak264::*;
use std::io::Read;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage: yuv2mp4 <in.yuv|-> <WxH> <fps> <out.mp4|out.h264> [qp] [gop]");
        std::process::exit(2);
    }
    let (w, h) = a[2].split_once('x').map(|(w, h)| (w.parse::<u32>().unwrap(), h.parse::<u32>().unwrap())).unwrap();
    let fps: f64 = a[3].parse().unwrap();
    let qp: u8 = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(22);
    let gop: u32 = a.get(6).and_then(|s| s.parse().ok()).unwrap_or(250);
    let mut data = Vec::new();
    if a[1] == "-" {
        std::io::stdin().read_to_end(&mut data).unwrap();
    } else {
        data = std::fs::read(&a[1]).unwrap();
    }
    let (cw, ch) = ((w as usize).div_ceil(2), (h as usize).div_ceil(2));
    let ysz = (w * h) as usize;
    let fsz = ysz + 2 * cw * ch;
    let frames: Vec<YuvFrame> = data
        .chunks_exact(fsz)
        .map(|f| YuvFrame::new(w, h, &f[..ysz], &f[ysz..ysz + cw * ch], &f[ysz + cw * ch..]))
        .collect();
    let cfg = EncoderConfig { width: w, height: h, fps, qp, gop, ..Default::default() };
    let t = std::time::Instant::now();
    let stream = encode_chunks(&cfg, &frames, &[]);
    let dt = t.elapsed().as_secs_f64();
    let out = if a[4].ends_with(".mp4") { mp4::mux(&stream, w, h, fps) } else { stream };
    std::fs::write(&a[4], &out).unwrap();
    eprintln!("{} frames in {:.3}s ({:.0} fps), {} bytes", frames.len(), dt, frames.len() as f64 / dt, out.len());
}
