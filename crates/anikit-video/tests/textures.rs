use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline::textures::CompTextures;
use anikit_video::spec::parse_json;
use rayon::prelude::*;

const SRC: &str = r##"{ "fps": 30, "compositions": { "film": { "size": [64, 36], "duration": 2, "background": "#203040",
    "layers": [ { "shape": "circle", "width": 10, "height": 10, "fill": "#ffcc00",
                  "animate": [ { "from": { "x": 0 }, "to": { "x": 64 }, "duration": 1 } ] } ] } },
  "scenes": [ "x" ] }"##;

#[test]
fn deterministic_shared_capped_and_parallel() {
    let p = parse_json(SRC).unwrap();
    let ev = Evaluator::new(&p.spec, Options::default()).unwrap();
    let comp = ev.composition("film").unwrap();
    let cache = CompTextures::new(usize::MAX);
    // 3,700 instances, instance i shows frame i (the second half of the film is static).
    let px: Vec<_> = (0..3700u32).into_par_iter().map(|i| cache.get(comp, (i % 60) as f32 / 30.0, (i % 2) as u8)).collect();
    let s = cache.stats();
    assert!((30..=34).contains(&s.renders), "{s:?}"); // 31 moving frames (lod = frame parity) + the held end, not 3,700
    assert_eq!(s.textures as u64, s.renders);
    // Same request → same pixels, in a fresh cache and in any order.
    let fresh = CompTextures::new(usize::MAX);
    for i in [3699u32, 17, 42, 1] {
        let a = &px[i as usize];
        let b = fresh.get(comp, (i % 60) as f32 / 30.0, (i % 2) as u8);
        assert_eq!(a.data_as_u8_slice(), b.data_as_u8_slice());
        assert_eq!((a.width() as u32, a.height() as u32), CompTextures::size(comp, (i % 2) as u8));
    }
    // Times inside one frame snap to it.
    assert!(std::sync::Arc::ptr_eq(&fresh.get(comp, 0.5, 0), &fresh.get(comp, 0.5 + 0.02, 0)));
    // A cap keeps memory bounded.
    let small = CompTextures::new(64 * 36 * 4 * 5);
    for i in 0..60 {
        small.get(comp, i as f32 / 30.0, 0);
    }
    assert!(small.stats().bytes <= 64 * 36 * 4 * 5, "{:?}", small.stats());
}

#[test]
fn components_reach_compositions_through_assets() {
    use anikit_video::components::Assets;
    let p = parse_json(SRC).unwrap();
    let ev = Evaluator::new(&p.spec, Options::default()).unwrap();
    let info = ev.assets.composition_info("film").unwrap();
    assert_eq!((info.width, info.height, info.duration, info.looped), (64, 36, 2.0, false));
    let a = ev.assets.composition_frame("film", 0.5, 1).unwrap();
    let b = CompTextures::new(usize::MAX).get(ev.composition("film").unwrap(), 0.5, 1);
    assert_eq!(a.data_as_u8_slice(), b.data_as_u8_slice());
    assert!(ev.assets.composition_info("nope").is_none());
}

/// A composition made of a shader layer, used as a texture on a 3D pane, must
/// show its pixels. Regression: the texture was first requested during the
/// frame's scale-collecting pass, where shader layers emit nothing, and that
/// empty list was cached, so shader textures on 3D surfaces stayed blank.
#[test]
fn shader_composition_textures_a_3d_surface() {
    const BIN: &str = env!("CARGO_BIN_EXE_anikit-video");
    let dir = std::env::temp_dir().join(format!("anikit-video-shadertex-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let spec = r##"{ "size": [320, 180], "compositions": { "sh": { "size": [64, 36], "duration": 1, "layers": [
        { "shader": "void mainImage(out vec4 o, in vec2 f){ o = vec4(1.0, 0.0, 0.0, 1.0); }" } ] } },
      "scenes": [ { "duration": 1, "layers": [ { "component": "scene3d", "props": { "env": "night", "floor": false,
        "objects": [ { "type": "pane", "size": [3, 1.7], "pos": [0, 0, 0], "texture": "#sh", "material": { "base": "#ffffff", "glow": 1 } } ],
        "camera": { "keys": [ { "t": 0, "pos": [0, 0, 3], "target": [0, 0, 0], "focal": 35 } ] } } } ] } ] }"##;
    std::fs::write(dir.join("s.json"), spec).unwrap();
    let out = dir.join("f.png");
    let st = std::process::Command::new(BIN).args(["still", "s.json", "--at", "0.5", "-o", "f.png"]).current_dir(&dir).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let img = image_rgba(&out);
    let (w, h, px) = img;
    let p = &px[((h / 2) * w + w / 2) * 4..][..4];
    assert!(p[0] > 150 && p[1] < 80 && p[2] < 80, "centre of the textured pane should be the shader's red, got {p:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn image_rgba(path: &std::path::Path) -> (usize, usize, Vec<u8>) {
    let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut r = dec.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    let ch = match info.color_type { png::ColorType::Rgba => 4, png::ColorType::Rgb => 3, _ => panic!("unexpected colour type") };
    let rgba: Vec<u8> = if ch == 4 { buf[..info.buffer_size()].to_vec() } else { buf[..info.buffer_size()].chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect() };
    (info.width as usize, info.height as usize, rgba)
}
