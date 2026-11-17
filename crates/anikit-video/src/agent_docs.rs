//! `anikit-video docs <topic>` and `anikit-video recipes [name]`: compact,
//! copy-pasteable pages for agents, so they read 30 lines instead of 3,000.
//! Every example and recipe is checked by the tests below.

use serde_json::Value;

use crate::templates::suggest;

/// `(topic, markdown)`; `start` is the index.
pub const PAGES: &[(&str, &str)] = &[
    ("start", include_str!("../docs-cli/start.md")),
    ("primitives", include_str!("../docs-cli/primitives.md")),
    ("animation", include_str!("../docs-cli/animation.md")),
    ("camera", include_str!("../docs-cli/camera.md")),
    ("compositions", include_str!("../docs-cli/compositions.md")),
    ("shader", include_str!("../docs-cli/shader.md")),
    ("3d", include_str!("../docs-cli/3d.md")),
    ("particles", include_str!("../docs-cli/particles.md")),
    ("physics", include_str!("../docs-cli/physics.md")),
    ("music", include_str!("../docs-cli/music.md")),
    ("post", include_str!("../docs-cli/post.md")),
    ("pacing", include_str!("../docs-cli/pacing.md")),
    ("critique", include_str!("../docs-cli/critique.md")),
    ("make-anything", include_str!("../docs-cli/make-anything.md")),
];

/// Hand-built shots: `(name, {"about", "spec"})`.
pub const RECIPES: &[(&str, &str)] = &[
    ("kinetic-slam", include_str!("../recipes/kinetic-slam.json")),
    ("whip-transition", include_str!("../recipes/whip-transition.json")),
    ("chrome-hero", include_str!("../recipes/chrome-hero.json")),
    ("particle-ignite", include_str!("../recipes/particle-ignite.json")),
    ("domino-cascade", include_str!("../recipes/domino-cascade.json")),
    ("raymarch-world", include_str!("../recipes/raymarch-world.json")),
    ("rack-focus-parallax", include_str!("../recipes/rack-focus-parallax.json")),
];

const ALIASES: &[(&str, &str)] = &[
    ("scene3d", "3d"), ("scene-3d", "3d"), ("three", "3d"), ("shaders", "shader"), ("glsl", "shader"), ("audio", "music"),
    ("sound", "music"), ("sfx", "music"), ("keyframes", "animation"), ("expressions", "animation"), ("text", "primitives"),
    ("shapes", "primitives"), ("paths", "primitives"), ("grade", "post"), ("motion-blur", "camera"), ("composition", "compositions"),
    ("review", "critique"), ("pace", "pacing"), ("index", "start"), ("anything", "make-anything"),
];

/// A docs page: `None` (or `start`) is the index; `component <kind>` / `components` come from the registry.
pub fn docs(topic: Option<&str>) -> Result<String, String> {
    let t = topic.unwrap_or("start").trim().to_lowercase();
    if t == "components" || t == "component" {
        let mut s = String::from("# Components: prebuilt widgets (shortcuts built from primitives)\n\n");
        for k in crate::components::KINDS {
            s += &format!("- `{}`: {} e.g. `{}`\n", k.name, k.summary, k.example);
        }
        return Ok(s);
    }
    if let Some(kind) = t.strip_prefix("component ") {
        let k = crate::components::KINDS.iter().find(|k| k.name == kind.trim()).ok_or_else(|| format!("unknown component '{kind}'; `anikit-video docs components` lists them"))?;
        return Ok(format!("# `{}`\n\n{}\n\n```json\n{{ \"component\": \"{}\", \"props\": {} }}\n```\n\nAll props: docs/video/components.md#{}\n", k.name, k.summary, k.name, k.example, k.name));
    }
    let t = ALIASES.iter().find(|(a, _)| *a == t).map(|(_, b)| b.to_string()).unwrap_or(t);
    if let Some((_, md)) = PAGES.iter().find(|(n, _)| *n == t) {
        return Ok(md.to_string());
    }
    let hint = suggest(&t, PAGES.iter().map(|(n, _)| *n)).map(|s| format!("did you mean '{s}'? ")).unwrap_or_default();
    Err(format!("unknown docs topic '{t}'. {hint}Topics: {}, components", PAGES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")))
}

/// `uniform float speed; // = 1.0  doc` → ("speed", "float", "1.0").
fn uniforms(src: &str) -> Vec<(String, String, String)> {
    src.lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix("uniform ")?;
            let (decl, comment) = rest.split_once(';')?;
            let mut it = decl.split_whitespace();
            let ty = it.next()?.to_string();
            let name = it.next()?.to_string();
            if ty.starts_with("sampler") {
                return None;
            }
            let def = comment.trim().strip_prefix("//").map(|c| c.trim().trim_start_matches('=').trim().split("  ").next().unwrap_or("").to_string()).unwrap_or_default();
            Some((name, ty, def))
        })
        .collect()
}

