//! Compile time per built-in: parse + validate vs JIT. `cargo run --release -p anikit-shader --example compile_time`
fn main() {
    for e in anikit_shader::library::ALL {
        let t0 = std::time::Instant::now();
        let p = anikit_shader::front::parse(e.src, &[]).unwrap();
        let front = t0.elapsed().as_secs_f64() * 1e3;
        drop(p);
        let t0 = std::time::Instant::now();
        let s = anikit_shader::Shader::compile(e.src).unwrap();
        let all = t0.elapsed().as_secs_f64() * 1e3;
        println!("{:<14} front {front:5.1} ms  total {all:5.1} ms  ({} insts)", e.name, s.insts);
    }
}
