fn main() {
    let src = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    match anikit_shader::Shader::compile(&src) {
        Ok(s) => println!("ok: {:.1} ms, {} insts", s.compile_ms, s.insts),
        Err(e) => println!("{e}"),
    }
}
