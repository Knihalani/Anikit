//! Convert a SoundFont 2 bank to SF3 (Ogg Vorbis samples, MuseScore convention).
//!
//! ```text
//! cargo run --release -p anikit-music --example build_sf3 -- in.sf2 out.sf3 [--quality Q] [--measure]
//! ```
//!
//! Every sample is encoded on its own (stereo pairs stay linked mono samples)
//! with the system `ffmpeg -c:a libvorbis -q:a Q` (a build-time tool only; the
//! runtime decoder is pure Rust). The sample headers are rewritten as SF3
//! expects: `sampleType |= 0x10`, `start`/`end` are byte offsets of each Ogg
//! stream in `smpl`, loop points are frames relative to the sample's start.
//! `sm24` is dropped and `ifil` becomes 3.0; everything else is copied as is.
//!
//! By default each sample is stored as a *compact Vorbis record* (see
//! `anikit_music::sf2::sampler::decode_compact_vorbis`): the same Vorbis
//! packets without Ogg framing or comment header, with identical setup
//! headers (the ~3.5 KB codebooks) stored once. For GeneralUser GS that saves
//! 3.5 MB of 9 MB. `--standard` writes plain Ogg streams instead (MuseScore
//! SF3, readable by FluidSynth/Polyphone).
//!
//! `--measure` decodes every stream again and prints SNR and log-spectral
//! distance against the original PCM (and checks the compact records decode
//! bit-identically to the Ogg streams).

use anikit_music::sf2::parse::{chunks, lists, Sf2};
use anikit_music::sf2::sampler::{decode_audio, decode_compact_vorbis, write_varint, COMPACT_MAGIC};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn wav_bytes(pcm: &[i16], rate: u32) -> Vec<u8> {
    let mut w = Vec::with_capacity(44 + pcm.len() * 2);
    let data = (pcm.len() * 2) as u32;
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data.to_le_bytes());
    for s in pcm {
        w.extend_from_slice(&s.to_le_bytes());
    }
    w
}

fn encode(pcm: &[i16], rate: u32, q: f32, tmp: &Path, i: usize) -> Vec<u8> {
    let wav = tmp.join(format!("{i}.wav"));
    let ogg = tmp.join(format!("{i}.ogg"));
    std::fs::write(&wav, wav_bytes(pcm, rate)).expect("write wav");
    let st = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(&wav)
        .args(["-map_metadata", "-1", "-fflags", "+bitexact", "-flags:a", "+bitexact", "-c:a", "libvorbis", "-q:a"])
        .arg(format!("{q}"))
        .arg(&ogg)
        .status()
        .expect("run ffmpeg (is it installed?)");
    assert!(st.success(), "ffmpeg failed on sample {i}");
    let b = std::fs::read(&ogg).expect("read ogg");
    let _ = std::fs::remove_file(&wav);
    let _ = std::fs::remove_file(&ogg);
    b
}

/// Split an Ogg stream into its packets (single logical stream).
fn ogg_packets(b: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    let mut p = 0;
    while p + 27 <= b.len() && &b[p..p + 4] == b"OggS" {
        let nseg = b[p + 26] as usize;
        let lace = &b[p + 27..p + 27 + nseg];
        let mut d = p + 27 + nseg;
        for &l in lace {
            cur.extend_from_slice(&b[d..d + l as usize]);
            d += l as usize;
            if l < 255 {
                out.push(std::mem::take(&mut cur));
            }
        }
        p = d;
    }
    assert!(p == b.len(), "trailing bytes in Ogg stream");
    out
}

/// Signal and error energies and a log-spectral distance accumulator.
#[derive(Default, Clone, Copy)]
struct Stats {
    sig: f64,
    err: f64,
    lsd_sum: f64,
    lsd_n: f64,
}

