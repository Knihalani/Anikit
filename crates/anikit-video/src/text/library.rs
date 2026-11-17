//! The font library: ~130 curated open-licensed (OFL) families, fetched on
//! first use.
//!
//! The catalogue (`fonts/library.json`) pins every file to one commit of
//! `github.com/google/fonts` plus its SHA-256, so a family resolves to the same
//! bytes on every machine. Files land in the cache directory
//! (`$ANIKIT_FONT_DIR`, else `$XDG_CACHE_HOME/anikit/fonts`, else
//! `~/.cache/anikit/fonts`) and are checked against their hash on every load.
//! Downloading shells out to `curl` (no HTTP stack in the binary).
//! `ANIKIT_OFFLINE=1` never touches the network.

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

/// One catalogue entry.
#[derive(Clone, Debug, Deserialize)]
pub struct Family {
    pub name: String,
    /// grotesque, geometric, humanist, didone, serif, slab, condensed, wide,
    /// blackletter, script, hand, stencil, mono, pixel, display.
    pub genre: String,
    /// One-line character description and best use.
    pub desc: String,
    /// Suggested partners (all in the catalogue).
    pub pairs: Vec<String>,
    /// Variable axes: `[tag, min, max]`.
    pub axes: Vec<(String, f32, f32)>,
    /// Shipped inside the binary (no fetch).
    #[serde(default)]
    pub bundled: bool,
    /// Directory in google/fonts (`ofl/fraunces`).
    #[serde(default)]
    pub dir: String,
    /// `[file name, size in bytes, sha256 hex]`.
    #[serde(default)]
    pub files: Vec<(String, u64, String)>,
}

#[derive(Deserialize)]
struct Catalogue {
    source: String,
    families: Vec<Family>,
}

fn catalogue() -> &'static Catalogue {
    static C: OnceLock<Catalogue> = OnceLock::new();
    C.get_or_init(|| serde_json::from_str(include_str!("../../fonts/library.json")).expect("fonts/library.json"))
}

/// Every family in the catalogue (bundled ones first).
pub fn families() -> &'static [Family] {
    &catalogue().families
}

fn key(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// A family by name; case, spaces and hyphens are ignored (`"big-shoulders"`).
pub fn find(name: &str) -> Option<&'static Family> {
    let k = key(name);
    if k.is_empty() {
        return None;
    }
    families().iter().find(|f| key(&f.name) == k)
}

/// Families matching every word of `query` against name, genre, description
/// and axis tags (empty query: all). Name matches rank first.
pub fn search(query: &str) -> Vec<&'static Family> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut hits: Vec<(usize, &Family)> = families()
        .iter()
        .filter_map(|f| {
            let hay = format!(
                "{} {} {} {} {}",
                f.name.to_lowercase(),
                f.genre,
                f.desc.to_lowercase(),
                f.axes.iter().map(|a| a.0.to_lowercase()).collect::<Vec<_>>().join(" "),
                if f.axes.is_empty() { "" } else { "variable" }
            );
            words.iter().all(|w| hay.contains(w.as_str())).then(|| {
                let rank = if words.iter().any(|w| key(&f.name).contains(&key(w))) { 0 } else { 1 };
                (rank, f)
            })
        })
        .collect();
    hits.sort_by_key(|h| h.0);
    hits.into_iter().map(|h| h.1).collect()
}

/// Closest catalogue names to a misspelt one (at most 3).
pub fn suggest(name: &str) -> Vec<&'static str> {
    let k = key(name);
    let mut v: Vec<(usize, &str)> = families()
        .iter()
        .map(|f| {
            let fk = key(&f.name);
            let d = if fk.starts_with(&k) || k.starts_with(&fk) { 0 } else { levenshtein(&k, &fk) };
            (d, f.name.as_str())
        })
        .filter(|(d, _)| *d <= (k.len() / 3).max(2))
        .collect();
    v.sort();
    v.into_iter().take(3).map(|x| x.1).collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j] + (ca != *cb) as usize).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Where fetched fonts live.
