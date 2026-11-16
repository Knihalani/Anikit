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
    EncoderConfig { width: w, height: h, deblock: false, ..Default::default() }
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
