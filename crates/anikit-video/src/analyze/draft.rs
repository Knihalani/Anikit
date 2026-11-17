//! `analyze --spec`: a draft Anikit spec with the reference's structure:
//! one scene per shot at the shot's time, the detected transitions, the
//! palette as theme tokens, camera moves, text in the detected boxes
//! (placeholder copy), music at the detected tempo, and a starting layer
//! for each shot's technique guess. A starting point to adapt, not a copy.

use super::image_ops::hex_to_rgb;
use super::{Analysis, Shot};
use serde_json::{json, Value};

fn rel_lum(h: &str) -> f32 {
    let c = hex_to_rgb(h);
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

fn contrast(a: &str, b: &str) -> f32 {
    let (x, y) = (rel_lum(a), rel_lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// The theme tokens: bg, text, primary, accent, muted.
fn theme(a: &Analysis) -> Value {
    let p = &a.global.palette;
    let by_l = |dark: bool| {
        p.iter()
            .filter(|s| s.share > 0.04)
            .min_by(|x, y| if dark { x.l.total_cmp(&y.l) } else { y.l.total_cmp(&x.l) })
            .map(|s| s.hex.clone())
            .unwrap_or_else(|| if dark { "#0B0B10".into() } else { "#F4F4F0".into() })
    };
    let dark = a.global.mean_luma < 0.55;
    let bg = by_l(dark);
    let mut text = by_l(!dark);
    if contrast(&bg, &text) < 4.5 {
        text = if dark { "#F4F4F0".into() } else { "#101014".into() };
    }
    let mut chroma: Vec<_> = p.iter().filter(|s| s.c > 0.04 && s.hex != bg).collect();
    chroma.sort_by(|x, y| (y.c * y.share.sqrt()).total_cmp(&(x.c * x.share.sqrt())));
    let primary = chroma.first().map(|s| s.hex.clone()).unwrap_or_else(|| text.clone());
    let accent = chroma.get(1).map(|s| s.hex.clone()).unwrap_or_else(|| primary.clone());
    json!({ "colors": { "bg": bg, "text": text, "primary": primary, "accent": accent } })
}

fn pct(v: f32) -> String {
    format!("{:.1}%", v * 100.0)
}

fn text_layers(s: &Shot, kinetic: bool) -> Vec<Value> {
    let copy = ["HEADLINE", "Supporting line", "caption"];
    s.text
        .iter()
        .take(3)
        .enumerate()
        .map(|(i, t)| {
            let size = (t.height * 100.0 * 0.85).clamp(1.6, 30.0);
            let (x, anchor) = match t.align.as_str() {
                "left" => (t.bbox[0], Some("left")),
                "right" => (t.bbox[0] + t.bbox[2], Some("right")),
                _ => (t.bbox[0] + t.bbox[2] / 2.0, None),
            };
            let y = t.bbox[1] + t.bbox[3] / 2.0;
            let mut l = json!({
                "text": copy[i.min(2)],
                "size": format!("{size:.1}vh"),
                "weight": if t.height > 0.06 { 800 } else { 500 },
                "color": "$text",
                "x": pct(x),
                "y": pct(y),
                "enter": if kinetic { "pop by char from center" } else { "rise" },
            });
            if let Some(a) = anchor {
                l["anchor"] = json!(a);
            }
            l
        })
        .collect()
}

fn technique_layer(tech: &str, s: &Shot, dur: f64) -> Option<Value> {
    let swatches: Vec<String> = s.palette.iter().filter(|p| p.share > 0.02).map(|p| p.hex.clone()).collect();
    match tech {
        "particles" => {
            let (_, mut spec) = crate::agent_docs::recipe_spec("particle-ignite")?;
            let mut layer = spec["scenes"][0]["layers"][0].take();
            let obj = &mut layer["props"]["objects"][0];
            obj["emit"]["text"] = json!("PRODUCT");
            if swatches.len() >= 2 {
                obj["color"] = json!(swatches);
            }
            layer["props"]["camera"]["keys"][1]["t"] = json!(dur);
            Some(layer)
        }
        "scene3d" => {
            let (_, mut spec) = crate::agent_docs::recipe_spec("chrome-hero")?;
            let mut layer = spec["scenes"][0]["layers"][0].take();
            layer["props"]["objects"][0]["text"] = json!("PRODUCT");
            layer["props"]["camera"]["keys"][1]["t"] = json!(dur);
            Some(layer)
        }
        "shader" | "footage-or-scene3d" => {
            let name = if s.luma < 0.4 { "aurora" } else { "flow-ink" };
            Some(json!({ "shader": name }))
        }
        "2d-motion-graphics" => {
            let fill = if swatches.len() > 1 { "$accent" } else { "$primary" };
            Some(json!({ "shape": "circle", "width": 360, "height": 360, "fill": fill, "opacity": 0.9, "enter": "pop" }))
        }
        _ => None,
    }
}

fn music(a: &Analysis) -> Option<Value> {
    let au = a.audio.as_ref()?;
    if au.bpm <= 0.0 {
        return None;
    }
    let bpm = au.bpm.round();
    let loud = au.loudness.iter().sum::<f32>() / au.loudness.len().max(1) as f32;
    let style = match bpm as u32 {
        0..=89 => if loud < 0.35 { "ambient" } else { "lofi" },
        90..=109 => "corporate",
        110..=132 => if a.global.pace == "calm" { "synthwave" } else { "house" },
        133..=152 => "trap",
        153..=185 => "dnb",
        _ => "cinematic-hybrid",
    };
    let hits: Vec<String> = au.hits.iter().take(4).map(|t| format!("@{t:.2}")).collect();
    // `fit: false` keeps the measured tempo (hits would otherwise nudge it by up to 8%).
    let mut m = json!({ "style": style, "bpm": bpm, "fit": false });
    if !hits.is_empty() {
        m["hits"] = json!(hits);
    }
    Some(m)
}

/// The draft spec for `a`.
pub fn draft_spec(a: &Analysis) -> Value {
    let g = &a.global;
    let size: Value = match g.aspect.as_str() {
        "16:9" | "9:16" | "1:1" | "4:5" => json!(g.aspect),
        _ => {
            let k = 1080.0 / g.width.min(g.height).max(1) as f64;
            json!([((g.width as f64 * k / 2.0).round() * 2.0) as u32, ((g.height as f64 * k / 2.0).round() * 2.0) as u32])
        }
    };
    let (w, h) = match g.aspect.as_str() {
        "9:16" => (1080.0, 1920.0),
        "1:1" => (1080.0, 1080.0),
        "4:5" => (1080.0, 1350.0),
        "16:9" => (1920.0, 1080.0),
        _ => (g.width as f32 * 1080.0 / g.width.min(g.height).max(1) as f32, g.height as f32 * 1080.0 / g.width.min(g.height).max(1) as f32),
    };
    let fps = if g.fps > 45.0 { 60 } else if g.fps < 26.0 { 24 } else { 30 };
    let tr = &a.transitions;
    let tdur = |i: usize| -> f64 {
        // Overlap of the transition into shot i (0 for cuts).
        if i == 0 || i > tr.len() {
            return 0.0;
        }
        match tr[i - 1].kind.as_str() {
            "crossfade" | "fadeBlack" | "fadeWhite" | "wipe" | "zoom" => tr[i - 1].duration.clamp(0.2, 1.2),
            "flash" => 0.2,
            _ => 0.0,
        }
    };
    let mut scenes = Vec::new();
    let mut whip_any = false;
    for (i, s) in a.shots.iter().enumerate() {
        let d = s.duration + tdur(i) / 2.0 + tdur(i + 1) / 2.0;
        let d = (d * 1000.0).round() / 1000.0;
        let tech = s.technique[0].as_str();
        let bg = s.palette.first().map(|p| p.hex.clone()).unwrap_or_else(|| "$bg".into());
        let bg2 = s.palette.get(1).map(|p| p.hex.clone()).unwrap_or_else(|| bg.clone());
        let background = if s.look == "flat-2d" { json!(bg) } else { json!(format!("radial-gradient({bg2}, {bg})")) };
        let mut scene = json!({ "id": format!("s{}-{}", i + 1, tech), "duration": d, "background": background });
        let tin = s.transition_in.as_str();
        scene["transition"] = match tin {
            "start" | "cut" => json!("cut"),
            "whip" => {
                whip_any = true;
                json!("cut")
            }
            "flash" => json!({ "type": "fadeWhite", "duration": tdur(i) }),
            "wipe" => json!({ "type": "wipe", "direction": tr[i - 1].direction.clone().unwrap_or_else(|| "left".into()), "duration": tdur(i) }),
            k => json!({ "type": k, "duration": tdur(i) }),
        };
        // Camera: the measured move over the shot.
        let c = &s.camera;
        let mut to = serde_json::Map::new();
        let mut from = serde_json::Map::new();
        if c.zoom > 1.03 || c.zoom < 0.97 {
            from.insert("zoom".into(), json!(1));
            to.insert("zoom".into(), json!(((c.zoom as f64 * 100.0).round() / 100.0).clamp(0.5, 2.5)));
        }
        if c.pan_x.abs() > 0.04 {
            from.insert("x".into(), json!(0));
            to.insert("x".into(), json!((-c.pan_x * w).round().clamp(-w, w)));
        }
        if c.pan_y.abs() > 0.04 {
            from.insert("y".into(), json!(0));
            to.insert("y".into(), json!((-c.pan_y * h).round().clamp(-h, h)));
        }
        if c.rotate.abs() > 2.0 {
            from.insert("rotation".into(), json!(0));
            to.insert("rotation".into(), json!(-c.rotate.round().clamp(-45.0, 45.0)));
        }
        let mut anims = Vec::new();
        if !to.is_empty() {
            anims.push(json!({ "from": from, "to": to, "duration": d, "ease": "inOutSine" }));
        }
        // Whips: the outgoing shot rips out, the incoming one lands.
        if tin == "whip" {
            anims.push(json!({ "from": { "x": -w }, "to": { "x": 0 }, "duration": 0.22, "ease": "outExpo" }));
        }
        if a.shots.get(i + 1).is_some_and(|n| n.transition_in == "whip") {
            anims.push(json!({ "to": { "x": w }, "at": (d - 0.2).max(0.0), "duration": 0.2, "ease": "inExpo" }));
        }
        let mut cam = json!({});
        if !anims.is_empty() {
            cam["animate"] = json!(anims);
        }
        if c.shake > 0.35 {
            cam["shake"] = json!({ "amount": (c.shake * w / 100.0 * 3.0).round().clamp(2.0, 30.0) });
        }
        if cam.as_object().is_some_and(|o| !o.is_empty()) {
            scene["camera"] = cam;
        }
        let mut layers = Vec::new();
        if let Some(l) = technique_layer(tech, s, d) {
            layers.push(l);
        }
        let kinetic = s.technique.iter().any(|t| t == "kinetic-type");
        layers.extend(text_layers(s, kinetic));
        if layers.is_empty() {
            layers.push(json!({ "shape": "rect", "width": "100%", "height": "100%", "fill": "$bg", "opacity": 0 }));
        }
        scene["layers"] = json!(layers);
        scenes.push(scene);
    }
    let mut spec = json!({
        "size": size,
        "fps": fps,
        "intent": { "pace": g.pace },
        "theme": theme(a),
        "scenes": scenes,
    });
    let mut post = serde_json::Map::new();
    if g.grain > 0.008 {
        post.insert("grain".into(), json!((((g.grain as f64 - 0.004) * 2.0).clamp(0.01, 0.06) * 100.0).round() / 100.0));
    }
    if g.vignette > 0.25 {
        post.insert("vignette".into(), json!(((g.vignette as f64).min(0.7) * 100.0).round() / 100.0));
    }
    if g.glow > 0.1 {
        post.insert("bloom".into(), json!(((g.glow as f64 * 0.8).min(0.8) * 100.0).round() / 100.0));
    }
    if !post.is_empty() {
        spec["post"] = Value::Object(post);
    }
    if whip_any {
        spec["motionBlur"] = json!({ "shutter": 270 });
    }
    if let Some(m) = music(a) {
        spec["music"] = m;
    }
    spec
}