fn compare(a: &[f32], b: &[f32]) -> Stats {
    let n = a.len().min(b.len());
    let mut s = Stats::default();
    for i in 0..n {
        s.sig += (a[i] as f64).powi(2);
        s.err += (a[i] as f64 - b[i] as f64).powi(2);
    }
    for i in n..a.len() {
        s.err += (a[i] as f64).powi(2);
    }
    // Log-spectral distance over 1024-frame Hann windows, bins within 60 dB of the frame peak.
    use rustfft::num_complex::Complex;
    const N: usize = 1024;
    let mut planner = rustfft::FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(N);
    let win: Vec<f32> = (0..N).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos()).collect();
    let mut off = 0;
    while off + N <= n {
        let mut xa: Vec<Complex<f32>> = (0..N).map(|i| Complex::new(a[off + i] * win[i], 0.0)).collect();
        let mut xb: Vec<Complex<f32>> = (0..N).map(|i| Complex::new(b[off + i] * win[i], 0.0)).collect();
        fft.process(&mut xa);
        fft.process(&mut xb);
        let peak = xa[..N / 2].iter().map(|c| c.norm()).fold(0.0f32, f32::max);
        if peak > 1e-3 {
            for k in 1..N / 2 {
                let ma = xa[k].norm();
                if ma > peak * 1e-3 {
                    let mb = xb[k].norm().max(1e-9);
                    s.lsd_sum += (20.0 * (mb / ma).log10()).abs() as f64;
                    s.lsd_n += 1.0;
                }
            }
        }
        off += N / 2;
    }
    s
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut q = 6.0f32;
    let mut measure = false;
    let mut standard = false;
    let mut files = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--quality" | "-q" => q = it.next().expect("--quality needs a value").parse().expect("quality"),
            "--measure" => measure = true,
            "--standard" => standard = true,
            _ => files.push(a.clone()),
        }
    }
    if files.len() != 2 {
        eprintln!("usage: build_sf3 in.sf2 out.sf3 [--quality Q] [--measure] [--standard]");
        std::process::exit(2);
    }
    let input = std::fs::read(&files[0]).expect("read input");
    let sf = Sf2::parse(&input).expect("parse input");
    assert!(sf.samples.iter().all(|s| !s.is_vorbis()), "input is already SF3");
    let smpl = &input[sf.data.smpl..sf.data.smpl + sf.data.smpl_len];
    let pcm_of = |h: &anikit_music::sf2::parse::SampleHeader| -> Vec<i16> {
        let (s, e) = (h.start as usize, (h.end as usize).min(smpl.len() / 2));
        (s..e.max(s)).map(|i| i16::from_le_bytes([smpl[2 * i], smpl[2 * i + 1]])).collect()
    };

    let tmp: PathBuf = std::env::temp_dir().join(format!("anikit-sf3-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("tmp dir");
    let t0 = std::time::Instant::now();
    let streams: Vec<Vec<u8>> = sf
        .samples
        .par_iter()
        .enumerate()
        .map(|(i, h)| {
            let pcm = pcm_of(h);
            if pcm.is_empty() {
                Vec::new()
            } else {
                encode(&pcm, h.rate, q, &tmp, i)
            }
        })
        .collect();
    let _ = std::fs::remove_dir_all(&tmp);
    eprintln!("encoded {} samples at q{q} in {:.1?}", streams.len(), t0.elapsed());

    // Compact records: factor out the Ogg framing, drop the comment header and
    // share identical setup headers (they hold the codebooks, ~3.5 KB each).
    let mut smpl_out = Vec::new();
    let records: Vec<Vec<u8>> = if standard {
        streams.clone()
    } else {
        let mut setups: Vec<(Vec<u8>, usize)> = Vec::new();
        let mut recs = Vec::new();
        for (s, h) in streams.iter().zip(sf.samples.iter()) {
            if s.is_empty() {
                recs.push(Vec::new());
                continue;
            }
            let pk = ogg_packets(s);
            assert!(pk.len() >= 3 && pk[0][0] == 1 && pk[1][0] == 3 && pk[2][0] == 5, "unexpected Vorbis headers");
            let so = match setups.iter().find(|(d, _)| *d == pk[2]) {
                Some((_, o)) => *o,
                None => {
                    let o = smpl_out.len();
                    smpl_out.extend_from_slice(&pk[2]);
                    setups.push((pk[2].clone(), o));
                    o
                }
            };
            let mut r = COMPACT_MAGIC.to_vec();
            write_varint(&mut r, pcm_of(h).len() as u64);
            write_varint(&mut r, pk[0].len() as u64);
            r.extend_from_slice(&pk[0]);
            write_varint(&mut r, so as u64);
            write_varint(&mut r, pk[2].len() as u64);
            write_varint(&mut r, (pk.len() - 3) as u64);
            for a in &pk[3..] {
                write_varint(&mut r, a.len() as u64);
                r.extend_from_slice(a);
            }
            recs.push(r);
        }
        eprintln!("{} distinct setup headers shared by {} samples", setups.len(), recs.len());
        recs
    };
    let mut offsets = Vec::with_capacity(records.len());
    for s in &records {
        let start = smpl_out.len() as u32;
        smpl_out.extend_from_slice(s);
        offsets.push((start, smpl_out.len() as u32));
    }

    if measure {
        let t1 = std::time::Instant::now();
        let per: Vec<(Stats, usize, bool)> = sf
            .samples
            .par_iter()
            .zip(streams.par_iter().zip(offsets.par_iter()))
            .map(|(h, (ogg, &(s, e)))| {
                let pcm: Vec<f32> = pcm_of(h).iter().map(|&s| s as f32 / 32768.0).collect();
                if ogg.is_empty() {
                    return (Stats::default(), 0, true);
                }
                let (ch, _) = decode_audio(Box::new(std::io::Cursor::new(ogg.clone())), Some("ogg")).expect("decode");
                let mut same = ch[0].len() == pcm.len();
                if !standard {
                    // The compact path must decode to exactly what the Ogg path gives.
                    let c = decode_compact_vorbis(&smpl_out, &smpl_out[s as usize..e as usize]).expect("compact");
                    same &= c == ch[0];
                }
                (compare(&pcm, &ch[0]), pcm.len(), same)
            })
            .collect();
        let (mut sig, mut err, mut lsd, mut lsdn) = (0.0, 0.0, 0.0, 0.0);
        let mut snrs: Vec<f64> = Vec::new();
        let mut mismatched = 0;
        for (s, _n, same) in &per {
            sig += s.sig;
            err += s.err;
            lsd += s.lsd_sum;
            lsdn += s.lsd_n;
            if s.sig > 0.0 {
                snrs.push(10.0 * (s.sig / s.err.max(1e-30)).log10());
            }
            if !same {
                mismatched += 1;
            }
        }
        snrs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pct = |p: f64| snrs[((snrs.len() - 1) as f64 * p) as usize];
        println!(
            "q{q}: overall SNR {:.1} dB, per-sample SNR median {:.1} dB, 10th pct {:.1} dB, worst {:.1} dB, \
             mean log-spectral distance {:.2} dB, length/compact mismatches {mismatched} (decode check {:.1?})",
            10.0 * (sig / err).log10(),
            pct(0.5),
            pct(0.1),
            snrs[0],
            lsd / lsdn.max(1.0),
            t1.elapsed()
        );
    }

    // Assemble the output: copy INFO (ifil → 3.0), new sdta, pdta with new shdr.
    let ls = lists(&input).expect("lists");
    let find = |k: &str| ls.iter().find(|(n, _)| n == k).map(|(_, c)| *c).expect("list");
    let info = find("INFO");
    let pdta = find("pdta");

    let mut info_out = b"INFO".to_vec();
    for c in chunks(&input, info.off, info.off + info.len).expect("INFO") {
        let mut payload = input[c.off..c.off + c.len].to_vec();
        if &c.id == b"ifil" {
            payload = [3u16.to_le_bytes(), 0u16.to_le_bytes()].concat();
        }
        if &c.id == b"ISFT" {
            payload = b"anikit build_sf3 (libvorbis)\0".to_vec();
        }
        push_chunk(&mut info_out, &c.id, &payload);
    }

    let mut sdta_out = b"sdta".to_vec();
    push_chunk(&mut sdta_out, b"smpl", &smpl_out);

    let mut pdta_out = b"pdta".to_vec();
    for c in chunks(&input, pdta.off, pdta.off + pdta.len).expect("pdta") {
        let mut payload = input[c.off..c.off + c.len].to_vec();
        if &c.id == b"shdr" {
            for (i, h) in sf.samples.iter().enumerate() {
                let r = &mut payload[i * 46..i * 46 + 46];
                let (s, e) = offsets[i];
                r[20..24].copy_from_slice(&s.to_le_bytes());
                r[24..28].copy_from_slice(&e.to_le_bytes());
                r[28..32].copy_from_slice(&h.loop_start.to_le_bytes());
                r[32..36].copy_from_slice(&h.loop_end.to_le_bytes());
                let kind = h.kind | 0x10;
                r[44..46].copy_from_slice(&kind.to_le_bytes());
            }
        }
        push_chunk(&mut pdta_out, &c.id, &payload);
    }

    let mut body = b"sfbk".to_vec();
    push_chunk(&mut body, b"LIST", &info_out);
    push_chunk(&mut body, b"LIST", &sdta_out);
    push_chunk(&mut body, b"LIST", &pdta_out);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    std::fs::write(&files[1], &out).expect("write output");
    println!(
        "wrote {} ({:.2} MB; sample data {:.2} MB from {:.2} MB of 16-bit PCM)",
        files[1],
        out.len() as f64 / 1e6,
        smpl_out.len() as f64 / 1e6,
        sf.data.smpl_len as f64 / 1e6
    );
}

fn push_chunk(out: &mut Vec<u8>, id: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() & 1 == 1 {
        out.push(0);
    }
}
