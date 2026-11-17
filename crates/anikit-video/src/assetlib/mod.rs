//! On-demand, freely licensed (CC0) assets: HDRIs, PBR texture sets, glTF
//! models, film LUTs and photographic textures, referenced from a spec by
//! name (`"polyhaven:hdri/kloppenheim_06"`, `"ambientcg:Wood049"`,
//! `"anikit:lut/teal-orange"`) and fetched once into a local cache.
//!
//! - The curated index (`assets/index.json`, ~220 entries) pins every file:
//!   URL, size and checksum (Poly Haven's md5, or a sha256 we computed), plus
//!   licence, author and a one-line description for `assets search`.
//! - Poly Haven and ambientCG assets outside the index resolve live through
//!   their public APIs (still checksum-verified for Poly Haven).
//! - Files land in `$ANIKIT_ASSET_CACHE` (default `~/.cache/anikit/assets`),
//!   one folder per asset and resolution, marked complete only after every
//!   file verified; a second use is a stat, not a download.
//! - Downloads use the system `curl` (no TLS stack in the binary).
//!   `ANIKIT_OFFLINE=1` never touches the network.
//! - Film LUTs (`anikit:lut/*`) are generated, so they need no network at all.

pub mod hash;
pub mod lut;
pub mod zip;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Sources `src` strings may name.
pub const PROVIDERS: &[&str] = &["polyhaven", "ambientcg", "khronos", "anikit"];
/// Asset types (`assets search --type`).
pub const TYPES: &[&str] = &["hdri", "texture", "model", "lut", "image"];

/// One downloadable file of an asset.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FileRef {
    /// Path inside the asset's cache folder (may contain `/`, e.g. `textures/x.jpg`).
    pub path: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// `main` (the file to open), or a PBR map: `albedo normal roughness ao displacement metalness`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// A zip to unpack (its image maps are kept, roles come from file names).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unzip: bool,
}

/// An index entry.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub desc: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub page: String,
    #[serde(default)]
    pub version: String,
    pub license: String,
    #[serde(rename = "licenseUrl", default)]
    pub license_url: String,
    /// Default resolution key of `res`.
    pub default: String,
    /// Resolution (`1k`, `2k`, `4k`, `glb`) → files.
    #[serde(default)]
    pub res: BTreeMap<String, Vec<FileRef>>,
    /// Real-world width of one texture tile in metres, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_m: Option<f64>,
}

#[derive(Deserialize)]
struct IndexFile {
    entries: Vec<Entry>,
}

/// The curated index plus the generated LUTs.
pub fn index() -> &'static [Entry] {
    static I: OnceLock<Vec<Entry>> = OnceLock::new();
    I.get_or_init(|| {
        let mut v: Vec<Entry> = serde_json::from_str::<IndexFile>(include_str!("../../assets/index.json")).map(|f| f.entries).unwrap_or_default();
        for (name, desc, tags) in lut::LOOKS {
            v.push(Entry {
                id: format!("anikit:lut/{name}"),
                kind: "lut".into(),
                name: name.to_string(),
                desc: desc.to_string(),
                tags: tags.split(", ").map(str::to_string).collect(),
                by: "anikit".into(),
                page: String::new(),
                version: "1".into(),
                license: "CC0-1.0".into(),
                license_url: "https://creativecommons.org/publicdomain/zero/1.0/".into(),
                default: "33".into(),
                res: BTreeMap::new(),
                size_m: None,
            });
        }
        v
    })
}

/// Whether `src` names a library asset (`provider:…`) rather than a file.
pub fn is_ref(src: &str) -> bool {
    src.split_once(':').is_some_and(|(p, rest)| PROVIDERS.contains(&p) && !rest.is_empty() && !rest.starts_with("//"))
}

/// `provider:rest@res` → (provider, rest, res).
fn split_ref(src: &str) -> Result<(&str, &str, Option<&str>), String> {
    let (p, rest) = src.split_once(':').ok_or_else(|| format!("`{src}` is not an asset ref (provider:name)"))?;
    let (rest, res) = match rest.rsplit_once('@') {
        Some((a, b)) => (a, Some(b)),
        None => (rest, None),
    };
    Ok((p, rest, res))
}