pub fn cache_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("ANIKIT_FONT_DIR") {
        return PathBuf::from(d);
    }
    if let Some(d) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(d).join("anikit/fonts");
    }
    if let Some(d) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(d).join("anikit/fonts");
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_default();
    PathBuf::from(home).join(".cache/anikit/fonts")
}

impl Family {
    fn file_path(&self, file: &str) -> PathBuf {
        cache_dir().join(self.dir.rsplit('/').next().unwrap_or(&self.dir)).join(file)
    }

    /// Total download size in bytes.
    pub fn size(&self) -> u64 {
        self.files.iter().map(|f| f.1).sum()
    }

    /// Every file is cached and intact.
    pub fn is_cached(&self) -> bool {
        self.bundled || self.files.iter().all(|(f, size, _)| std::fs::metadata(self.file_path(f)).is_ok_and(|m| m.len() == *size))
    }

    /// The family's font files, verified, downloading the missing ones.
    pub fn load(&self) -> Result<Vec<Vec<u8>>, String> {
        if self.bundled {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for (file, _, sha) in &self.files {
            let path = self.file_path(file);
            let cached = std::fs::read(&path).ok().filter(|b| sha256_hex(b) == *sha);
            let bytes = match cached {
                Some(b) => b,
                None => self.fetch(file, sha, &path)?,
            };
            out.push(bytes);
        }
        Ok(out)
    }

    fn fetch(&self, file: &str, sha: &str, path: &std::path::Path) -> Result<Vec<u8>, String> {
        let manual = || {
            format!(
                "font '{}' is not cached; put {} in {} (from {}{}/), or run `anikit-video fonts --fetch \"{}\"` online",
                self.name,
                file,
                path.parent().map(|p| p.display().to_string()).unwrap_or_default(),
                catalogue().source,
                self.dir,
                self.name
            )
        };
        if std::env::var_os("ANIKIT_OFFLINE").is_some_and(|v| v != "0") {
            return Err(format!("{} (ANIKIT_OFFLINE is set)", manual()));
        }
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let url = format!("{}{}/{}", catalogue().source, self.dir, file.replace('[', "%5B").replace(']', "%5D").replace(',', "%2C"));
        let tmp = path.with_extension(format!("part{}", std::process::id()));
        let st = std::process::Command::new("curl")
            .args(["-fsSL", "--retry", "2", "--max-time", "120", "-o"])
            .arg(&tmp)
            .arg(&url)
            .status();
        let fail = |why: String| {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("{}: download failed ({why})", manual()))
        };
        match st {
            Ok(s) if s.success() => {}
            Ok(s) => return fail(format!("curl exit {}", s.code().unwrap_or(-1))),
            Err(e) => return fail(format!("can't run curl: {e}")),
        }
        let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
        if sha256_hex(&bytes) != sha {
            return fail("checksum mismatch".into());
        }
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(bytes)
    }
}

