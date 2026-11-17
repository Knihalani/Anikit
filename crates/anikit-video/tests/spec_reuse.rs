//! Spec reuse expands to exactly the long-hand spec.
use anikit_video::spec::parse_json;

fn read(p: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)).unwrap()
}

#[test]
fn long_reuse_equals_bench_long() {
    let a = parse_json(&read("bench/video/anikit/long.json")).unwrap();
    let b = parse_json(&read("docs/video/examples/long-reuse.json")).unwrap();
    assert!(b.warnings.is_empty(), "{:?}", b.warnings);
    assert_eq!(serde_json::to_value(&a.spec).unwrap(), serde_json::to_value(&b.spec).unwrap());
    assert_eq!(b.spec.scenes.len(), 12);
}

#[test]
fn defs_use_with_through_the_parser() {
    let src = r##"{
      "defs": { "stat": { "type": "box", "width": 360, "children": [ { "text": "$label", "style": "label" }, { "text": "$value", "style": "title" } ] } },
      "scenes": [ { "layers": [ { "use": "stat", "repeat": 3, "x": "=200 + $i * 420", "with": { "label": "Metric $i", "value": "=10 * ($i + 1)" } } ] } ]
    }"##;
    let p = parse_json(src).unwrap();
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    let layers = &p.spec.scenes[0].layers;
    assert_eq!(layers.len(), 3);
    let v = serde_json::to_value(&layers[2]).unwrap();
    assert_eq!(v["children"][0]["text"], "Metric 2");
    assert_eq!(v["children"][1]["text"], "30");
    let e = parse_json(r#"{ "defs": { "stat": {} }, "scenes": [ { "layers": [ { "use": "stats" } ] } ] }"#).unwrap_err();
    assert_eq!(e[0].to_string(), "error: scenes[0].layers[0].use: unknown def 'stats' (defs: stat), did you mean 'stat'?");
}
