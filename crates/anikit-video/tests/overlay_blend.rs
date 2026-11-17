//! Regression: a mid-grey layer with an Overlay blend leaves the image unchanged
//! (grain is drawn this way).
use anikit_video::dl::{Blend, DisplayList, Op, Paint, Rgba, IDENTITY};
use anikit_video::render::Renderer;
use std::sync::Arc;

fn frame(ops: Vec<Op>) -> Vec<u8> {
    let mut r = Renderer::new(Arc::new(Default::default()));
    let dl = DisplayList { width: 64, height: 64, background: Rgba([0.75, 0.19, 0.12, 1.0]), ops };
    r.render(&dl).data_as_u8_slice().to_vec()
}

#[test]
fn mid_grey_overlay_is_identity() {
    let grey = Op::RoundRect { x: 0.0, y: 0.0, w: 64.0, h: 64.0, radius: 0.0, paint: Paint::Solid(Rgba([0.5, 0.5, 0.5, 1.0])), transform: IDENTITY };
    let plain = frame(vec![]);
    let over = frame(vec![Op::PushLayer { opacity: 1.0, blend: Blend::Overlay, filters: vec![], clip: None, cache_key: None }, grey, Op::PopLayer]);
    let px = |d: &[u8]| [d[0], d[1], d[2]];
    let (a, b) = (px(&plain), px(&over));
    for c in 0..3 {
        assert!((a[c] as i32 - b[c] as i32).abs() <= 2, "overlay changed {a:?} -> {b:?}");
    }
}