/// The recipe list, or one recipe: its notes and a ready spec.
pub fn recipes(name: Option<&str>) -> Result<String, String> {
    let lib = anikit_shader::library::ALL;
    let Some(n) = name else {
        let mut s = String::from("Shots (anikit-video recipes <name> prints a ready spec):\n");
        for (n, src) in RECIPES {
            let v: Value = serde_json::from_str(src).unwrap();
            let about = v["about"].as_str().unwrap_or("");
            s += &format!("  {n:20} {}\n", about.split(". ").next().unwrap_or(about).trim_end_matches('.'));
        }
        s += "Shaders (one line in a spec: {\"shader\": \"<name>\", \"uniforms\": {...}}; filters add \"filter\": true; transitions: \"transition\": \"<name>\"):\n";
        for e in lib {
            s += &format!("  {:13} {:10} {}\n", e.name, format!("{:?}", e.use_).to_lowercase(), e.about);
        }
        return Ok(s);
    };
    if let Some((_, src)) = RECIPES.iter().find(|(r, _)| *r == n) {
        let v: Value = serde_json::from_str(src).unwrap();
        return Ok(format!("# {n}\n\n{}\n\n```json\n{}\n```\n", v["about"].as_str().unwrap_or(""), crate::agent_cli::compact_json(&v["spec"])));
    }
    if let Some(e) = lib.iter().find(|e| e.name == n) {
        let us = uniforms(e.src);
        let ex: Vec<String> = us.iter().filter(|(_, t, d)| t == "float" && !d.is_empty()).map(|(n, _, d)| format!("\"{n}\": {d}")).collect();
        let use_ = format!("{:?}", e.use_).to_lowercase();
        let snippet = if use_.contains("transition") {
            format!("\"transition\": {{ \"type\": \"{}\", \"duration\": 0.6 }}", e.name)
        } else {
            format!("{{ \"shader\": \"{}\"{}, \"uniforms\": {{ {} }} }}", e.name, if use_.contains("filter") { ", \"filter\": true" } else { "" }, ex.join(", "))
        };
        let mut s = format!("# {} ({use_})\n\n{}\n\nUniforms:\n", e.name, e.about);
        for (n, t, d) in &us {
            s += &format!("- `{n}` {t} = {d}\n");
        }
        s += &format!("\n```json\n{snippet}\n```\n\nSource (copy and edit as your own shader): crates/anikit-shader/library/{}.glsl\n", e.name);
        return Ok(s);
    }
    let all: Vec<&str> = RECIPES.iter().map(|(n, _)| *n).chain(lib.iter().map(|e| e.name)).collect();
    let hint = suggest(n, all.iter().copied()).map(|s| format!("did you mean '{s}'? ")).unwrap_or_default();
    Err(format!("unknown recipe '{n}'. {hint}`anikit-video recipes` lists them"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiles(spec: Value, what: &str) {
        let p = crate::spec::parse::from_value(spec).unwrap_or_else(|e| panic!("{what}: {e:?}"));
        assert!(p.warnings.is_empty(), "{what}: {:?}", p.warnings);
        let opts = crate::eval::Options { fonts: Some(std::sync::Arc::new(crate::text::FontStore::new())), ..Default::default() };
        let ev = crate::eval::Evaluator::new(&p.spec, opts).unwrap_or_else(|e| panic!("{what}: {e}"));
        let errs: Vec<_> = ev.diagnostics.iter().filter(|d| d.to_string().starts_with("error")).collect();
        assert!(errs.is_empty(), "{what}: {errs:?}");
    }

    #[test]
    fn every_page_example_and_recipe_compiles() {
        for (n, md) in PAGES {
            assert!(md.lines().count() <= 60, "docs {n} is over 60 lines");
            for block in md.split("```json\n").skip(1) {
                let src = block.split("\n```").next().unwrap();
                let v: Value = serde_json::from_str(src).unwrap_or_else(|e| panic!("docs {n}: {e}\n{src}"));
                if v.get("scenes").is_some() && v.get("music").is_none() {
                    compiles(v, &format!("docs {n}"));
                }
            }
        }
        for (n, src) in RECIPES {
            let v: Value = serde_json::from_str(src).unwrap();
            assert!(v["about"].as_str().is_some_and(|a| a.len() > 20), "{n}");
            compiles(v["spec"].clone(), &format!("recipe {n}"));
        }
    }

    #[test]
    fn lookups_aliases_and_suggestions() {
        assert!(docs(None).unwrap().contains("docs 3d"));
        assert!(docs(Some("scene3d")).unwrap().contains("scene3d"));
        assert!(docs(Some("shadr")).unwrap_err().contains("did you mean 'shader'"));
        assert!(docs(Some("components")).unwrap().contains("bar-chart"));
        assert!(recipes(None).unwrap().contains("kinetic-slam") && recipes(None).unwrap().contains("sdf-scene"));
        assert!(recipes(Some("chrome-hero")).unwrap().contains("scene3d"));
        assert!(recipes(Some("aurora")).unwrap().contains("\"shader\": \"aurora\""));
        assert!(recipes(Some("kinetic-slm")).unwrap_err().contains("did you mean 'kinetic-slam'"));
    }
}