/// Find an index entry by ref; `polyhaven:kloppenheim_06` (no type) and case differences are fine.
pub fn find(src: &str) -> Option<&'static Entry> {
    let (p, rest, _) = split_ref(src).ok()?;
    let want = format!("{p}:{rest}").to_lowercase();
    let idx = index();
    idx.iter().find(|e| e.id.to_lowercase() == want).or_else(|| {
        let tail = rest.rsplit('/').next().unwrap_or(rest).to_lowercase();
        let ty = rest.split_once('/').map(|(t, _)| t);
        idx.iter().find(|e| {
            e.id.starts_with(&format!("{p}:")) && e.id.rsplit(['/', ':']).next().is_some_and(|n| n.to_lowercase() == tail) && ty.is_none_or(|t| e.kind == t || (t == "image" && e.kind == "texture"))
        })
    })
}

/// A fetched asset: its folder and the files in it.
#[derive(Clone, Debug)]
pub struct Fetched {
    pub entry: Entry,
    pub res: String,
    pub dir: PathBuf,
    /// `(role, path)`.
    pub files: Vec<(Option<String>, PathBuf)>,
}

impl Fetched {
    /// The file to open: the `main` role, else the albedo map, else the first file.
    pub fn main(&self) -> PathBuf {
        self.map("main").or_else(|| self.map("albedo")).map(Path::to_path_buf).or_else(|| self.files.first().map(|f| f.1.clone())).unwrap_or_else(|| self.dir.clone())
    }

    /// A PBR map (`albedo normal roughness ao displacement metalness opacity`).
    pub fn map(&self, role: &str) -> Option<&Path> {
        self.files.iter().find(|f| f.0.as_deref() == Some(role)).map(|f| f.1.as_path())
    }
}

/// How files are downloaded: `fetch(url, dest)`. The default runs `curl`; tests swap in a mock.
pub type Fetcher = dyn Fn(&str, &Path) -> Result<(), String> + Send + Sync;

/// A cache folder plus a way to fill it.
pub struct Library {
    pub root: PathBuf,
    fetch: Box<Fetcher>,
    pub offline: bool,
    /// Print "fetching …" lines to stderr.
    pub verbose: bool,
    /// Entries consulted before the index (tests, private catalogues).
    pub extra: Vec<Entry>,
}

/// The default cache folder: `$ANIKIT_ASSET_CACHE`, `$XDG_CACHE_HOME/anikit/assets`, or `~/.cache/anikit/assets`.
pub fn default_root() -> PathBuf {
    if let Some(p) = std::env::var_os("ANIKIT_ASSET_CACHE") {
        return PathBuf::from(p);
    }
    if let Some(p) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(p).join("anikit/assets");
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    home.join(".cache/anikit/assets")
}

/// Download with the system `curl` (follows redirects, retries, fails on HTTP errors).
pub fn curl_fetch(url: &str, dest: &Path) -> Result<(), String> {
    let out = std::process::Command::new("curl")
        .args(["-fsSL", "--retry", "3", "--retry-delay", "1", "--connect-timeout", "20", "-A", concat!("anikit-video/", env!("CARGO_PKG_VERSION")), "-o"])
        .arg(dest)
        .arg(url)
        .output()
        .map_err(|e| format!("can't run `curl` ({e}); install curl to download assets"))?;
    if out.status.success() {
        Ok(())
    } else {
        let _ = std::fs::remove_file(dest);
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// The process-wide library (default root, `curl`, `ANIKIT_OFFLINE`).
pub fn global() -> &'static Library {
    static L: OnceLock<Library> = OnceLock::new();
    L.get_or_init(|| Library {
        root: default_root(),
        fetch: Box::new(curl_fetch),
        offline: std::env::var("ANIKIT_OFFLINE").is_ok_and(|v| !v.is_empty() && v != "0"),
        verbose: true,
        extra: Vec::new(),
    })
}

/// Fetch (or find in the cache) an asset through the global library.
pub fn get(src: &str) -> Result<Fetched, String> {
    global().get(src)
}

/// The local file for `src`: library refs are fetched, anything else is joined to `base`.
pub fn resolve_path(base: &Path, src: &str) -> Result<PathBuf, String> {
    if is_ref(src) {
        get(src).map(|f| f.main())
    } else {
        Ok(base.join(src))
    }
}

/// The PBR map role a file name stands for (ambientCG and Poly Haven naming).
pub fn role_of(name: &str) -> Option<&'static str> {
    let n = name.rsplit('/').next().unwrap_or(name).to_lowercase();
    if !(n.ends_with(".jpg") || n.ends_with(".jpeg") || n.ends_with(".png") || n.ends_with(".exr")) || n.contains("preview") || n.contains("normaldx") || n.contains("nor_dx") {
        return None;
    }
    let stem = n.rsplit_once('.').map(|x| x.0).unwrap_or(&n);
    const R: &[(&str, &str)] = &[
        ("_color", "albedo"), ("_diff", "albedo"), ("_basecolor", "albedo"), ("_albedo", "albedo"), ("_normalgl", "normal"), ("_nor_gl", "normal"), ("_roughness", "roughness"), ("_rough", "roughness"),
        ("_ambientocclusion", "ao"), ("_ao", "ao"), ("_displacement", "displacement"), ("_disp", "displacement"), ("_metalness", "metalness"), ("_metal", "metalness"), ("_opacity", "opacity"),
    ];
    R.iter().find(|(s, _)| stem.contains(s)).map(|x| x.1)
}

