//! Dark, dithered gradient: the worst case for blocking.
//! Usage: dark [frames] [qp]  (env AQ=<strength>, DEBLOCK=0/1, OUT=<dir>)
//! Prints size, fps, PSNR-Y, "smooth" PSNR (4×4 box-filtered, where blocking shows
//! and dither doesn't) and the block-edge excess; writes a lifted crop as PGM.
use ak264::*;
use std::time::Instant;

fn hash(x: u32) -> u32 {
    let mut h = x.wrapping_mul(0x9E37_79B1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85EB_CA77);
    h ^ (h >> 13)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    let qp: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(23);
    let file = std::env::var("FILE").ok();
    let (w, h) = if file.is_some() { (1920u32, 1080u32) } else { (960u32, 544u32) };
    let (wu, hu) = (w as usize, h as usize);
    let frames: Vec<YuvBuffer> = if let Some(p) = &file {
        let data = std::fs::read(p).unwrap();
        let (ys, cs) = (wu * hu, wu * hu / 4);
        data.chunks_exact(ys + 2 * cs)
            .take(n as usize)
            .map(|f| YuvBuffer { width: w, height: h, y: f[..ys].to_vec(), u: f[ys..ys + cs].to_vec(), v: f[ys + cs..].to_vec() })
            .collect()
    } else {
        (0..n)
        .map(|t| {
            let mut b = YuvBuffer::new(w, h);
            let drift = t as f64 * 0.37;
            for y in 0..hu {
                for x in 0..wu {
                    // dark vignette-like gradient, 16..44 in limited range
                    let (fx, fy) = ((x as f64 + drift) / wu as f64, y as f64 / hu as f64);
                    let g = 16.0 + 22.0 * (1.0 - ((fx - 0.3).powi(2) + (fy - 0.4).powi(2)).sqrt()) + 6.0 * fx;
                    let r = hash((y * wu + x) as u32 ^ hash(t + 1));
                    let d = ((r & 0xff) as f64 + ((r >> 8) & 0xff) as f64) / 510.0 - 0.5; // triangular
                    b.y[y * wu + x] = (g + d).round().clamp(0.0, 255.0) as u8;
                }
            }
            for y in 0..hu / 2 {
                for x in 0..wu / 2 {
                    let fx = (x * 2) as f64 / wu as f64;
                    let r = hash((y * wu + x) as u32 ^ hash(t + 7777));
                    let d = (r & 0xff) as f64 / 255.0 - 0.5;
                    b.u[y * wu / 2 + x] = (131.0 + 3.0 * fx + d).round() as u8;
                    b.v[y * wu / 2 + x] = (126.0 - 2.0 * fx + d).round() as u8;
                }
            }
            b
        })
        .collect()
    };
    let n = frames.len() as u32;
    let aq: f32 = std::env::var("AQ").ok().and_then(|v| v.parse().ok()).unwrap_or(-1.0);
    let deblock = std::env::var("DEBLOCK").map(|v| v != "0").unwrap_or(true);
    let mut cfg = EncoderConfig { width: w, height: h, qp, gop: 60, deblock, ..Default::default() };
    if aq >= 0.0 {
        cfg.aq_strength = aq;
    }
    let mut e = Encoder::new(cfg);
    let t0 = Instant::now();
    let mut bytes = 0;
    let mut recon = Vec::new();
    for f in &frames {
        bytes += e.encode(&f.frame(), &FrameHints::default()).len();
        recon.push(e.reconstruction());
    }
    let mut dt = t0.elapsed().as_secs_f64();
    if let Ok(x) = std::env::var("X264") {
        // reference: x264 at CRF `x` (veryfast) through ffmpeg
        let dir = std::env::temp_dir();
        let (src, enc) = (dir.join("dark-src.yuv"), dir.join("dark-x264.mp4"));
        let raw: Vec<u8> = frames.iter().flat_map(|f| f.to_bytes()).collect();
        std::fs::write(&src, &raw).unwrap();
        let t = Instant::now();
        let ok = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "yuv420p", "-s", &format!("{w}x{h}"), "-r", "30", "-i"])
            .arg(&src)
            .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", &x])
            .arg(&enc)
            .status()
            .unwrap();
        assert!(ok.success());
        dt = t.elapsed().as_secs_f64();
        bytes = std::fs::metadata(&enc).unwrap().len() as usize;
        let out = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&enc)
            .args(["-f", "rawvideo", "-pix_fmt", "yuv420p", "-"])
            .output()
            .unwrap();
        let fs = raw.len() / frames.len();
        for (i, r) in recon.iter_mut().enumerate() {
            let d = &out.stdout[i * fs..(i + 1) * fs];
            r.y.copy_from_slice(&d[..wu * hu]);
        }
    }
    // metrics on luma
    let (mut se, mut sse_s, mut edge, mut inner, mut edge_s, mut inner_s) = (0f64, 0f64, 0f64, 0f64, 0f64, 0f64);
    let (mut ne, mut ni) = (0f64, 0f64);
    let box4 = |p: &[u8], x: usize, y: usize| -> f64 {
        let mut s = 0u32;
        for j in 0..4 {
            for i in 0..4 {
                s += p[(y + j) * wu + x + i] as u32;
            }
        }
        s as f64 / 16.0
    };
    for (f, r) in frames.iter().zip(&recon) {
        for i in 0..wu * hu {
            let d = f.y[i] as f64 - r.y[i] as f64;
            se += d * d;
        }
        for y in (0..hu - 4).step_by(2) {
            for x in (0..wu - 4).step_by(2) {
                let d = box4(&f.y, x, y) - box4(&r.y, x, y);
                sse_s += d * d;
            }
        }
        // step across 8-pixel boundaries vs elsewhere, on 4-row averages (dither averaged out)
        for y in (0..hu - 4).step_by(4) {
            let col = |p: &[u8], x: usize| (0..4).map(|j| p[(y + j) * wu + x] as f64).sum::<f64>() / 4.0;
            for x in 1..wu {
                let (a, b) = ((col(&r.y, x) - col(&r.y, x - 1)).abs(), (col(&f.y, x) - col(&f.y, x - 1)).abs());
                if x % 8 == 0 {
                    edge += a;
                    edge_s += b;
                    ne += 1.0;
                } else {
                    inner += a;
                    inner_s += b;
                    ni += 1.0;
                }
            }
        }
    }
    // Flicker: how often a 4×4 block mean of the reconstruction jumps by ≥ 0.75
    // between frames where the source's barely moves (< 0.25).
    let (mut flick, mut nflick) = (0f64, 0f64);
    for k in 1..frames.len() {
        if k % 60 == 0 {
            continue; // scene cut / IDR
        }
        for y in (0..hu - 4).step_by(4) {
            for x in (0..wu - 4).step_by(4) {
                let ds = (box4(&frames[k].y, x, y) - box4(&frames[k - 1].y, x, y)).abs();
                if ds < 0.25 {
                    let dr = (box4(&recon[k].y, x, y) - box4(&recon[k - 1].y, x, y)).abs();
                    flick += (dr >= 0.75) as u32 as f64;
                    nflick += 1.0;
                }
            }
        }
    }
    let npx = (wu * hu * frames.len()) as f64;
    let ns = ((hu - 4) / 2 * (wu - 4) / 2 * frames.len()) as f64;
    let psnr = |sse: f64, n: f64| 10.0 * (255.0f64 * 255.0 / (sse / n).max(1e-9)).log10();
    let excess = (edge / ne - inner / ni) - (edge_s / ne - inner_s / ni);
    println!(
        "{} qp {qp} aq {aq}: {:.0} fps, {} KB, PSNR-Y {:.2}, smooth {:.2}, block-edge excess {:.3}, flicker {:.3}%",
        std::env::var("X264").map(|c| format!("x264 crf {c}")).unwrap_or("ak264".into()),
        n as f64 / dt,
        bytes / 1024,
        psnr(se, npx),
        psnr(sse_s, ns),
        excess,
        100.0 * flick / nflick.max(1.0)
    );
    if let Ok(dir) = std::env::var("OUT") {
        // lifted 320×180 crop of the last P frame, ×4 nearest, as PGM
        let k = std::env::var("FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(recon.len() - 2);
        let r = if std::env::var("SRC").is_ok() { &frames[k] } else { &recon[k] };
        let (cw, ch, sc) = (320usize, 180usize, 3usize);
        let lift: (i32, i32) = std::env::var("LIFT").ok().and_then(|c| c.split_once(',').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()))).unwrap_or((16, 6));
        let mut pgm = format!("P5 {} {} 255\n", cw * sc, ch * sc).into_bytes();
        for y in 0..ch * sc {
            for x in 0..cw * sc {
                let (ox, oy) = std::env::var("CROP").ok().and_then(|c| c.split_once(',').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()))).unwrap_or((100usize, 200usize));
                let v = r.y[(oy + y / sc) * wu + ox + x / sc] as i32;
                pgm.push(((v - lift.0) * lift.1).clamp(0, 255) as u8);
            }
        }
        std::fs::write(format!("{dir}/dark-q{qp}-aq{aq}{}.pgm", std::env::var("X264").map(|c| format!("-x264-{c}")).unwrap_or_default()), pgm).unwrap();
    }
}
