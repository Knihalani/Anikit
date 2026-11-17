//! The TS and Python builders' component catalogues must list every kind the
//! renderer registers. Regenerate with:
//! `ANIKIT_VIDEO_BIN=target/release/anikit-video node packages/anikit/scripts/gen-video-components.mjs`

#[test]
fn builder_component_catalogues_are_complete() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let ts = std::fs::read_to_string(format!("{root}/packages/anikit/src/video/components.ts")).unwrap();
    let py = std::fs::read_to_string(format!("{root}/crates/anikit-py/python/anikit/_video_components.py")).unwrap();
    let missing: Vec<&str> = anikit_video::components::KINDS
        .iter()
        .map(|k| k.name)
        .filter(|n| !ts.contains(&format!("\"{n}\"")) || !py.contains(&format!("\"{n}\"")))
        .collect();
    assert!(missing.is_empty(), "builder catalogues miss {missing:?}; run scripts/gen-video-components.mjs");
}