const OK: &str = ".anikit-ok";

impl Library {
    pub fn new(root: PathBuf, fetch: Box<Fetcher>) -> Library {
        Library { root, fetch, offline: false, verbose: false, extra: Vec::new() }
    }

    /// An entry for `src`: from the index, or looked up live (Poly Haven / ambientCG).
    pub fn entry(&self, src: &str) -> Result<Entry, String> {
        let bare = src.rsplit_once('@').map(|x| x.0).unwrap_or(src);
        if let Some(e) = self.extra.iter().find(|e| e.id == bare) {
            return Ok(e.clone());
        }
        if let Some(e) = find(src) {
            return Ok(e.clone());
        }
        let (p, rest, _) = split_ref(src)?;
        let name = rest.rsplit('/').next().unwrap_or(rest);
        let hint = || {
            let near = search(name, None, 3);
            if near.is_empty() { String::new() } else { format!(" Close matches: {}.", near.iter().map(|e| e.id.as_str()).collect::<Vec<_>>().join(", ")) }
        };
        match p {
            "polyhaven" if !self.offline => self.live_polyhaven(name).map_err(|e| format!("asset `{src}`: {e}.{}", hint())),
            "ambientcg" if !self.offline => self.live_ambientcg(name).map_err(|e| format!("asset `{src}`: {e}.{}", hint())),
            "anikit" => Err(format!("unknown asset `{src}`; anikit provides LUTs: {}", lut::LOOKS.iter().map(|l| format!("anikit:lut/{}", l.0)).collect::<Vec<_>>().join(", "))),
            _ => Err(format!("unknown asset `{src}` (not in the index{}).{} Search with `anikit-video assets search <words>`.", if self.offline { "; offline, so no live lookup" } else { "" }, hint())),
        }
    }

    fn json(&self, url: &str) -> Result<serde_json::Value, String> {
        let tmp = self.root.join(format!(".tmp-{}-{}.json", std::process::id(), hash_str(url)));
        std::fs::create_dir_all(&self.root).map_err(|e| format!("{}: {e}", self.root.display()))?;
        (self.fetch)(url, &tmp).map_err(|e| offline_hint(url, &e))?;
        let s = std::fs::read_to_string(&tmp).map_err(|e| e.to_string());
        let _ = std::fs::remove_file(&tmp);
        serde_json::from_str(&s?).map_err(|e| format!("{url}: {e}"))
    }

