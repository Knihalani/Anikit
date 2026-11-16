//! Expand a built-in template with its example params and report spec diagnostics:
//! `cargo run -p anikit-video --example expand -- launch 9:16 [--print]`
use anikit_video::eval::{Evaluator, Options};
use anikit_video::templates::{brand::Brand, Format, Template, BUILTIN};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let names: Vec<&str> = if a.len() > 1 && a[1] != "all" { vec![a[1].as_str()] } else { BUILTIN.iter().map(|(n, _)| *n).collect() };
    for n in names {
        let t = Template::builtin(n).unwrap();
        for f in a.get(2).map(|s| vec![s.as_str()]).unwrap_or(vec!["16:9", "9:16", "1:1"]) {
            let spec = t.expand(&t.example_params(), &Brand::default(), Format::parse(f).unwrap()).unwrap();
            if a.iter().any(|x| x == "--print") {
                println!("{}", serde_json::to_string_pretty(&spec).unwrap());
            }
            match anikit_video::spec::parse::from_value(spec) {
                Err(d) => d.iter().for_each(|d| println!("{n} {f} PARSE {d}")),
                Ok(p) => {
                    p.warnings.iter().for_each(|d| println!("{n} {f} {d}"));
                    let (_, d) = Evaluator::build(&p.spec, &Options { load_assets: false, ..Options::default() });
                    d.iter().for_each(|d| println!("{n} {f} {d}"));
                }
            }
        }
    }
}
