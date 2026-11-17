//! Per-clip mode/bit statistics: stats file.yuv frames qp
use ak264::*;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (path, n, qp): (&str, usize, u8) = (&a[1], a[2].parse().unwrap(), a[3].parse().unwrap());
    let (w, h) = (1920u32, 1080u32);
    let fs = (w * h * 3 / 2) as usize;
    let data = std::fs::read(path).unwrap();
    let mut e = Encoder::new(EncoderConfig { width: w, height: h, qp, gop: 60, deblock: true, aq_strength: 0.0, ssim_floor: 0.0, ..Default::default() });
    let (mut idr_b, mut p_b) = (0usize, 0usize);
    let mut t = FrameStats::default();
    for i in 0..n.min(data.len() / fs) {
        let b = &data[i * fs..(i + 1) * fs];
        let (ys, cs) = ((w * h) as usize, (w * h / 4) as usize);
        let f = YuvFrame { width: w, height: h, y: &b[..ys], u: &b[ys..ys + cs], v: &b[ys + cs..], y_stride: w as usize, c_stride: w as usize / 2 };
        let len = e.encode(&f, &FrameHints::default()).len();
        let s = e.last_stats();
        if s.idr {
            idr_b += len;
            continue;
        }
        p_b += len;
        if std::env::var("PER").is_ok() {
            println!("{i} {len} skip {} inter {} i16 {} i4 {}", s.skip, s.inter, s.intra16, s.intra4);
        }
        t.skip += s.skip;
        t.inter += s.inter;
        t.intra16 += s.intra16;
        t.intra4 += s.intra4;
        t.changed += s.changed;
        t.bits_intra += s.bits_intra;
        t.bits_inter += s.bits_inter;
    }
    let mbs = (t.skip + t.inter + t.intra16 + t.intra4) as f64;
    println!(
        "IDR {:.2} MB, P {:.2} MB | P MBs: skip {:.1}% inter {:.1}% i16 {:.1}% i4 {:.1}% | P bits: intra {:.2} MB inter {:.2} MB",
        idr_b as f64 / 1e6,
        p_b as f64 / 1e6,
        100.0 * t.skip as f64 / mbs,
        100.0 * t.inter as f64 / mbs,
        100.0 * t.intra16 as f64 / mbs,
        100.0 * t.intra4 as f64 / mbs,
        t.bits_intra as f64 / 8e6,
        t.bits_inter as f64 / 8e6
    );
}