    /// Any Poly Haven asset by its slug, via the public API (md5-verified).
    fn live_polyhaven(&self, slug: &str) -> Result<Entry, String> {
        let info = self.json(&format!("https://api.polyhaven.com/info/{slug}"))?;
        let files = self.json(&format!("https://api.polyhaven.com/files/{slug}"))?;
        let name = info["name"].as_str().ok_or("not found on Poly Haven")?.to_string();
        let fr = |path: String, v: &serde_json::Value, role: Option<&str>| FileRef {
            path,
            url: v["url"].as_str().unwrap_or_default().to_string(),
            size: v["size"].as_u64(),
            md5: v["md5"].as_str().map(str::to_string),
            sha256: None,
            role: role.map(str::to_string),
            unzip: false,
        };
        let mut res = BTreeMap::new();
        let (kind, default) = match info["type"].as_u64() {
            Some(0) => {
                for r in ["1k", "2k", "4k"] {
                    let h = &files["hdri"][r]["hdr"];
                    if h.is_object() {
                        res.insert(r.to_string(), vec![fr(format!("{slug}_{r}.hdr"), h, Some("main"))]);
                    }
                }
                ("hdri", "1k")
            }
            Some(1) => {
                for r in ["1k", "2k"] {
                    let mut v = Vec::new();
                    for (role, key) in [("albedo", "Diffuse"), ("normal", "nor_gl"), ("roughness", "Rough"), ("ao", "AO"), ("displacement", "Displacement"), ("metalness", "Metal")] {
                        let h = &files[key][r]["jpg"];
                        if h.is_object() {
                            v.push(fr(format!("{slug}_{role}_{r}.jpg"), h, Some(role)));
                        }
                    }
                    if !v.is_empty() {
                        res.insert(r.to_string(), v);
                    }
                }
                ("texture", "2k")
            }
            Some(2) => {
                for r in ["1k", "2k"] {
                    let g = &files["gltf"][r]["gltf"];
                    if !g.is_object() {
                        continue;
                    }
                    let main = g["url"].as_str().unwrap_or_default().rsplit('/').next().unwrap_or("model.gltf").to_string();
                    let mut v = vec![fr(main, g, Some("main"))];
                    if let Some(inc) = g["include"].as_object() {
                        for (p, h) in inc {
                            v.push(fr(p.clone(), h, None));
                        }
                    }
                    // Cutout maps the glTF doesn't reference (see gltf.rs `sibling_alpha`).
                    if let Some(all) = files.as_object() {
                        for (key, m) in all.iter().filter(|(k, _)| k.to_lowercase().ends_with("alpha")) {
                            let h = if m[r]["png"].is_object() { &m[r]["png"] } else { &m[r]["jpg"] };
                            if let Some(ext) = h["url"].as_str().and_then(|u| u.rsplit_once('.')).map(|x| x.1) {
                                v.push(fr(format!("textures/{slug}_{key}_{r}.{ext}"), h, None));
                            }
                        }
                    }
                    res.insert(r.to_string(), v);
                }
                ("model", "1k")
            }
            _ => return Err("unknown Poly Haven asset type".into()),
        };
        Ok(Entry {
            id: format!("polyhaven:{kind}/{slug}"),
            kind: kind.into(),
            desc: info["description"].as_str().unwrap_or(&name).to_string(),
            name,
            tags: info["tags"].as_array().map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            by: info["authors"].as_object().map(|a| a.keys().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_default(),
            page: format!("https://polyhaven.com/a/{slug}"),
            version: info["files_hash"].as_str().unwrap_or_default().chars().take(12).collect(),
            license: "CC0-1.0".into(),
            license_url: "https://polyhaven.com/license".into(),
            default: default.into(),
            res,
            size_m: info["dimensions"][0].as_f64().map(|mm| mm / 1000.0),
        })
    }

    /// Any ambientCG material by id (the zip's sha256 is pinned on first download).
    fn live_ambientcg(&self, id: &str) -> Result<Entry, String> {
        let d = self.json(&format!("https://ambientcg.com/api/v2/full_json?id={id}&include=downloadData,tagData,displayData"))?;
        let a = d["foundAssets"].get(0).ok_or("not found on ambientCG")?;
        let mut res = BTreeMap::new();
        if let Some(dl) = a["downloadFolders"]["default"]["downloadFiletypeCategories"]["zip"]["downloads"].as_array() {
            for (r, attr) in [("1k", "1K-JPG"), ("2k", "2K-JPG")] {
                if let Some(x) = dl.iter().find(|x| x["attribute"] == attr) {
                    res.insert(r.to_string(), vec![FileRef { path: x["fileName"].as_str().unwrap_or("x.zip").into(), url: x["downloadLink"].as_str().unwrap_or_default().into(), size: x["size"].as_u64(), md5: None, sha256: None, role: None, unzip: true }]);
                }
            }
        }
        if res.is_empty() {
            return Err("no JPG download on ambientCG (not a material?)".into());
        }
        let real = a["assetId"].as_str().unwrap_or(id).to_string();
        Ok(Entry {
            id: format!("ambientcg:{real}"),
            kind: "texture".into(),
            name: a["displayName"].as_str().unwrap_or(&real).to_string(),
            desc: a["displayName"].as_str().unwrap_or(&real).to_string(),
            tags: a["tags"].as_array().map(|t| t.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            by: "ambientCG".into(),
            page: format!("https://ambientcg.com/a/{real}"),
            version: a["releaseDate"].as_str().unwrap_or_default().chars().take(10).collect(),
            license: "CC0-1.0".into(),
            license_url: "https://docs.ambientcg.com/license/".into(),
            default: "2k".into(),
            res,
            size_m: a["dimensionX"].as_f64().filter(|x| *x > 0.0).map(|cm| cm / 100.0),
        })
    }

    /// The cache folder for an entry at a resolution.
    pub fn dir_for(&self, e: &Entry, res: &str) -> PathBuf {
        let (p, rest) = e.id.split_once(':').unwrap_or(("misc", &e.id));
        let safe: String = rest.chars().map(|c| if c.is_ascii_alphanumeric() || "-_./".contains(c) { c } else { '_' }).collect();
        self.root.join(p).join(safe.replace("..", "_")).join(res)
    }

    /// Fetch `src` (cached after the first time).
    pub fn get(&self, src: &str) -> Result<Fetched, String> {
        let (p, rest, want_res) = split_ref(src)?;
        if p == "anikit" {
            return self.get_generated(src, rest);
        }
        let e = self.entry(src)?;
        let res = match want_res {
            Some(r) if e.res.contains_key(r) => r.to_string(),
            Some(r) => return Err(format!("asset `{}` has no `{r}` resolution (has: {})", e.id, e.res.keys().cloned().collect::<Vec<_>>().join(", "))),
            None => e.default.clone(),
        };
        let files = e.res.get(&res).ok_or_else(|| format!("asset `{}` has no files", e.id))?.clone();
        let dir = self.dir_for(&e, &res);
        let marker = dir.join(OK);
        if let Ok(m) = std::fs::read_to_string(&marker) {
            // Same version, and every pinned plain file still there (an index
            // update can add a file without a new upstream version).
            let complete = files.iter().all(|f| f.unzip || dir.join(&f.path).exists());
            if let (true, Ok(f)) = (complete, self.cached_files(&e, &res, &dir, &m)) {
                return Ok(f);
            }
        }
        if self.offline {
            return Err(format!(
                "asset `{}` isn't in the cache ({}) and ANIKIT_OFFLINE is set. Fetch it once online with `anikit-video assets get {}`, or copy a cache folder there.",
                e.id,
                self.root.display(),
                e.id
            ));
        }
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        if self.verbose {
            let total: u64 = files.iter().filter_map(|f| f.size).sum();
            eprintln!("anikit: fetching {} ({res}, {:.1} MB, {}) → {}", e.id, total as f64 / 1e6, e.license, dir.display());
        }
        let mut got: Vec<(Option<String>, PathBuf)> = Vec::new();
        let mut pins: Vec<serde_json::Value> = Vec::new();
        for f in &files {
            let rel = sanitize(&f.path)?;
            let dest = dir.join(&rel);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            let tmp = dest.with_extension(format!("part{}", std::process::id()));
            (self.fetch)(&f.url, &tmp).map_err(|err| format!("asset `{}`: {}", e.id, offline_hint(&f.url, &err)))?;
            let (algo, want) = match (&f.sha256, &f.md5) {
                (Some(s), _) => ("sha256", Some(s.clone())),
                (None, Some(m)) => ("md5", Some(m.clone())),
                _ => ("sha256", None),
            };
            let have = hash::file_hex(&tmp, algo).map_err(|e| format!("{}: {e}", tmp.display()))?;
            if let Some(w) = &want {
                if !w.eq_ignore_ascii_case(&have) {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(format!("asset `{}`: checksum mismatch for {} ({algo} {have}, pinned {w}); the upstream file changed or the download was corrupted", e.id, f.url));
                }
            }
            if let (Some(sz), Ok(md)) = (f.size, std::fs::metadata(&tmp)) {
                if md.len() != sz && want.is_none() {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(format!("asset `{}`: {} is {} bytes, expected {sz}", e.id, f.url, md.len()));
                }
            }
            pins.push(serde_json::json!({ "url": f.url, algo: have }));
            if f.unzip {
                let data = std::fs::read(&tmp).map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(&tmp);
                for (name, bytes) in zip::extract(&data, |n| role_of(n).is_some())? {
                    let leaf = sanitize(name.rsplit('/').next().unwrap_or(&name))?;
                    let out = dir.join(&leaf);
                    std::fs::write(&out, bytes).map_err(|e| format!("{}: {e}", out.display()))?;
                    got.push((role_of(&leaf.to_string_lossy()).map(str::to_string), out));
                }
            } else {
                std::fs::rename(&tmp, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
                got.push((f.role.clone().or_else(|| role_of(&f.path).map(str::to_string)), dest));
            }
        }
        got.sort_by(|a, b| a.1.cmp(&b.1));
        let m = serde_json::json!({
            "id": e.id, "version": e.version, "res": res, "license": e.license, "by": e.by, "page": e.page,
            "pins": pins,
            "files": got.iter().map(|(r, p)| serde_json::json!({ "role": r, "path": p.strip_prefix(&dir).unwrap_or(p).to_string_lossy() })).collect::<Vec<_>>(),
        });
        std::fs::write(&marker, serde_json::to_string_pretty(&m).unwrap_or_default()).map_err(|e| format!("{}: {e}", marker.display()))?;
        Ok(Fetched { entry: e, res, dir, files: got })
    }

    fn cached_files(&self, e: &Entry, res: &str, dir: &Path, m: &str) -> Result<Fetched, ()> {
        let v: serde_json::Value = serde_json::from_str(m).map_err(|_| ())?;
        if v["version"].as_str() != Some(e.version.as_str()) {
            return Err(());
        }
        let mut files = Vec::new();
        for f in v["files"].as_array().ok_or(())? {
            let p = dir.join(f["path"].as_str().ok_or(())?);
            if !p.exists() {
                return Err(());
            }
            files.push((f["role"].as_str().map(str::to_string), p));
        }
        Ok(Fetched { entry: e.clone(), res: res.to_string(), dir: dir.to_path_buf(), files })
    }

    /// `anikit:lut/<name>`: written to the cache as a `.cube` (no network).
    fn get_generated(&self, src: &str, rest: &str) -> Result<Fetched, String> {
        let e = find(src).cloned().ok_or_else(|| self.entry(src).err().unwrap_or_default())?;
        let name = rest.strip_prefix("lut/").unwrap_or(rest);
        let l = lut::generated(name).ok_or_else(|| format!("unknown LUT `{src}`"))?;
        let dir = self.dir_for(&e, "33");
        let p = dir.join(format!("{name}.cube"));
        if !p.exists() {
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            std::fs::write(&p, l.to_cube()).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        Ok(Fetched { entry: e, res: "33".into(), dir, files: vec![(Some("main".into()), p)] })
    }
}

fn sanitize(p: &str) -> Result<PathBuf, String> {
    let path = Path::new(p);
    if path.is_absolute() || path.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return Err(format!("unsafe file path in asset: {p}"));
    }
    Ok(path.to_path_buf())
}

fn hash_str(s: &str) -> String {
    let mut h = hash::Md5::default();
    h.update(s.as_bytes());
    h.hex()[..12].to_string()
}

fn offline_hint(url: &str, err: &str) -> String {
    let host = url.split("//").nth(1).and_then(|r| r.split('/').next()).unwrap_or(url);
    let net = ["resolve host", "Could not connect", "timed out", "Failed to connect", "Connection refused", "Network is unreachable", "SSL", "proxy", "403"].iter().any(|s| err.contains(s));
    if net {
        format!("can't download from {host} ({err}). Looks like no network access: run `anikit-video assets get <ref>` once online (files are cached in {}), or use a local file", default_root().display())
    } else {
        format!("download from {url} failed: {err}")
    }
}

// ------------------------------------------------------------ search

/// Best matches for `query` (words; all must match somewhere), optionally of one type.
pub fn search(query: &str, kind: Option<&str>, limit: usize) -> Vec<&'static Entry> {
    let words: Vec<String> = query.to_lowercase().split(|c: char| c.is_whitespace() || c == ',').filter(|w| !w.is_empty()).map(str::to_string).collect();
    let mut scored: Vec<(f32, &Entry)> = index()
        .iter()
        .filter(|e| match kind {
            None => true,
            Some("image") => e.kind == "texture" || e.kind == "image",
            Some("material") => e.kind == "texture",
            Some(k) => e.kind == k,
        })
        .filter_map(|e| {
            let name = e.name.to_lowercase();
            let id = e.id.to_lowercase();
            let desc = e.desc.to_lowercase();
            let mut s = 0.0;
            for w in &words {
                let stem = w.trim_end_matches('s');
                let mut best: f32 = 0.0;
                for t in &e.tags {
                    let t = t.to_lowercase();
                    if t == *w || t == stem {
                        best = best.max(3.0);
                    } else if t.contains(stem) {
                        best = best.max(1.5);
                    }
                }
                if name.contains(stem) || id.contains(stem) {
                    best = best.max(2.5);
                }
                if desc.contains(stem) {
                    best = best.max(1.2);
                }
                if e.kind == *w || (w == "material" && e.kind == "texture") {
                    best = best.max(2.0);
                }
                if best == 0.0 {
                    return None;
                }
                s += best;
            }
            Some((s, e))
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.1.id.cmp(&b.1.id)));
    scored.into_iter().take(limit).map(|x| x.1).collect()
}

// ------------------------------------------------------------ CLI

pub const DOCS: &str = include_str!("../../docs-cli/assets.md");

fn line(e: &Entry) -> String {
    let size = e.res.get(&e.default).map(|f| f.iter().filter_map(|x| x.size).sum::<u64>()).unwrap_or(0);
    let sz = if size > 0 { format!(" {:.0} MB", (size as f64 / 1e6).max(1.0)) } else { String::new() };
    format!("{:<44} {:<7} {}  [{}]{}", e.id, e.kind, e.desc, e.tags.iter().take(6).cloned().collect::<Vec<_>>().join(", "), sz)
}

/// `anikit-video assets …` (and `docs assets`). `None` if `args` isn't for us.
pub fn run(args: &[String]) -> Option<i32> {
    match (args.first().map(String::as_str), args.get(1).map(String::as_str)) {
        (Some("docs"), Some("assets" | "asset" | "hdri" | "hdris" | "materials" | "lut" | "luts")) => {
            print!("{DOCS}");
            Some(0)
        }
        (Some("assets" | "asset"), _) => Some(match cmd(&args[1..]) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("anikit-video assets: {e}");
                1
            }
        }),
        _ => None,
    }
}

