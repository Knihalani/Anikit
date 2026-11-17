//! Motion-blur cost and accuracy: per blurred frame, the plain render time, the
//! motion-blur time, and the error against the brute-force reference (64 full frames).
//! `cargo run --release -p anikit-video --example mb_bench -- docs/video/examples/showcase.json [step]`
use anikit_video::eval::{Evaluator, Options};
use anikit_video::render::Renderer;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let path = a.get(1).cloned().unwrap_or_else(|| "docs/video/examples/showcase.json".into());
    let step: u32 = a.get(2).map_or(5, |s| s.parse().unwrap());
    let only: Option<u32> = a.get(3).map(|s| s.parse().unwrap());
    let p = anikit_video::spec::parse_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let ev = Evaluator::new(&p.spec, Options::default()).unwrap();
    let shutter = if ev.shutter > 0.0 { ev.shutter } else { 0.5 / ev.fps };
    let mut r = Renderer::new(ev.assets.snapshot());
    let mut rr = Renderer::new(ev.assets.snapshot());
    rr.set_brute_force_motion(true);
    let (mut tb, mut tm, mut nf, mut worst, mut se, mut np) = (0.0, 0.0, 0, 0u8, 0.0f64, 0usize);
    for i in (0..ev.frames()).step_by(step as usize) {
        if only.is_some_and(|o| o != i) {
            continue;
        }
        let t = i as f32 / ev.fps;
        if ev.is_static(t - shutter * 0.5, t + shutter * 0.5) {
            continue;
        }
        let l = ev.eval(t);
        // Plain cost of a moving frame: warm on the previous frame, then a full render.
        r.render(&ev.eval(t - 1.0 / ev.fps));
        r.invalidate();
        let t0 = Instant::now();
        r.render(&l);
        let b = t0.elapsed().as_secs_f64();
        let mut eval = |t: f32| ev.eval(t);
        r.render(&ev.eval(t - 1.0 / ev.fps));
        let t0 = Instant::now();
        let got = r.render_motion_blur(&mut eval, t, shutter).data_as_u8_slice().to_vec();
        let m = t0.elapsed().as_secs_f64();
        let want = if std::env::var_os("AK_NOREF").is_some() {
            &got[..]
        } else {
            rr.render_motion_blur(&mut eval, t, shutter).data_as_u8_slice()
        };
        let mut fw = 0u8;
        let w = l.width as usize;
        let mut bb = [usize::MAX, usize::MAX, 0, 0];
        for (k, (x, y)) in got.iter().zip(want).enumerate() {
            let d = x.abs_diff(*y);
            if d > 6 {
                let (px, py) = ((k / 4) % w, k / 4 / w);
                bb = [bb[0].min(px), bb[1].min(py), bb[2].max(px), bb[3].max(py)];
            }
            fw = fw.max(d);
            se += (d as f64) * (d as f64);
        }
        np += got.len();
        if std::env::var("AK_MBDUMP").ok().and_then(|v| v.parse::<u32>().ok()) == Some(i) {
            let diff: Vec<u8> = got
                .iter()
                .zip(want)
                .enumerate()
                .map(|(k, (x, y))| if k % 4 == 3 { 255 } else { (x.abs_diff(*y) as u32 * 8).min(255) as u8 })
                .collect();
            for (name, px) in [("got", &got[..]), ("want", want), ("diff", &diff[..])] {
                image::save_buffer(
                    format!("/tmp/claude-0/mb_{i}_{name}.png"),
                    px,
                    l.width,
                    l.height,
                    image::ExtendedColorType::Rgba8,
                )
                .unwrap();
            }
        }
        worst = worst.max(fw);
        tb += b;
        tm += m;
        nf += 1;
        if std::env::var_os("AK_QUIET").is_none() {
            println!(
                "frame {i:4}  plain {:6.1} ms  blur {:6.1} ms  ({:4.1}×)  max err {fw} {bb:?}",
                b * 1e3,
                m * 1e3,
                m / b
            );
        }
    }
    let psnr = 10.0 * (255.0f64 * 255.0 / (se / np.max(1) as f64)).log10();
    println!(
        "{nf} moving frames: plain {:.1} ms, blur {:.1} ms avg ({:.2}×); vs reference max err {worst}, PSNR {psnr:.1} dB",
        tb / nf as f64 * 1e3,
        tm / nf as f64 * 1e3,
        tm / tb
    );
}
