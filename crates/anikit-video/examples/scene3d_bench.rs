//! `scene3d` speed and fidelity benchmark.
//!
//! `cargo run --release -p anikit-video --example scene3d_bench -- [options]`
//!
//! Renders a few consecutive frames of each scene (default: the
//! `examples/video/lookdev/*.json` shots) at full size, prints the time per
//! frame, and, when reference frames exist, compares against them:
//! SSIM (luma), CIE76 ΔE p99.9 and max, and flicker (frame-to-frame change of
//! the luma error, 8-bit levels, p99.9). Thresholds match `qbench`: SSIM ≥
//! 0.995, ΔE p99.9 ≤ 2.3, flicker p99.9 ≤ 4.
//!
//! * `--reference`: write reference frames (converged sampling, every
//!   speed-only shortcut off) instead of timing. Slow.
//! * `--ref DIR` (default `target/scene3d-ref`), `--out DIR` (default
//!   `target/scene3d-bench`): where reference and test frames go.
//! * `--only a,b`, `--frames N` (default 3), `--at T` (default 1.0),
//!   `--scale K` (default 1).

use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline;
use anikit_video::spec;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn build(path: &Path) -> Evaluator {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let parsed = spec::parse_json(&src).unwrap_or_else(|d| panic!("{}: {:?}", path.display(), d.iter().map(|x| x.message.clone()).collect::<Vec<_>>()));
    let base = path.parent().unwrap().to_path_buf();
    let opts = Options { base_dir: base, fonts: Some(Arc::new(anikit_video::text::FontStore::new())), load_assets: true, assets: None };
    let (ev, d) = Evaluator::build(&parsed.spec, &opts);
    ev.unwrap_or_else(|| panic!("{}: {:?}", path.display(), d.iter().map(|x| x.message.clone()).collect::<Vec<_>>()))
}