fn cmd(args: &[String]) -> Result<(), String> {
    let mut pos: Vec<&str> = Vec::new();
    let (mut kind, mut limit, mut json, mut res, mut out) = (None, 12usize, false, None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--type" | "-t" => kind = Some(it.next().ok_or("--type needs a value")?.clone()),
            "--limit" | "-n" => limit = it.next().and_then(|x| x.parse().ok()).ok_or("--limit needs a number")?,
            "--res" => res = Some(it.next().ok_or("--res needs a value (1k, 2k, 4k)")?.clone()),
            "-o" | "--out" => out = Some(it.next().ok_or("-o needs a path")?.clone()),
            "--json" => json = true,
            s if s.starts_with("--type=") => kind = Some(s[7..].to_string()),
            s if s.starts_with('-') => return Err(format!("unknown option `{s}`")),
            s => pos.push(s),
        }
    }
    if let Some(k) = &kind {
        let k = k.trim_end_matches('s');
        if !TYPES.contains(&k) && k != "material" {
            return Err(format!("unknown type `{k}` (types: {})", TYPES.join(", ")));
        }
        kind = Some(k.to_string());
    }
    let with_res = |r: &str| match &res {
        Some(x) if !r.contains('@') => format!("{r}@{x}"),
        _ => r.to_string(),
    };
    match pos.first().copied() {
        Some("search" | "find") => {
            let q = pos[1..].join(" ");
            let hits = if q.is_empty() { index().iter().filter(|e| kind.as_deref().is_none_or(|k| e.kind == k)).take(limit).collect() } else { search(&q, kind.as_deref(), limit) };
            if json {
                println!("{}", serde_json::to_string(&hits.iter().map(|e| serde_json::json!({ "id": e.id, "type": e.kind, "desc": e.desc, "tags": e.tags, "license": e.license })).collect::<Vec<_>>()).unwrap_or_default());
            } else if hits.is_empty() {
                println!("no matches for `{q}`. Try fewer or broader words (e.g. `sunset`, `studio`, `wood`, `metal`, `plant`), or --type.");
            } else {
                for e in hits {
                    println!("{}", line(e));
                }
            }
            Ok(())
        }
        Some("list" | "ls") => {
            let mut n: BTreeMap<&str, usize> = BTreeMap::new();
            for e in index() {
                *n.entry(e.kind.as_str()).or_default() += 1;
            }
            if kind.is_none() && !json {
                println!("{} assets: {}", index().len(), n.iter().map(|(k, v)| format!("{v} {k}")).collect::<Vec<_>>().join(", "));
                println!("`anikit-video assets list --type hdri` lists one type; `assets search <words>` finds by tag/description.");
                return Ok(());
            }
            let all: Vec<&Entry> = index().iter().filter(|e| kind.as_deref().is_none_or(|k| e.kind == k || (k == "image" && e.kind == "texture"))).collect();
            if json {
                println!("{}", serde_json::to_string(&all).unwrap_or_default());
            } else {
                for e in all {
                    println!("{}", line(e));
                }
            }
            Ok(())
        }
        Some("info") => {
            let r = pos.get(1).ok_or("usage: assets info <ref>")?;
            let e = global().entry(r)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&e).unwrap_or_default());
            } else {
                println!("{}  ({})\n{}\ntags: {}\nlicence: {} {}\nby: {}\npage: {}\nversion: {}", e.id, e.kind, e.desc, e.tags.join(", "), e.license, e.license_url, e.by, e.page, e.version);
                for (r, fs) in &e.res {
                    println!("  {r}{}: {} file(s), {:.1} MB", if *r == e.default { " (default)" } else { "" }, fs.len(), fs.iter().filter_map(|f| f.size).sum::<u64>() as f64 / 1e6);
                }
            }
            Ok(())
        }
        Some("get" | "fetch") => {
            if pos.len() < 2 {
                return Err("usage: assets get <ref…> [--res 2k]".into());
            }
            for r in &pos[1..] {
                let f = global().get(&with_res(r))?;
                if json {
                    println!("{}", serde_json::json!({ "id": f.entry.id, "res": f.res, "dir": f.dir, "main": f.main(), "files": f.files.iter().map(|(r, p)| serde_json::json!({"role": r, "path": p})).collect::<Vec<_>>() }));
                } else {
                    println!("{}  {}", f.entry.id, f.main().display());
                    for (r, p) in &f.files {
                        if let Some(r) = r.as_deref().filter(|r| *r != "main") {
                            println!("  {r:<13}{}", p.display());
                        }
                    }
                }
            }
            Ok(())
        }
        Some("lut") => {
            let r = pos.get(1).ok_or("usage: assets lut <name> [-o look.cube]")?;
            let name = r.strip_prefix("anikit:lut/").unwrap_or(r);
            let l = lut::generated(name).ok_or_else(|| format!("unknown LUT `{r}`; `assets list --type lut`"))?;
            match out {
                Some(o) => std::fs::write(&o, l.to_cube()).map_err(|e| format!("{o}: {e}"))?,
                None => print!("{}", l.to_cube()),
            }
            Ok(())
        }
        Some("credits" | "licenses" | "licences") => {
            let p = pos.get(1).ok_or("usage: assets credits <spec.json>")?;
            let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
            let refs = refs_in(&text);
            if refs.is_empty() {
                println!("no library assets in {p}");
            }
            for r in refs {
                match global().entry(&r) {
                    Ok(e) => println!("{} by {} ({}, {}) {}", e.name, e.by, e.license, e.id, e.page),
                    Err(err) => println!("{r}: {err}"),
                }
            }
            Ok(())
        }
        Some("path" | "cache") => {
            println!("{}", global().root.display());
            Ok(())
        }
        Some(x) => Err(format!("unknown subcommand `{x}`. Use: search, list, info, get, lut, credits, path (`anikit-video docs assets`)")),
        None => {
            print!("{DOCS}");
            Ok(())
        }
    }
}

/// Every `provider:…` string in a spec's text (for credits).
pub fn refs_in(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in text.split('"') {
        if is_ref(part) && !part.contains(char::is_whitespace) && !out.iter().any(|x| x == part) {
            out.push(part.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests;
