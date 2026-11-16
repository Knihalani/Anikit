#![allow(clippy::needless_range_loop)]
mod common;
use ak264::synth::Scene;
use ak264::*;
use common::*;

/// Encode frames one by one; check ffmpeg decodes bit-exactly to our reconstruction.
fn roundtrip(cfg: EncoderConfig, frames: &[YuvBuffer], name: &str) -> (Vec<u8>, f64) {
    let mut e = Encoder::new(cfg.clone());
    let mut stream = Vec::new();
    let mut recon = Vec::new();
    let mut src = Vec::new();
    for f in frames {
        stream.extend_from_slice(e.encode(&f.frame(), &FrameHints::default()));
        recon.extend_from_slice(&e.reconstruction().to_bytes());
        src.extend_from_slice(&f.to_bytes());
    }
    if !have_ffmpeg() {
        eprintln!("ffmpeg missing: skipping decode check");
        return (stream, 0.0);
    }
    let (err, dec) = decode(&stream, name);
    assert!(err.is_empty(), "ffmpeg errors for {name}: {err}");
    assert_eq!(dec.len(), recon.len(), "{name}: decoded size");
    if dec != recon {
        let fsz = frames[0].to_bytes().len();
        let i = dec.iter().zip(&recon).position(|(a, b)| a != b).unwrap();
        panic!("{name}: decode mismatch at frame {} offset {}", i / fsz, i % fsz);
    }
    let p = psnr(&dec, &src);
    (stream, p)
}

fn cfg(w: u32, h: u32) -> EncoderConfig {
    let deblock = std::env::var("AK264_DEBLOCK").map(|v| v != "0").unwrap_or(true);
    let subpel = std::env::var("AK264_SUBPEL").map(|v| v != "0").unwrap_or(true);
    let cabac = std::env::var("AK264_CABAC").map(|v| v != "0").unwrap_or(true);
    EncoderConfig { width: w, height: h, deblock, cabac, subpel, ..Default::default() }
}

#[test]
fn single_idr() {
    let s = Scene::new(64, 48);
    let (_, p) = roundtrip(cfg(64, 48), &[s.frame(0)], "idr");
    assert!(p > 38.0, "psnr {p}");
}

#[test]
fn moving_cards() {
    let s = Scene::new(176, 144);
    let frames: Vec<_> = (0..12).map(|t| s.frame(t)).collect();
    let (st, p) = roundtrip(cfg(176, 144), &frames, "cards");
    eprintln!("cards: {} bytes, psnr {p:.2}", st.len());
    assert!(p > 40.0, "psnr {p}");
}

#[test]
fn static_frames_are_tiny_and_exact() {
    let s = Scene::new(320, 240);
    let f0 = s.frame(0);
    let frames: Vec<_> = (0..10).map(|_| f0.clone()).collect();
    let mut e = Encoder::new(cfg(320, 240));
    let first = e.encode(&f0.frame(), &FrameHints::default()).len();
    let p = e.encode(&f0.frame(), &FrameHints::default()).len();
    assert!(p < 16, "static P frame is {p} bytes");
    assert!(first > 100);
    let (st, psnr) = roundtrip(cfg(320, 240), &frames, "static");
    eprintln!("static: {} bytes psnr {psnr:.2}", st.len());
}

#[test]
fn full_frame_change() {
    let mut s = Scene::new(320, 192);
    s.moving_bg = true;
    s.cards = 5;
    let frames: Vec<_> = (0..8).map(|t| s.frame(t)).collect();
    let (st, p) = roundtrip(cfg(320, 192), &frames, "fullchange");
    eprintln!("fullchange: {} bytes psnr {p:.2}", st.len());
    assert!(p > 40.0, "psnr {p}");
}

#[test]
fn odd_sizes() {
    for (w, h) in [(18u32, 18u32), (100, 58), (202, 34), (16, 16), (2, 2)] {
        let s = Scene::new(w, h);
        let frames: Vec<_> = (0..4).map(|t| s.frame(t)).collect();
        let (_, p) = roundtrip(cfg(w, h), &frames, &format!("odd{w}x{h}"));
        assert!(p > if w * h < 64 { 35.0 } else { 38.0 }, "{w}x{h}: psnr {p}");
    }
}

#[test]
fn qp_range() {
    let s = Scene::new(96, 64);
    let frames: Vec<_> = (0..3).map(|t| s.frame(t)).collect();
    for qp in [0u8, 10, 30, 40, 51] {
        let c = EncoderConfig { qp, ..cfg(96, 64) };
        roundtrip(c, &frames, &format!("qp{qp}"));
    }
}

#[test]
fn slices_in_parallel() {
    let s = Scene::new(256, 160);
    let frames: Vec<_> = (0..6).map(|t| s.frame(t)).collect();
    let c = EncoderConfig { threads: 4, ..cfg(256, 160) };
    let (_, p) = roundtrip(c, &frames, "slices");
    assert!(p > 40.0);
}