/// Process CPU time (user + system) in ms, from /proc (Linux); 0 elsewhere.
fn cpu_ms() -> f64 {
    let Ok(st) = std::fs::read_to_string("/proc/self/stat") else { return 0.0 };
    // Fields after the command name (which may contain spaces).
    let rest = &st[st.rfind(')').map(|i| i + 2).unwrap_or(0)..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    let tick = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    // utime and stime are fields 14 and 15 (1-based), i.e. 11 and 12 here, in 1/100 s.
    (tick(11) + tick(12)) * 10.0
}

fn load_png(p: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::open(p).ok()?.to_rgba8();
    Some((img.width(), img.height(), img.into_raw()))
}

fn lin(c: u8) -> f64 {
    let v = c as f64 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn lab(p: &[u8]) -> [f64; 3] {
    let (r, g, b) = (lin(p[0]), lin(p[1]), lin(p[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn luma8(p: &[u8]) -> f64 {
    0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64
}

/// Mean SSIM over 8×8 windows (stride 4) on luma.
fn ssim(a: &[f64], b: &[f64], w: usize, h: usize) -> f64 {
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    let mut sum = 0.0;
    let mut n = 0.0;
    let mut y = 0;
    while y + 8 <= h {
        let mut x = 0;
        while x + 8 <= w {
            let (mut ma, mut mb, mut va, mut vb, mut cv) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for j in 0..8 {
                for i in 0..8 {
                    let k = (y + j) * w + x + i;
                    ma += a[k];
                    mb += b[k];
                }
            }
            ma /= 64.0;
            mb /= 64.0;
            for j in 0..8 {
                for i in 0..8 {
                    let k = (y + j) * w + x + i;
                    let (da, db) = (a[k] - ma, b[k] - mb);
                    va += da * da;
                    vb += db * db;
                    cv += da * db;
                }
            }
            va /= 63.0;
            vb /= 63.0;
            cv /= 63.0;
            sum += ((2.0 * ma * mb + c1) * (2.0 * cv + c2)) / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1.0;
            x += 4;
        }
        y += 4;
    }
    sum / n
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() as f64 - 1.0) * p).round() as usize]
}

struct Cmp {
    ssim: f64,
    de999: f64,
    demax: f64,
    err: Vec<f64>,
}

fn compare(a: &[u8], b: &[u8], w: usize, h: usize) -> Cmp {
    let la: Vec<f64> = a.chunks_exact(4).map(luma8).collect();
    let lb: Vec<f64> = b.chunks_exact(4).map(luma8).collect();
    let mut de: Vec<f64> = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .map(|(p, q)| {
            let (x, y) = (lab(p), lab(q));
            ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
        })
        .collect();
    let demax = de.iter().cloned().fold(0.0, f64::max);
    let err = la.iter().zip(&lb).map(|(x, y)| x - y).collect();
    Cmp { ssim: ssim(&la, &lb, w, h), de999: pct(&mut de, 0.999), demax, err }
}

fn main() {
    let reference = std::env::args().any(|a| a == "--reference");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let refdir = arg("--ref").map(PathBuf::from).unwrap_or(root.join("target/scene3d-ref"));
    let outdir = arg("--out").map(PathBuf::from).unwrap_or(root.join("target/scene3d-bench"));
    let frames: u32 = arg("--frames").map(|s| s.parse().unwrap()).unwrap_or(3);
    let at: f32 = arg("--at").map(|s| s.parse().unwrap()).unwrap_or(1.0);
    let scale: f32 = arg("--scale").map(|s| s.parse().unwrap()).unwrap_or(1.0);
    let only: Option<Vec<String>> = arg("--only").map(|s| s.split(',').map(String::from).collect());
    let names = ["cyc", "chrome", "glass", "panes"];
    std::fs::create_dir_all(&refdir).unwrap();
    std::fs::create_dir_all(&outdir).unwrap();
    if reference {
        // SAFETY: set before any rendering thread starts.
        unsafe { std::env::set_var("ANIKIT_SCENE3D_REFERENCE", "1") };
    }
    println!("{:<8} {:>9} {:>9} {:>8} {:>8} {:>8} {:>8} {:>8}", "scene", "ms/frame", "min ms", "cpu ms", "SSIM", "ΔE99.9", "ΔEmax", "flicker");
    for name in names {
        if only.as_ref().is_some_and(|o| !o.iter().any(|x| x == name)) {
            continue;
        }
        let ev = build(&root.join(format!("examples/video/lookdev/{name}.json")));
        let f0 = (at * ev.fps).round() as u32;
        let mut times = Vec::new();
        let mut cpus = Vec::new();
        let mut cmps: Vec<Cmp> = Vec::new();
        // One warm-up frame (textures, bakes) outside the timing.
        if !reference {
            let _ = pipeline::render_rgba(&ev, pipeline::frame_time(&ev, f0 + 1000), scale);
        }
        for k in 0..frames {
            let t = pipeline::frame_time(&ev, f0 + k);
            let c = Instant::now();
            let c0 = cpu_ms();
            let (w, h, px) = pipeline::render_rgba(&ev, t, scale);
            times.push(c.elapsed().as_secs_f64() * 1e3);
            cpus.push(cpu_ms() - c0);
            let file = format!("{name}-{k}.png");
            if reference {
                pipeline::save_png(&refdir.join(&file), w, h, px).unwrap();
                eprintln!("{name} frame {k}: {:.0} ms", times.last().unwrap());
                continue;
            }
            if let Some((rw, rh, r)) = load_png(&refdir.join(&file)) {
                if (rw, rh) == (w, h) {
                    cmps.push(compare(&px, &r, w as usize, h as usize));
                }
            }
            pipeline::save_png(&outdir.join(&file), w, h, px).unwrap();
        }
        if reference {
            continue;
        }
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        let min = times.iter().cloned().fold(f64::MAX, f64::min);
        let cpu = cpus.iter().sum::<f64>() / cpus.len() as f64;
        if cmps.is_empty() {
            println!("{name:<8} {mean:>9.1} {min:>9.1} {cpu:>8.0}");
            continue;
        }
        let ssim = cmps.iter().map(|c| c.ssim).fold(1.0, f64::min);
        let de = cmps.iter().map(|c| c.de999).fold(0.0, f64::max);
        let demax = cmps.iter().map(|c| c.demax).fold(0.0, f64::max);
        let mut flick = 0.0f64;
        for p in cmps.windows(2) {
            let mut d: Vec<f64> = p[0].err.iter().zip(&p[1].err).map(|(a, b)| (a - b).abs()).collect();
            flick = flick.max(pct(&mut d, 0.999));
        }
        let ok = ssim >= 0.995 && de <= 2.3 && flick <= 4.0;
        println!("{name:<8} {mean:>9.1} {min:>9.1} {cpu:>8.0} {ssim:>8.4} {de:>8.2} {demax:>8.1} {flick:>8.2} {}", if ok { "ok" } else { "FAIL" });
    }
}
