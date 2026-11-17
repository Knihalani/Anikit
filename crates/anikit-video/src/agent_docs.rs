//! `anikit-video docs <topic>` and `anikit-video recipes [name]`: compact,
//! copy-pasteable pages for agents, so they read 30 lines instead of 3,000.
//! Every example and recipe is checked by the tests below.

use serde_json::Value;

use crate::templates::suggest;

/// `(topic, markdown)`; `start` is the index.
pub const PAGES: &[(&str, &str)] = &[
    ("start", include_str!("../docs-cli/start.md")),
    ("taste", include_str!("../docs-cli/taste.md")),
    ("primitives", include_str!("../docs-cli/primitives.md")),
    ("animation", include_str!("../docs-cli/animation.md")),
    ("camera", include_str!("../docs-cli/camera.md")),
    ("compositions", include_str!("../docs-cli/compositions.md")),
    ("shader", include_str!("../docs-cli/shader.md")),
    ("3d", include_str!("../docs-cli/3d.md")),
    ("particles", include_str!("../docs-cli/particles.md")),
    ("physics", include_str!("../docs-cli/physics.md")),
    ("fluid", include_str!("../docs-cli/fluid.md")),
    ("physarum", include_str!("../docs-cli/physarum.md")),
    ("post", include_str!("../docs-cli/post.md")),
    ("pacing", include_str!("../docs-cli/pacing.md")),
    ("critique", include_str!("../docs-cli/critique.md")),
    ("make-anything", include_str!("../docs-cli/make-anything.md")),
    ("expressions", include_str!("../docs-cli/expressions.md")),
    ("script", include_str!("../docs-cli/script.md")),
    ("draw", include_str!("../docs-cli/draw.md")),
    ("mograph", include_str!("../docs-cli/mograph.md")),
    ("type", include_str!("../docs-cli/type.md")),
    ("graphic", include_str!("../docs-cli/graphic.md")),
    ("launch", include_str!("../docs-cli/launch.md")),
    ("analyze", include_str!("../docs-cli/analyze.md")),
    ("board", include_str!("../docs-cli/board.md")),
    ("assets", include_str!("../docs-cli/assets.md")),
];

/// Hand-built shots: `(name, {"about", "spec"})`.
pub const RECIPES: &[(&str, &str)] = &[
    ("line-drawing", include_str!("../recipes/line-drawing.json")),
    ("window-light", include_str!("../recipes/window-light.json")),
    ("launch-sting", include_str!("../recipes/launch-sting.json")),
    ("kinetic-slam", include_str!("../recipes/kinetic-slam.json")),
    ("whip-transition", include_str!("../recipes/whip-transition.json")),
    ("chrome-hero", include_str!("../recipes/chrome-hero.json")),
    ("particle-ignite", include_str!("../recipes/particle-ignite.json")),
    ("domino-cascade", include_str!("../recipes/domino-cascade.json")),
    ("raymarch-world", include_str!("../recipes/raymarch-world.json")),
    ("swiss-grid-poster", include_str!("../recipes/swiss-grid-poster.json")),
    ("riso-overprint", include_str!("../recipes/riso-overprint.json")),
    ("torn-collage", include_str!("../recipes/torn-collage.json")),
    ("op-art-field", include_str!("../recipes/op-art-field.json")),
];

const ALIASES: &[(&str, &str)] = &[
    ("scene3d", "3d"), ("ink", "fluid"), ("slime", "physarum"), ("slime-mould", "physarum"), ("network", "physarum"), ("smoke", "fluid"), ("fire", "fluid"), ("scene-3d", "3d"), ("three", "3d"), ("shaders", "shader"), ("glsl", "shader"), ("audio", "music"),
    ("sound", "music"), ("sfx", "music"), ("keyframes", "animation"), ("expression", "expressions"), ("scripts", "script"), ("rhai", "script"), ("drawing", "draw"), ("motion-graphics", "mograph"), ("typography", "type"), ("fonts", "type"), ("font", "type"), ("axes", "type"), ("variable", "type"), ("features", "type"), ("opentype", "type"), ("repeater", "mograph"), ("trails", "mograph"), ("echo", "mograph"), ("grid", "graphic"), ("pattern", "graphic"), ("patterns", "graphic"), ("halftone", "graphic"), ("duotone", "graphic"), ("print", "graphic"), ("graphic-design", "graphic"), ("layout", "graphic"), ("text", "primitives"),
    ("shapes", "primitives"), ("paths", "primitives"), ("grade", "post"), ("motion-blur", "camera"), ("composition", "compositions"),
    ("review", "critique"), ("pace", "pacing"), ("index", "start"), ("anything", "make-anything"),
    ("launch-video", "launch"), ("product", "launch"), ("announcement", "launch"),
    ("analyse", "analyze"), ("design", "taste"), ("style", "taste"), ("quality", "taste"), ("canon", "taste"), ("reverse", "analyze"), ("reference", "analyze"), ("moodboard", "board"), ("mood-board", "board"), ("references", "board"), ("asset", "assets"), ("hdri", "assets"), ("hdris", "assets"), ("materials", "assets"), ("textures", "assets"), ("models", "assets"), ("lut", "assets"), ("luts", "assets"), ("library", "assets"),
];

