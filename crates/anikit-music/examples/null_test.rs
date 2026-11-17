//! Null test between two directories of raw f32 renders (from `ANIKIT_DEMOS_RAW=1 … --example demos`):
//! prints the peak and RMS of the difference per file in dBFS.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (x, y) = (std::path::Path::new(&a[0]), std::path::Path::new(&a[1]));
    let mut worst = f64::NEG_INFINITY;
    let mut names: Vec<_> = std::fs::read_dir(x).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).filter(|n| n.to_string_lossy().ends_with(".f32")).collect();
    names.sort();
    for n in names {
        let rd = |p: &std::path::Path| -> Vec<f32> { std::fs::read(p.join(&n)).unwrap_or_default().chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect() };
        let (u, v) = (rd(x), rd(y));
        if u.len() != v.len() {
            println!("{:?}: LENGTH {} vs {}", n, u.len(), v.len());
            worst = 0.0;
            continue;
        }
        let (mut pk, mut ss) = (0.0f64, 0.0f64);
        for (p, q) in u.iter().zip(&v) {
            let d = (*p - *q) as f64;
            pk = pk.max(d.abs());
            ss += d * d;
        }
        let db = |v: f64| if v > 0.0 { 20.0 * v.log10() } else { f64::NEG_INFINITY };
        let rms = (ss / u.len().max(1) as f64).sqrt();
        println!("{:<16} peak diff {:>8.1} dBFS  rms diff {:>8.1} dBFS", n.to_string_lossy(), db(pk), db(rms));
        worst = worst.max(db(pk));
    }
    println!("worst peak difference: {worst:.1} dBFS");
}
