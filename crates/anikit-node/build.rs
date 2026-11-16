//! Generates one N-API wrapper per `ak_*` export of `anikit-wasm`, from its
//! source, so the native addon always matches the WebAssembly module's ABI.
//! Parameters are `u32` or `f64`; returns are scalars or pointers (turned into
//! offsets into the shared arena, like addresses in wasm linear memory).
use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let src_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../anikit-wasm/src/lib.rs");
    println!("cargo:rerun-if-changed={}", src_path.display());
    let src = std::fs::read_to_string(&src_path).expect("read anikit-wasm/src/lib.rs");
    let mut out = String::new();
    let mut names = Vec::new();
    let mut rest = src.as_str();
    while let Some(at) = rest.find("extern \"C\" fn ak_") {
        let sig_start = at + "extern \"C\" fn ".len();
        let open = rest[sig_start..].find('(').unwrap() + sig_start;
        let name = rest[sig_start..open].trim().to_string();
        let close = rest[open..].find(')').unwrap() + open;
        let params: Vec<(String, String)> = rest[open + 1..close]
            .split(',')
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(|p| {
                let (n, t) = p.split_once(':').unwrap();
                (n.trim().to_string(), t.trim().to_string())
            })
            .collect();
        let brace = rest[close..].find('{').unwrap() + close;
        let ret = rest[close + 1..brace].trim().trim_start_matches("->").trim().to_string();
        rest = &rest[brace..];
        let n = params.len();
        let mut call = String::new();
        for (i, (_, t)) in params.iter().enumerate() {
            let conv = match t.as_str() {
                "u32" => format!("to_u32(arg(env, argv[{i}]))"),
                "f64" => format!("arg(env, argv[{i}])"),
                other => panic!("{name}: unsupported parameter type {other}"),
            };
            if i > 0 {
                call.push_str(", ");
            }
            call.push_str(&conv);
        }
        let ret_expr = match ret.as_str() {
            "" | "()" => "{ let () = r; undefined(env) }".to_string(),
            "f64" => "ret_f64(env, r)".to_string(),
            "u32" => "ret_i32(env, r as i32)".to_string(),
            "i32" => "ret_i32(env, r)".to_string(),
            "usize" => "ret_u32(env, r as u32)".to_string(), // counts, or offsets (`host_addr`)
            t if t.starts_with('*') => "ret_u32(env, offset(r))".to_string(),
            other => panic!("{name}: unsupported return type {other}"),
        };
        let body = format!("anikit_wasm::{name}({call})");
        writeln!(
            out,
            "unsafe extern \"C\" fn w_{name}(env: Env, info: CbInfo) -> Value {{\n    let mut argc: usize = {n};\n    let mut argv: [Value; {cap}] = [core::ptr::null_mut(); {cap}];\n    napi_get_cb_info(env, info, &mut argc, argv.as_mut_ptr(), core::ptr::null_mut(), core::ptr::null_mut());\n    let _ = &argv;\n    #[allow(unused_unsafe, clippy::let_unit_value)]\n    let r = unsafe {{ {body} }};\n    {ret_expr}\n}}",
            cap = n.max(1)
        )
        .unwrap();
        names.push(name);
    }
    out.push_str("pub(crate) const EXPORTS: &[(&str, Callback)] = &[\n");
    for n in &names {
        writeln!(out, "    (\"{n}\\0\", w_{n}),").unwrap();
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("wrappers.rs");
    std::fs::write(dest, out).unwrap();
}
