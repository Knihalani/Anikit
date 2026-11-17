use anikit_video::eval::{Evaluator, Options};
use anikit_video::render::post::PostSettings;
use anikit_video::render::Renderer;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = anikit_video::spec::parse_str(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let ev = Evaluator::new(&p.spec, Options::default()).unwrap();
    let l = ev.eval(9.0);
    let mut r = Renderer::new(ev.assets.snapshot());
    r.set_post(PostSettings::preset(&a[2]), 1);
    r.render(&l);
    let t = std::time::Instant::now();
    for _ in 0..4 {
        r.render(&l);
    }
    println!("{} {:.1} ms/frame", a[2], t.elapsed().as_secs_f64() * 250.0);
}
