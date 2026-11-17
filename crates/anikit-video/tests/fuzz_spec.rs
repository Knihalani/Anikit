//! Mutation fuzzing: random edits to valid specs must never panic in parse,
//! compile or eval (agents send all sorts of things).
use anikit_video::eval::{Evaluator, Options};
use serde_json::{json, Value};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn paths(v: &Value, cur: &mut Vec<Value>, out: &mut Vec<Vec<Value>>) {
    out.push(cur.clone());
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                cur.push(json!(k));
                paths(x, cur, out);
                cur.pop();
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                cur.push(json!(i));
                paths(x, cur, out);
                cur.pop();
            }
        }
        _ => {}
    }
}

const NASTY: &[&str] = &[
    "", "=", "=1/0", "=sin(", "@", "@nope", "-1e30", "1e30", "NaN", "50%", "-50vw", "#zz", "$", "auto", "0s", "-3s", "inOutBack", "spring",
    "linear-gradient(", "radial-gradient(#fff)", "{value", "**", "[x]{", "chars", "fadeUp", "scramble by char", "+", "*", "0ms", "12f",
];

fn random_value(r: &mut Rng) -> Value {
    match r.below(9) {
        0 => Value::Null,
        1 => json!(r.below(2) == 0),
        2 => json!((r.next() % 4000) as f64 - 2000.0),
        3 => json!([1e9, -1e9, 0.0, 1e-9, 100000.0][r.below(5)]),
        4 => json!(NASTY[r.below(NASTY.len())]),
        5 => json!([]),
        6 => json!({}),
        7 => json!([NASTY[r.below(NASTY.len())], 3]),
        _ => json!({ "text": "x", "type": NASTY[r.below(NASTY.len())] }),
    }
}

#[test]
fn mutations_never_panic() {
    let seeds = [
        include_str!("../../../docs/video/examples/showcase.json"),
        r#"{ "size": [64, 36], "scenes": [ "Title | sub", { "layers": [ "x", { "shape": "star", "animate": "rise", "loop": "spin" },
            { "type": "box", "style": { "gap": 4 }, "children": [ { "text": "{value}", "value": 5, "enter": "count" } ] },
            { "component": "bar-chart", "props": { "values": [1, 2] } } ] } ],
            "captions": { "cues": [ { "text": "", "at": 0, "end": 1, "words": [["a", 0, 0.5]] } ] } }"#,
    ];
    let opts = Options { load_assets: false, ..Default::default() };
    let mut r = Rng(0x1234_5678_9ABC_DEF1);
    let mut n = 0;
    for src in seeds {
        let base: Value = serde_json::from_str(src).unwrap();
        let mut all = Vec::new();
        paths(&base, &mut Vec::new(), &mut all);
        for _ in 0..4000 {
            let mut v = base.clone();
            for _ in 0..1 + r.below(3) {
                let p = all[r.below(all.len())].clone();
                let ptr: String = p.iter().map(|s| match s { Value::String(k) => format!("/{}", k.replace('~', "~0").replace('/', "~1")), x => format!("/{x}") }).collect();
                if let Some(slot) = v.pointer_mut(&ptr) {
                    *slot = random_value(&mut r);
                }
            }
            let text = v.to_string();
            let res = std::panic::catch_unwind(|| {
                if let Ok(p) = anikit_video::spec::parse_json(&text) {
                    let (ev, _) = Evaluator::build(&p.spec, &opts);
                    if let Some(ev) = ev {
                        for k in 0..6 {
                            let _ = ev.eval(ev.duration * k as f32 / 5.0);
                        }
                        let _ = ev.is_static(0.0, ev.duration);
                    }
                }
            });
            assert!(res.is_ok(), "panic on spec:\n{text}");
            n += 1;
        }
    }
    eprintln!("{n} mutated specs ok");
}

#[test]
fn mutated_specs_render_without_panicking() {
    use anikit_video::templates::{brand::Brand, Format, Template};
    let showcase: Value = serde_json::from_str(include_str!("../../../docs/video/examples/showcase.json")).unwrap();
    let t = Template::builtin("launch").unwrap();
    let launch = t.expand(&t.example_params(), &Brand::default(), Format::parse("16:9").unwrap()).unwrap();
    let bases = [showcase, launch];
    let alls: Vec<Vec<Vec<Value>>> = bases
        .iter()
        .map(|b| {
            let mut all = Vec::new();
            paths(b, &mut Vec::new(), &mut all);
            all
        })
        .collect();
    let opts = Options { load_assets: false, ..Default::default() };
    let mut r = Rng(0xDEAD_BEEF_1234_5678);
    let mut rendered = 0;
    for it in 0..1500 {
        let (base, all) = (&bases[it % 2], &alls[it % 2]);
        let mut v = base.clone();
        for _ in 0..1 + r.below(3) {
            let p = all[r.below(all.len())].clone();
            let ptr: String = p.iter().map(|s| match s { Value::String(k) => format!("/{}", k.replace('~', "~0").replace('/', "~1")), x => format!("/{x}") }).collect();
            if let Some(slot) = v.pointer_mut(&ptr) {
                *slot = random_value(&mut r);
            }
        }
        let text = v.to_string();
        let res = std::panic::catch_unwind(|| {
            if let Ok(p) = anikit_video::spec::parse_json(&text) {
                if let (Some(ev), _) = Evaluator::build(&p.spec, &opts) {
                    // Small frames keep this fast; scale exercises transforms too.
                    for k in 0..3 {
                        let t = ev.duration * k as f32 / 2.0;
                        let _ = anikit_video::pipeline::render_rgba(&ev, t, 0.08);
                    }
                    return true;
                }
            }
            false
        });
        match res {
            Ok(true) => rendered += 1,
            Ok(false) => {}
            Err(_) => panic!("render panic on spec:\n{text}"),
        }
    }
    eprintln!("{rendered} mutated specs rendered");
    assert!(rendered > 100, "too few mutated specs reached the renderer: {rendered}");
}
