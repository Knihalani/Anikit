//! Stereo correlation and level per stem: `stems_corr '<json>'`.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let spec = anikit_music::MusicSpec::from_json(&a[0]).unwrap();
    let r = anikit_music::render(&spec, &anikit_music::RenderOpts { stems: true, ..Default::default() }).unwrap();
    for s in &r.stems {
        let (mut ll, mut rr, mut lr) = (0.0f64, 0.0f64, 0.0f64);
        for (x, y) in s.l.iter().zip(&s.r) {
            ll += (*x as f64).powi(2);
            rr += (*y as f64).powi(2);
            lr += *x as f64 * *y as f64;
        }
        let rms = ((ll + rr) / (2.0 * s.l.len() as f64)).sqrt();
        println!("{:<6} corr {:.2}  rms {:.1} dBFS", s.name, lr / (ll * rr).sqrt().max(1e-12), 20.0 * rms.max(1e-9).log10());
    }
    println!("master corr {:.2}", r.report["correlation"].as_f64().unwrap());
}