/// A docs page: `None` (or `start`) is the index; `component <kind>` / `components` come from the registry.
pub fn docs(topic: Option<&str>) -> Result<String, String> {
    let t = topic.unwrap_or("start").trim().to_lowercase();
    if t == "components" || t == "component" {
        return Ok(crate::components::docs::index());
    }
    if let Some(kind) = t.strip_prefix("component ") {
        return crate::components::docs::doc(kind.trim()).ok_or_else(|| format!("unknown component '{kind}'; `anikit-video docs components` lists them"));
    }
    let t = ALIASES.iter().find(|(a, _)| *a == t).map(|(_, b)| b.to_string()).unwrap_or(t);
    if t == "music" {
        return Ok(crate::music::docs().to_string());
    }
    if let Some(k) = t.strip_prefix("key ") {
        // The key as typed (keys are camelCase).
        let raw = topic.unwrap_or("").trim();
        return key_doc(raw.get(4..).unwrap_or(k).trim());
    }
    if t == "essentials" || t == "all" {
        // Everything a typical piece needs in one read (about 13k characters).
        let mut s = String::from("# Essentials: one read for a typical piece\n\nLoop: write s.json → `anikit-video preview s.json` (repeat) → ONE `render s.json -o out.mp4`. More: `anikit-video docs <topic>` (3d, particles, physics, post, pacing, taste, critique, graphic: grids, patterns, print treatments).\n\n");
        for p in ["primitives", "animation", "expressions", "compositions", "shader"] {
            if let Some((_, md)) = PAGES.iter().find(|(n, _)| *n == p) {
                s += md;
                s += "\n";
            }
        }
        s += crate::music::docs();
        return Ok(s);
    }
    if let Some((_, md)) = PAGES.iter().find(|(n, _)| *n == t) {
        return Ok(md.to_string());
    }
    let hint = suggest(&t, PAGES.iter().map(|(n, _)| *n).chain(["music", "components"])).map(|s| format!("did you mean '{s}'? ")).unwrap_or_default();
    Err(format!("unknown docs topic '{t}'. {hint}Topics: {}, music, components", PAGES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")))
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

/// A recipe's notes and ready spec: hand-built shots, or a shader-library entry wrapped in a minimal spec.
pub fn recipe_spec(name: &str) -> Option<(String, Value)> {
    let Some((_, src)) = RECIPES.iter().find(|(r, _)| *r == name) else {
        return library_spec(name);
    };
    let v: Value = serde_json::from_str(src).ok()?;
    let mut about = v["about"].as_str().unwrap_or("").to_string();
    if let Some(c) = v["cost"].as_str() {
        about += &format!(" Cost: {c}.");
    }
    Some((about, v["spec"].clone()))
}

/// A list line: the first clause, cut at a word near 90 characters (the
/// full text is one `recipes <name>` away).
fn short(about: &str) -> String {
    let first = about.split(". ").next().unwrap_or(about).trim_end_matches('.');
    if first.chars().count() <= 90 {
        return first.to_string();
    }
    let cut: String = first.chars().take(90).collect();
    format!("{}…", cut.rsplit_once(' ').map(|(a, _)| a).unwrap_or(&cut).trim_end_matches([',', ';']))
}

/// The recipe list, or one recipe: its notes and a ready spec.
pub fn recipes(name: Option<&str>) -> Result<String, String> {
    let lib = anikit_shader::library::ALL;
    let Some(n) = name else {
        let mut s = String::from("Shots: techniques to borrow, each naming the taste decision it shows and when not to use it (anikit-video recipes <name>; docs taste first). Costs are draft and final at 1080p on 4 cores, so iterate in draft and run a 3D final in the background:\n");
        for (n, src) in RECIPES {
            let v: Value = serde_json::from_str(src).unwrap();
            let about = v["about"].as_str().unwrap_or("");
            let cost = v["cost"].as_str().map(|c| format!(" [{}]", c.split(" (").next().unwrap_or(c))).unwrap_or_default();
            s += &format!("  {n:20} {}{cost}\n", short(about));
        }
        s += "Shaders (one line in a spec: {\"shader\": \"<name>\", \"uniforms\": {...}}; filters add \"filter\": true; transitions: \"transition\": \"<name>\"):\n";
        for e in lib {
            s += &format!("  {:13} {:10} {}\n", e.name, format!("{:?}", e.use_).to_lowercase(), short(e.about));
        }
        return Ok(s);
    };
    if let Some((_, src)) = RECIPES.iter().find(|(r, _)| *r == n) {
        let v: Value = serde_json::from_str(src).unwrap();
        let cost = v["cost"].as_str().map(|c| format!("\n\nCost: {c}.")).unwrap_or_default();
        return Ok(format!("# {n}\n\n{}{cost}\n\n`anikit-video recipes {n} -o {n}.json` writes the spec.\n\n```json\n{}\n```\n", v["about"].as_str().unwrap_or(""), crate::agent_cli::compact_json(&v["spec"])));
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

/// Paste-able fragments: type treatments, camera moves, music and grade blocks.
const SNIPPETS: &str = include_str!("../snippets/snippets.json");

/// Where a snippet goes in a spec, and the fragment itself.
fn snippet_place(v: &Value) -> String {
    match v["kind"].as_str().unwrap_or("") {
        "layer" => "a layer: put it in a scene's \"layers\"".into(),
        "scene-key" => format!("a scene key: \"{}\": <this>", v["key"].as_str().unwrap_or("")),
        _ => format!("a top-level key: \"{}\": <this>", v["key"].as_str().unwrap_or("")),
    }
}

/// The snippet list, or one snippet as compact JSON with where it goes.
pub fn snippets(name: Option<&str>) -> Result<String, String> {
    let all: Vec<Value> = serde_json::from_str(SNIPPETS).map_err(|e| e.to_string())?;
    let Some(n) = name else {
        let mut s = String::from("Snippets (anikit-video snippets <name> prints one; paste and change the words and colours):\n");
        for v in &all {
            s += &format!("  {:18} {:9} {}\n", v["name"].as_str().unwrap_or(""), v["kind"].as_str().unwrap_or(""), v["about"].as_str().unwrap_or(""));
        }
        return Ok(s);
    };
    match all.iter().find(|v| v["name"] == n) {
        Some(v) => Ok(format!("// {}\n// {}\n{}\n", v["about"].as_str().unwrap_or(""), snippet_place(v), crate::agent_cli::compact_json(&v["json"]))),
        None => Err(format!("no snippet '{n}'; run `anikit-video snippets` for the list")),
    }
}

/// `docs key <name>`: every spec property with that name, from the JSON
/// Schema (type, description, examples), a few lines each.
fn key_doc(name: &str) -> Result<String, String> {
    let schema: Value = serde_json::from_str(&crate::spec::schema_json()).map_err(|e| e.to_string())?;
    let defs = schema.get("definitions").or_else(|| schema.get("$defs")).and_then(|d| d.as_object()).cloned().unwrap_or_default();
    let mut out = String::new();
    let mut all = std::collections::BTreeSet::new();
    let ty = |p: &Value| -> String {
        if let Some(r) = p.get("$ref").and_then(|r| r.as_str()) {
            return r.rsplit('/').next().unwrap_or(r).to_string();
        }
        for k in ["anyOf", "oneOf", "allOf"] {
            if let Some(a) = p.get(k).and_then(|a| a.as_array()) {
                return a.iter().filter_map(|x| x.get("$ref").and_then(|r| r.as_str()).map(|r| r.rsplit('/').next().unwrap_or(r).to_string()).or_else(|| x.get("type").map(|t| t.to_string().replace('"', "")))).collect::<Vec<_>>().join(" | ");
            }
        }
        p.get("type").map(|t| t.to_string().replace('"', "")).unwrap_or_default()
    };
    for (def, d) in &defs {
        let Some(props) = d.get("properties").and_then(|p| p.as_object()) else { continue };
        for (k, p) in props {
            all.insert(k.clone());
            if k.eq_ignore_ascii_case(name) {
                let desc = p.get("description").and_then(|d| d.as_str()).unwrap_or("").replace('\n', " ");
                let ex = p.get("examples").map(|e| format!("\n  e.g. {}", crate::agent_cli::compact_json(e))).unwrap_or_default();
                // An object type: list its fields, so one call answers "what goes inside".
                let t = ty(p);
                // Follow `A | null` and `string | AObj` wrappers one level down.
                let mut names: Vec<String> = t.split(" | ").map(str::to_string).collect();
                for n in names.clone() {
                    if let Some(a) = defs.get(&n).and_then(|d| d.get("anyOf").or_else(|| d.get("oneOf"))).and_then(|a| a.as_array()) {
                        names.extend(a.iter().filter_map(|x| x.get("$ref").and_then(|r| r.as_str()).map(|r| r.rsplit('/').next().unwrap_or(r).to_string())));
                        if a.iter().any(|x| x.get("type").and_then(|t| t.as_str()) == Some("string")) {
                            names.push("string".into());
                        }
                    }
                }
                let fields = names.iter().filter_map(|n| defs.get(n)).filter_map(|d| d.get("properties").and_then(|p| p.as_object())).map(|p| p.keys().cloned().collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join(" / ");
                let fields = if !fields.is_empty() && names.iter().any(|n| n == "string") { format!("a string, or {{ {fields} }}") } else { fields };
                let fields = if fields.is_empty() { String::new() } else { format!("\n  fields: {fields}") };
                let desc = if desc.is_empty() { String::new() } else { format!("\n  {desc}") };
                out += &format!("{def}.{k}: {t}{desc}{fields}{ex}\n");
            }
        }
    }
    if out.is_empty() {
        let hint = suggest(name, all.iter().map(|s| s.as_str())).map(|s| format!(" Did you mean '{s}'?")).unwrap_or_default();
        return Err(format!("no spec key '{name}'.{hint} (`anikit-video schema` has them all)"));
    }
    Ok(out)
}

/// A shader-library entry as a minimal ready spec (layer, filter or transition).
fn library_spec(n: &str) -> Option<(String, Value)> {
    let e = anikit_shader::library::ALL.iter().find(|e| e.name == n)?;
    let use_ = format!("{:?}", e.use_).to_lowercase();
    let spec = if use_.contains("transition") {
        serde_json::json!({ "scenes": [ "Before", { "transition": { "type": e.name, "duration": 0.6 }, "layers": [ "After" ] } ] })
    } else if use_.contains("filter") {
        serde_json::json!({ "scenes": [ { "layers": [ { "text": "Filtered", "style": "title" }, { "shader": e.name, "filter": true } ] } ] })
    } else {
        serde_json::json!({ "scenes": [ { "duration": 4, "layers": [ { "shader": e.name } ] } ] })
    };
    Some((e.about.to_string(), spec))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_snippet_compiles_in_place() {
        let all: Vec<Value> = serde_json::from_str(SNIPPETS).unwrap();
        for v in &all {
            let frag = v["json"].clone();
            let key = v["key"].as_str().unwrap_or("");
            let mut spec = serde_json::json!({ "size": "16:9", "scenes": [{ "duration": 3, "layers": [{ "text": "x" }] }] });
            match v["kind"].as_str().unwrap() {
                "layer" => spec["scenes"][0]["layers"].as_array_mut().unwrap().push(frag),
                "scene-key" => spec["scenes"][0][key] = frag,
                _ => spec[key] = frag,
            }
            crate::spec::parse::from_value(spec).unwrap_or_else(|d| panic!("{}: {:?}", v["name"], d.iter().map(|x| x.to_string()).collect::<Vec<_>>()));
        }
        assert!(snippets(None).unwrap().contains("type-slam"));
        assert!(docs(Some("key anchor")).unwrap().contains("anchor"));
        assert!(docs(Some("key ancor")).unwrap_err().contains("anchor"));
        let e = docs(Some("essentials")).unwrap();
        assert!(e.len() < 15_000 && e.contains("compositions") && e.contains("shader"), "{}", e.len());
        assert!(snippets(Some("snap-zoom")).unwrap().contains("\"shake\""));
    }

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
            assert!(md.lines().count() <= 80, "docs {n} is over 80 lines");
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
        assert!(docs(None).unwrap().contains("docs 3d") && docs(None).unwrap().contains("docs taste"));
        assert!(docs(Some("taste")).unwrap().contains("striking as a still") && docs(Some("design")).unwrap().contains("What tends to land"));
        assert!(docs(Some("scene3d")).unwrap().contains("scene3d"));
        assert!(docs(Some("shadr")).unwrap_err().contains("did you mean 'shader'"));
        assert!(docs(Some("components")).unwrap().contains("terminal"));
        assert!(recipes(None).unwrap().contains("kinetic-slam") && recipes(None).unwrap().contains("sdf-scene"));
        assert!(recipes(Some("chrome-hero")).unwrap().contains("scene3d"));
        assert!(recipes(Some("aurora")).unwrap().contains("\"shader\": \"aurora\""));
        assert!(recipes(Some("kinetic-slm")).unwrap_err().contains("did you mean 'kinetic-slam'"));
    }
}
