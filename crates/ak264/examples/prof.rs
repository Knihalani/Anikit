use ak264::synth::Scene;
use ak264::*;
use std::time::Instant;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scene = args.get(1).cloned().unwrap_or("typical".into());
    let n: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(60);
    let deblock = std::env::var("DEBLOCK").map(|v| v != "0").unwrap_or(true);
    let (w, h) = (1920, 1080);
    let mut s = Scene::new(w, h);
    match scene.as_str() {
        "hold" => { s.cards = 1; s.speed = 2; }
        "full" => { s.moving_bg = true; s.cards = 6; s.speed = 6; }
        _ => { s.cards = 4; s.speed = 5; }
    }
    let bufs: Vec<YuvBuffer> = (0..n).map(|t| s.frame(t)).collect();
    let cfg = EncoderConfig { width: w, height: h, qp: 22, gop: 1000, deblock, ..Default::default() };
    let mut best = (f64::MAX, 0.0);
    for rep in 0..5 {
        let mut e = Encoder::new(cfg.clone());
        let t = Instant::now();
        let mut bytes = 0;
        let mut st = FrameStats::default();
        let mut tidr = 0.0;
        for b in &bufs {
            let t1 = Instant::now();
            bytes += e.encode(&b.frame(), &FrameHints::default()).len();
            if e.last_stats().idr { tidr += t1.elapsed().as_secs_f64(); }
            if rep == 4 && std::env::var("PERFRAME").is_ok() { eprint!("{} ", e.last_stats().bytes); }
            let s = e.last_stats();
            st.skip += s.skip; st.inter += s.inter; st.intra16 += s.intra16; st.intra4 += s.intra4; st.changed += s.changed;
        }
        let dt = t.elapsed().as_secs_f64();
        if dt < best.0 { best = (dt, tidr); }
        let (dt, tidr) = best;
        if rep == 4 {
            println!("{scene}: IDR {:.1} ms, P {:.0} fps; {:.0} fps, {} KB; per frame: skip {} inter {} i16 {} i4 {} changed {}", tidr * 1e3, (n - 1) as f64 / (dt - tidr), n as f64 / dt, bytes / 1024,
                st.skip / n, st.inter / n, st.intra16 / n, st.intra4 / n, st.changed / n);
        }
    }
}