#[test]
fn chunks_concatenate() {
    let s = Scene::new(160, 96);
    let bufs: Vec<_> = (0..23).map(|t| s.frame(t)).collect();
    let frames: Vec<_> = bufs.iter().map(|b| b.frame()).collect();
    let c = EncoderConfig { gop: 5, ..cfg(160, 96) };
    let stream = encode_chunks(&c, &frames, &[]);
    // Sequential encoding with the same GOP must give the same reconstruction.
    let mut e = Encoder::new(c.clone());
    let mut recon = Vec::new();
    let mut seq = Vec::new();
    for f in &frames {
        seq.extend_from_slice(e.encode(f, &FrameHints::default()));
        recon.extend_from_slice(&e.reconstruction().to_bytes());
    }
    if !have_ffmpeg() {
        return;
    }
    let (err, dec) = decode(&stream, "chunks");
    assert!(err.is_empty(), "{err}");
    assert_eq!(dec.len(), recon.len());
    assert!(dec == recon, "chunked decode differs from sequential recon");
}

#[test]
fn hints_change_map_and_motion() {
    let (w, h) = (320u32, 192u32);
    let s = Scene::new(w, h);
    let frames: Vec<_> = (0..8).map(|t| s.frame(t)).collect();
    let (mbw, mbh) = (w.div_ceil(16) as usize, h.div_ceil(16) as usize);
    let mut e = Encoder::new(cfg(w, h));
    let mut stream = Vec::new();
    let mut recon = Vec::new();
    let mut src = Vec::new();
    for t in 0..frames.len() {
        let mut hints = FrameHints::default();
        if t > 0 {
            // exact change map from the card rectangles (old and new positions)
            let mut ch = vec![false; mbw * mbh];
            let mut motions = vec![];
            for (i, (a, b)) in s.cards_at(t as u32 - 1).iter().zip(s.cards_at(t as u32)).enumerate() {
                for r in [a, &b] {
                    let x0 = (r.0.max(0) as usize) / 16;
                    let y0 = (r.1.max(0) as usize) / 16;
                    let x1 = (((r.0 + r.2).clamp(0, w as i32) as usize).div_ceil(16)).min(mbw);
                    let y1 = (((r.1 + r.3).clamp(0, h as i32) as usize).div_ceil(16)).min(mbh);
                    for my in y0..y1 {
                        for mx in x0..x1 {
                            ch[my * mbw + mx] = true;
                        }
                    }
                }
                let _ = i;
                motions.push(MotionHint {
                    x: b.0.max(0) as u32,
                    y: b.1.max(0) as u32,
                    w: b.2 as u32,
                    h: b.3 as u32,
                    dx: b.0 - a.0,
                    dy: b.1 - a.1,
                });
            }
            hints = FrameHints { changed: Some(ch), motions };
        }
        stream.extend_from_slice(e.encode(&frames[t].frame(), &hints));
        recon.extend_from_slice(&e.reconstruction().to_bytes());
        src.extend_from_slice(&frames[t].to_bytes());
    }
    if !have_ffmpeg() {
        return;
    }
    let (err, dec) = decode(&stream, "hints");
    assert!(err.is_empty(), "{err}");
    assert!(dec == recon);
    let p = psnr(&dec, &src);
    eprintln!("hints: {} bytes psnr {p:.2}", stream.len());
    assert!(p > 40.0);
}

#[test]
fn cavlc_and_options_matrix() {
    let mut s = Scene::new(192, 112);
    s.moving_bg = true;
    s.cards = 4;
    s.fade = 0.3;
    let frames: Vec<_> = (0..6).map(|t| s.frame(t)).collect();
    for cabac in [false, true] {
        for deblock in [false, true] {
            for subpel in [false, true] {
                let c = EncoderConfig { cabac, deblock, subpel, threads: 2, ..cfg(192, 112) };
                let (_, p) = roundtrip(c, &frames, &format!("m{}{}{}", cabac as u8, deblock as u8, subpel as u8));
                assert!(p > 40.0);
            }
        }
    }
}

#[test]
fn synthetic_scene_hints_are_exact() {
    // The scene's own change map must cover every changed macroblock.
    for name in ["typical", "hold"] {
        let s = Scene::preset(name, 320, 192);
        let frames: Vec<_> = (0..6).map(|t| s.frame(t)).collect();
        let hints: Vec<FrameHints> = (0..6).map(|t| if t == 0 { FrameHints::default() } else { s.hints(t) }).collect();
        let c = cfg(320, 192);
        let refs: Vec<YuvFrame> = frames.iter().map(|f| f.frame()).collect();
        let a = encode_chunks(&c, &refs, &[]);
        let b = encode_chunks(&c, &refs, &hints);
        if !have_ffmpeg() {
            return;
        }
        let (_, da) = decode(&a, "ha");
        let (_, db) = decode(&b, "hb");
        assert!(psnr(&da, &db) > 45.0, "{name}");
    }
}

#[test]
fn long_gop_frame_num_wraps() {
    let s = Scene::new(48, 32);
    let frames: Vec<_> = (0..300).map(|t| s.frame(t / 3)).collect();
    let c = EncoderConfig { gop: 1000, ..cfg(48, 32) };
    let (_, p) = roundtrip(c, &frames, "wrap");
    assert!(p > 40.0);
}
