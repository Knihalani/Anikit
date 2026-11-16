//! Expand a built-in template with its example params: `cargo run -p anikit-video --example expand -- launch 9:16`
use anikit_video::templates::{brand::Brand, Format, Template};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = Template::builtin(&a[1]).unwrap();
    let spec = t.expand(&t.example_params(), &Brand::default(), Format::parse(a.get(2).map(|s| s.as_str()).unwrap_or("16:9")).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&spec).unwrap());
}
