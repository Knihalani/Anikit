//! Per-kind documentation pages for agents (`anikit-video docs component <kind>`),
//! cut from `docs/video/components.md` so there is one source of truth.

use super::{camel, canonical, KINDS};

const MD: &str = include_str!("../../../../docs/video/components.md");

/// Sections of the components guide: `(level, heading, body)`.
fn sections() -> Vec<(usize, &'static str, &'static str)> {
    let mut out = Vec::new();
    let mut starts: Vec<(usize, usize, usize)> = Vec::new(); // (level, heading start, body start)
    let mut pos = 0;
    for line in MD.split_inclusive('\n') {
        let lvl = line.bytes().take_while(|&b| b == b'#').count();
        if (2..=3).contains(&lvl) && line.as_bytes().get(lvl) == Some(&b' ') {
            starts.push((lvl, pos, pos + line.len()));
        }
        pos += line.len();
    }
    for (i, &(lvl, hs, bs)) in starts.iter().enumerate() {
        // A section runs until the next heading of the same or a higher level.
        let end = starts[i + 1..].iter().find(|s| s.0 <= lvl).map(|s| s.1).unwrap_or(MD.len());
        let next = starts.get(i + 1).map(|s| s.1).unwrap_or(MD.len()).min(end);
        let body_end = if lvl == 2 { next } else { end };
        out.push((lvl, MD[hs..bs].trim_end(), MD[bs..body_end].trim()));
    }
    out
}

fn mentions(text: &str, kind: &str) -> bool {
    let c = camel(kind);
    text.contains(&format!("`{c}`")) || text.contains(&format!("`{kind}`"))
}

/// The documentation page for one component kind (camelCase, kebab-case or an alias),
/// or `None` for an unknown kind.
pub fn doc(kind: &str) -> Option<String> {
    let name = canonical(kind);
    let k = KINDS.iter().find(|k| k.name == name)?;
    let mut page = format!("# {} ({})\n{}\n\nExample: {{\"component\": \"{}\", \"props\": {}}}\n", camel(k.name), k.name, k.summary, camel(k.name), k.example);
    let secs = sections();
    if let Some((_, h, body)) = secs.iter().find(|(l, h, _)| *l == 3 && mentions(h, k.name)) {
        page.push_str(&format!("\n{h}\n{body}\n"));
        return Some(page);
    }
    // Grouped kinds: the group's text up to its first table, plus the table rows naming this kind.
    if let Some((_, h, body)) = secs.iter().find(|(l, _, b)| *l == 2 && mentions(b, k.name)) {
        page.push_str(&format!("\n{h}\n"));
        let mut header: Vec<&str> = Vec::new();
        let mut in_table = false;
        let mut prose = 0;
        for line in body.lines() {
            if line.starts_with('|') {
                if !in_table {
                    header.clear();
                    in_table = true;
                }
                if header.len() < 2 {
                    header.push(line);
                } else if mentions(line, k.name) {
                    for hl in header.drain(..) {
                        page.push_str(hl);
                        page.push('\n');
                    }
                    page.push_str(line);
                    page.push('\n');
                }
            } else {
                in_table = false;
                // Shared prose (common props, looks) applies to every kind in the group.
                if prose < 30 && !line.trim().is_empty() {
                    prose += 1;
                    page.push_str(line);
                    page.push('\n');
                }
            }
        }
    }
    Some(page)
}

/// One line per kind: `camelName — summary`.
pub fn index() -> String {
    let mut s = String::from("# Components (`anikit-video docs component <name>` for one)\n");
    for k in KINDS {
        s.push_str(&format!("{} — {}\n", camel(k.name), k.summary));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_page() {
        for k in KINDS {
            let p = doc(k.name).unwrap();
            assert!(p.lines().count() >= 4, "{}: thin page\n{p}", k.name);
        }
        assert!(doc("barChart").unwrap().contains("bar"));
        assert!(doc("nope").is_none());
        assert!(index().lines().count() > KINDS.len());
    }
}

#[cfg(test)]
#[test]
#[ignore]
fn print_pages() {
    for k in std::env::var("KINDS").unwrap_or("phone,aurora,button".into()).split(',') {
        println!("{}\n-----", doc(k).unwrap());
    }
}