/// SHA-256 of `data` as lowercase hex.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    let mut w = [0u32; 64];
    for chunk in msg.chunks_exact(64) {
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

/// `anikit-video fonts [query] [--fetch NAME…] [--json]` output.
pub fn listing(query: &str, json: bool) -> String {
    let hits = search(query);
    if json {
        let v: Vec<serde_json::Value> = hits
            .iter()
            .map(|f| {
                serde_json::json!({
                    "name": f.name, "genre": f.genre, "desc": f.desc, "pairs": f.pairs,
                    "axes": f.axes.iter().map(|a| format!("{} {}-{}", a.0, a.1, a.2)).collect::<Vec<_>>(),
                    "cached": f.is_cached(),
                })
            })
            .collect();
        return serde_json::to_string(&v).unwrap();
    }
    if hits.is_empty() {
        return format!("no fonts match '{query}'. Try a genre: grotesque geometric humanist didone serif slab condensed wide blackletter script hand stencil mono pixel display, or an axis tag (wdth, opsz, SOFT).");
    }
    let mut s = String::new();
    let mut genre = "";
    let grouped = query.trim().is_empty();
    let mut sorted = hits.clone();
    if grouped {
        sorted.sort_by_key(|f| GENRES.iter().position(|g| *g == f.genre).unwrap_or(99));
    }
    for f in sorted {
        if grouped && f.genre != genre {
            genre = &f.genre;
            s.push_str(&format!("\n## {genre}\n"));
        }
        let axes = if f.axes.is_empty() {
            String::new()
        } else {
            format!(" [{}]", f.axes.iter().map(|a| format!("{} {}-{}", a.0, a.1, a.2)).collect::<Vec<_>>().join(", "))
        };
        let mark = if f.bundled { " (bundled)" } else if f.is_cached() { " (cached)" } else { "" };
        s.push_str(&format!("{}{}{} — {}. Pairs: {}\n", f.name, mark, axes, f.desc, f.pairs.join(", ")));
    }
    if grouped {
        s.push_str(&format!(
            "\n{} families. Use `\"font\": \"Name\"`; library fonts download once (~50-500 KB) to {}. Animate axes with `\"axes\": {{\"wght\": 800}}` (anikit-video docs type).\n",
            families().len(),
            cache_dir().display()
        ));
    }
    s
}

const GENRES: &[&str] =
    &["grotesque", "geometric", "humanist", "didone", "serif", "slab", "condensed", "wide", "blackletter", "script", "hand", "stencil", "mono", "pixel", "display"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known() {
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let long = vec![b'a'; 1000];
        assert_eq!(sha256_hex(&long), "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3");
    }

    #[test]
    fn catalogue_is_consistent() {
        assert!(families().len() >= 100);
        for f in families() {
            assert!(GENRES.contains(&f.genre.as_str()), "{}: genre {}", f.name, f.genre);
            assert!(f.bundled || !f.files.is_empty(), "{}", f.name);
            for p in &f.pairs {
                assert!(find(p).is_some(), "{} pairs with unknown {p}", f.name);
            }
            for (_, _, sha) in &f.files {
                assert_eq!(sha.len(), 64);
            }
        }
        assert_eq!(find("big-shoulders").unwrap().name, "Big Shoulders");
        assert_eq!(find("FRAUNCES").unwrap().name, "Fraunces");
        assert!(suggest("Frances").contains(&"Fraunces"));
        assert_eq!(search("didone")[0].genre, "didone");
        assert!(search("SOFT").iter().any(|f| f.name == "Fraunces"));
    }
}

/// Font names a spec mentions: string values under any key containing
/// "font" (`font`, `fontFamily`, `theme.fonts.heading`…), split on commas,
/// with their JSON paths. The top-level `fonts` map (name → file) is skipped.
pub fn spec_font_names(spec: &serde_json::Value) -> Vec<(String, String)> {
    fn walk(v: &serde_json::Value, path: &str, in_font: bool, out: &mut Vec<(String, String)>) {
        match v {
            serde_json::Value::String(s) if in_font => {
                for n in s.split(',') {
                    let n = n.trim().trim_matches(|c| c == '"' || c == '\'');
                    let low = n.to_ascii_lowercase();
                    if !n.is_empty() && !low.ends_with(".ttf") && !low.ends_with(".otf") && !low.ends_with(".ttc") && !low.contains('/') {
                        out.push((path.to_string(), n.to_string()));
                    }
                }
            }
            serde_json::Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    walk(x, &format!("{path}[{i}]"), in_font, out);
                }
            }
            serde_json::Value::Object(m) => {
                for (k, x) in m {
                    if path.is_empty() && k == "fonts" {
                        continue;
                    }
                    let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                    walk(x, &p, in_font || k.to_ascii_lowercase().contains("font"), out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(spec, "", false, &mut out);
    out
}
