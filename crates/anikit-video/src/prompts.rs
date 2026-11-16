//! The prompt library (`prompts/*.md`), compiled in so the CLI and the MCP
//! server can serve it anywhere. `template:<name>` guides are generated from
//! each template's declaration, so they never drift from the parameters.

use serde_json::Value;

use crate::templates::{suggest, Template, BUILTIN};

/// `(name, markdown)` for every hand-written prompt.
pub const PROMPTS: &[(&str, &str)] = &[
    ("launch-video-generator", include_str!("../../../prompts/launch-video-generator.md")),
    ("script-writer", include_str!("../../../prompts/script-writer.md")),
    ("storyboarder", include_str!("../../../prompts/storyboarder.md")),
    ("critic", include_str!("../../../prompts/critic.md")),
    ("copy-guard", include_str!("../../../prompts/copy-guard.md")),
    ("brand-kit", include_str!("../../../prompts/brand-kit.md")),
    ("motion-rules", include_str!("../../../prompts/motion-rules.md")),
];

/// The `description:` line of a prompt's front matter.
pub fn description(md: &str) -> String {
    md.lines()
        .skip(1)
        .take_while(|l| *l != "---")
        .find_map(|l| l.strip_prefix("description:"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// The body without front matter.
pub fn body(md: &str) -> &str {
    if let Some(rest) = md.strip_prefix("---\n") {
        if let Some(i) = rest.find("\n---\n") {
            return rest[i + 5..].trim_start();
        }
    }
    md
}

/// Every prompt name, including the generated `template:<name>` guides.
pub fn names() -> Vec<String> {
    let mut v: Vec<String> = PROMPTS.iter().map(|(n, _)| n.to_string()).collect();
    v.extend(BUILTIN.iter().map(|(n, _)| format!("template:{n}")));
    v
}

/// `(name, description)` for listings.
pub fn list() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = PROMPTS.iter().map(|(n, md)| (n.to_string(), description(md))).collect();
    for (n, _) in BUILTIN {
        if let Ok(t) = Template::builtin(n) {
            let title = t.doc.get("title").and_then(|x| x.as_str()).unwrap_or(n);
            v.push((format!("template:{n}"), format!("How to fill the '{n}' template ({title}) well: slots, word limits, examples, tips.")));
        }
    }
    v
}

/// A prompt by name (`template:<name>`, or a bare template name, gives its guide).
pub fn get(name: &str) -> Result<String, String> {
    if let Some((_, md)) = PROMPTS.iter().find(|(n, _)| *n == name) {
        return Ok(md.to_string());
    }
    let tname = name.strip_prefix("template:").unwrap_or(name);
    if let Ok(t) = Template::builtin(tname) {
        return Ok(template_guide(&t));
    }
    let all = names();
    let hint = suggest(name, all.iter().map(|s| s.as_str())).map(|s| format!("did you mean '{s}'? ")).unwrap_or_default();
    Err(format!("unknown prompt '{name}'. {hint}Available: {}", all.join(", ")))
}

fn limits(p: &Value) -> String {
    let mut v = Vec::new();
    if let Some(n) = p.get("maxWords") {
        v.push(format!("≤ {n} words"));
    }
    if let Some(n) = p.get("maxChars") {
        v.push(format!("≤ {n} chars"));
    }
    if let (Some(a), Some(b)) = (p.get("minItems"), p.get("maxItems")) {
        v.push(format!("{a}–{b} items"));
    }
    if let Some(vals) = p.get("values").and_then(|x| x.as_array()) {
        v.push(vals.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" / "));
    }
    v.join(", ")
}

/// A guide for one template, generated from its declaration.
pub fn template_guide(t: &Template) -> String {
    let d = &t.doc;
    let s = |k: &str| d.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut out = format!(
        "---\nname: template:{name}\ndescription: How to fill the '{name}' template well.\n---\n\n# Template guide: {title} (`{name}`)\n\n{desc}\n\n**Typical length:** {dur}. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.\n\n## Parameters\n\n| Param | Type | Required | Limits | What to write |\n|---|---|---|---|---|\n",
        name = t.name,
        title = s("title"),
        desc = s("description"),
        dur = if s("duration").is_empty() { "varies".into() } else { s("duration") },
    );
    for (k, p) in t.params() {
        let ty = p.get("type").and_then(|x| x.as_str()).unwrap_or("string");
        let req = if p.get("required") == Some(&Value::Bool(true)) { "yes" } else { "" };
        let mut what = p.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if let Some(def) = p.get("default") {
            let shown = match def.as_str() {
                Some(d) if d.starts_with("{{brand.") => format!("the brand kit's `{}`", d.trim_start_matches("{{brand.").trim_end_matches("}}")),
                _ => format!("`{def}`"),
            };
            what += &format!(" Default: {shown}.");
        }
        out += &format!("| `{k}` | {ty} | {req} | {} | {what} |\n", limits(&p));
        if let Some(props) = p.get("items").and_then(|i| i.get("properties")).or_else(|| p.get("properties")).and_then(|x| x.as_object()) {
            for (sk, sp) in props {
                let sty = sp.get("type").and_then(|x| x.as_str()).unwrap_or("string");
                let sreq = if sp.get("required") == Some(&Value::Bool(true)) { "yes" } else { "" };
                let sd = sp.get("description").and_then(|x| x.as_str()).unwrap_or("");
                out += &format!("| `{k}[].{sk}` | {sty} | {sreq} | {} | {sd} |\n", limits(sp));
            }
        }
    }
    out += &format!(
        "\n## Example\n\n```json\n{}\n```\n\nCreate: MCP `create_from_template {{template: \"{name}\", params, brand, format}}`, or `anikit-video templates expand {name} params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.\n",
        t.example_params().to_string(),
        name = t.name
    );
    if let Some(tips) = d.get("tips").and_then(|x| x.as_array()) {
        out += "\n## Tips\n\n";
        for tip in tips.iter().filter_map(|x| x.as_str()) {
            out += &format!("- {tip}\n");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_and_guides() {
        for (n, md) in PROMPTS {
            assert!(!description(md).is_empty(), "{n} has no description");
            assert!(body(md).starts_with('#'), "{n} body should start with a heading");
        }
        let g = get("template:launch").unwrap();
        assert!(g.contains("`hook`") && g.contains("≤ 7 words"), "{g}");
        assert!(get("launch-video-generater").unwrap_err().contains("did you mean 'launch-video-generator'"));
    }
}

#[cfg(test)]
mod sync {
    /// `prompts/templates/<name>.md` are generated; regenerate with
    /// `for n in …; do anikit-video templates guide $n > prompts/templates/$n.md; done`.
    #[test]
    fn template_guides_are_up_to_date() {
        for (n, _) in crate::templates::BUILTIN {
            let path = format!("{}/../../prompts/templates/{n}.md", env!("CARGO_MANIFEST_DIR"));
            let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
            let fresh = super::template_guide(&crate::templates::Template::builtin(n).unwrap());
            assert!(on_disk == fresh, "{path} is stale: run `anikit-video templates guide {n} > prompts/templates/{n}.md`");
        }
    }
}
