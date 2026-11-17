//! Render one music spec (JSON) to a WAV: `render_spec '<json>' out.wav`.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let spec = anikit_music::MusicSpec::from_json(&a[0]).unwrap();
    let r = anikit_music::render(&spec, &Default::default()).unwrap();
    let n = r.l.len();
    let mut b = Vec::with_capacity(44 + n * 8);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + n as u32 * 8).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&r.rate.to_le_bytes());
    b.extend_from_slice(&(r.rate * 8).to_le_bytes());
    b.extend_from_slice(&8u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(n as u32 * 8).to_le_bytes());
    for (x, y) in r.l.iter().zip(&r.r) {
        b.extend_from_slice(&x.to_le_bytes());
        b.extend_from_slice(&y.to_le_bytes());
    }
    std::fs::write(&a[1], b).unwrap();
    let rep = &r.report;
    println!("{} {:.1}s {:.2} LUFS {:.2} dBTP corr {:.2} {:.1}x", a[1], r.duration(), rep["lufs"].as_f64().unwrap(), rep["truePeak"].as_f64().unwrap(), rep["correlation"].as_f64().unwrap(), r.timings.realtime);
}
