//! Asset library tests; the network is mocked (a URL → bytes table).

use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("anikit-assets-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

struct Mock {
    files: Mutex<HashMap<String, Vec<u8>>>,
    calls: AtomicUsize,
}

fn lib(tag: &str, mock: Arc<Mock>) -> Library {
    let m = mock.clone();
    Library::new(
        tmpdir(tag),
        Box::new(move |url: &str, dest: &Path| {
            m.calls.fetch_add(1, Ordering::SeqCst);
            match m.files.lock().unwrap().get(url) {
                Some(b) => std::fs::write(dest, b).map_err(|e| e.to_string()),
                None => Err("curl: (6) Could not resolve host: example.test".into()),
            }
        }),
    )
}

fn md5(b: &[u8]) -> String {
    let mut h = hash::Md5::default();
    h.update(b);
    h.hex()
}

fn sha(b: &[u8]) -> String {
    let mut h = hash::Sha256::default();
    h.update(b);
    h.hex()
}

fn entry(id: &str, kind: &str, files: Vec<FileRef>) -> Entry {
    Entry {
        id: id.into(),
        kind: kind.into(),
        name: id.into(),
        desc: "test".into(),
        tags: vec![],
        by: "me".into(),
        page: String::new(),
        version: "v1".into(),
        license: "CC0-1.0".into(),
        license_url: String::new(),
        default: "1k".into(),
        res: [("1k".to_string(), files)].into_iter().collect(),
        size_m: None,
    }
}

#[test]
fn index_is_curated_pinned_and_cc0() {
    let idx = index();
    assert!(idx.len() >= 200, "starter index has {} entries", idx.len());
    let mut ids = std::collections::HashSet::new();
    let mut by_kind: HashMap<&str, usize> = HashMap::new();
    for e in idx {
        assert!(ids.insert(e.id.as_str()), "duplicate id {}", e.id);
        assert_eq!(e.license, "CC0-1.0", "{}", e.id);
        assert!(!e.desc.is_empty() && e.desc.len() <= 160, "{}: description", e.id);
        assert!(is_ref(&e.id), "{} is not a ref", e.id);
        *by_kind.entry(e.kind.as_str()).or_default() += 1;
        if e.kind != "lut" {
            assert!(e.res.contains_key(&e.default), "{}: default res", e.id);
            if e.kind == "texture" && !e.id.starts_with("ambientcg:") {
                assert!(e.res.values().all(|fs| fs.iter().any(|f| f.role.as_deref() == Some("albedo"))), "{}: no albedo map", e.id);
            }
            for f in e.res.values().flatten() {
                assert!(f.md5.is_some() || f.sha256.is_some(), "{}: {} is not pinned", e.id, f.url);
                assert!(f.url.starts_with("https://"), "{}", f.url);
                sanitize(&f.path).unwrap();
            }
        }
    }
    for k in ["hdri", "texture", "model", "lut"] {
        assert!(by_kind.get(k).copied().unwrap_or(0) >= 10, "few {k}: {by_kind:?}");
    }
}

#[test]
fn refs_and_lookup() {
    assert!(is_ref("polyhaven:hdri/kloppenheim_06"));
    assert!(is_ref("ambientcg:Wood049"));
    assert!(!is_ref("C:\\pics\\a.png"));
    assert!(!is_ref("https://example.com/a.png"));
    assert!(!is_ref("images/a.png"));
    // Short forms and case.
    let e = find("polyhaven:studio_small_09").expect("studio_small_09 in the index");
    assert_eq!(e.id, "polyhaven:hdri/studio_small_09");
    assert_eq!(find("POLYHAVEN:hdri/Studio_Small_09@2k").unwrap().id, e.id);
    assert!(find("anikit:lut/teal-orange").is_some());
    assert_eq!(refs_in(r#"{"env":{"hdri":"polyhaven:hdri/x"},"src":"a.png","m":"ambientcg:Wood049","n":"polyhaven:hdri/x"}"#), vec!["polyhaven:hdri/x", "ambientcg:Wood049"]);
}

#[test]
fn search_ranks_tags_and_filters_types() {
    let s = search("sunset", Some("hdri"), 5);
    assert!(!s.is_empty());
    assert!(s.iter().all(|e| e.kind == "hdri"));
    assert!(s[0].tags.iter().any(|t| t.contains("sunset")) || s[0].desc.to_lowercase().contains("sunset"));
    let w = search("wood", Some("texture"), 10);
    assert!(w.len() >= 5, "{w:?}");
    assert!(search("studio", None, 3).iter().any(|e| e.kind == "hdri"));
    assert!(!search("teal", Some("lut"), 3).is_empty());
    assert!(search("zzzqqq", None, 3).is_empty());
    // Plural and comma separated words.
    assert!(!search("plants, pot", Some("model"), 3).is_empty());
}

#[test]
fn fetch_verifies_caches_and_reports_offline() {
    let hdr = b"#?RADIANCE\nfake".to_vec();
    let mock = Arc::new(Mock { files: Mutex::new(HashMap::new()), calls: AtomicUsize::new(0) });
    mock.files.lock().unwrap().insert("https://example.test/a.hdr".into(), hdr.clone());
    let mut l = lib("fetch", mock.clone());
    l.extra.push(entry("polyhaven:hdri/fake", "hdri", vec![FileRef { path: "fake_1k.hdr".into(), url: "https://example.test/a.hdr".into(), size: Some(hdr.len() as u64), md5: Some(md5(&hdr)), sha256: None, role: Some("main".into()), unzip: false }]));
    let f = l.get("polyhaven:hdri/fake").unwrap();
    assert_eq!(std::fs::read(f.main()).unwrap(), hdr);
    assert_eq!(mock.calls.load(Ordering::SeqCst), 1);
    // Cached: no second download, even offline.
    l.offline = true;
    let g = l.get("polyhaven:hdri/fake").unwrap();
    assert_eq!(g.main(), f.main());
    assert_eq!(mock.calls.load(Ordering::SeqCst), 1);
    // Wrong resolution names the ones that exist.
    assert!(l.get("polyhaven:hdri/fake@8k").unwrap_err().contains("1k"));
    // Not cached + offline: a clear message, no network call.
    l.extra.push(entry("polyhaven:hdri/other", "hdri", vec![FileRef { path: "o.hdr".into(), url: "https://example.test/o.hdr".into(), size: None, md5: Some("00".into()), sha256: None, role: None, unzip: false }]));
    let e = l.get("polyhaven:hdri/other").unwrap_err();
    assert!(e.contains("ANIKIT_OFFLINE") && e.contains("assets get"), "{e}");
    assert_eq!(mock.calls.load(Ordering::SeqCst), 1);
    // Online but unreachable: says so.
    l.offline = false;
    let e = l.get("polyhaven:hdri/other").unwrap_err();
    assert!(e.contains("no network access"), "{e}");
    let _ = std::fs::remove_dir_all(&l.root);
}

#[test]
fn checksum_mismatch_is_rejected() {
    let mock = Arc::new(Mock { files: Mutex::new(HashMap::new()), calls: AtomicUsize::new(0) });
    mock.files.lock().unwrap().insert("https://example.test/t.jpg".into(), b"tampered".to_vec());
    let mut l = lib("mismatch", mock);
    l.extra.push(entry("ambientcg:Fake001", "texture", vec![FileRef { path: "t.jpg".into(), url: "https://example.test/t.jpg".into(), size: None, md5: None, sha256: Some(sha(b"original")), role: Some("albedo".into()), unzip: false }]));
    let e = l.get("ambientcg:Fake001").unwrap_err();
    assert!(e.contains("checksum mismatch"), "{e}");
    assert!(!l.dir_for(&l.extra[0], "1k").join(OK).exists());
    let _ = std::fs::remove_dir_all(&l.root);
}

#[test]
fn zips_unpack_into_pbr_roles() {
    let z = zip::make(&[
        ("Fake001_1K-JPG_Color.jpg", b"c"),
        ("Fake001_1K-JPG_NormalGL.jpg", b"n"),
        ("Fake001_1K-JPG_NormalDX.jpg", b"dx"),
        ("Fake001_1K-JPG_Roughness.jpg", b"r"),
        ("Fake001_1K-JPG_AmbientOcclusion.jpg", b"a"),
        ("Fake001_1K-JPG_Displacement.jpg", b"d"),
        ("Fake001.usda", b"u"),
    ]);
    let mock = Arc::new(Mock { files: Mutex::new(HashMap::new()), calls: AtomicUsize::new(0) });
    mock.files.lock().unwrap().insert("https://example.test/f.zip".into(), z.clone());
    let mut l = lib("zip", mock);
    l.extra.push(entry("ambientcg:Fake001", "texture", vec![FileRef { path: "f.zip".into(), url: "https://example.test/f.zip".into(), size: Some(z.len() as u64), md5: None, sha256: Some(sha(&z)), role: None, unzip: true }]));
    let f = l.get("ambientcg:Fake001").unwrap();
    for (role, body) in [("albedo", "c"), ("normal", "n"), ("roughness", "r"), ("ao", "a"), ("displacement", "d")] {
        assert_eq!(std::fs::read_to_string(f.map(role).unwrap()).unwrap(), body, "{role}");
    }
    assert_eq!(f.files.len(), 5, "NormalDX and .usda are skipped");
    assert_eq!(f.main(), f.map("albedo").unwrap());
    let _ = std::fs::remove_dir_all(&l.root);
}

#[test]
fn unsafe_paths_are_refused() {
    assert!(sanitize("../evil").is_err());
    assert!(sanitize("/etc/passwd").is_err());
    assert!(sanitize("textures/a.jpg").is_ok());
}

#[test]
fn generated_luts_need_no_network() {
    let mock = Arc::new(Mock { files: Mutex::new(HashMap::new()), calls: AtomicUsize::new(0) });
    let mut l = lib("lut", mock.clone());
    l.offline = true;
    let f = l.get("anikit:lut/teal-orange").unwrap();
    let cube = std::fs::read_to_string(f.main()).unwrap();
    assert!(cube.contains("LUT_3D_SIZE 33"));
    assert_eq!(mock.calls.load(Ordering::SeqCst), 0);
    let _ = std::fs::remove_dir_all(&l.root);
}

#[test]
fn hdri_moods_point_into_the_index() {
    for (mood, r, why) in HDRI_MOODS {
        assert!(find(r).is_some_and(|e| e.kind == "hdri"), "{mood} → {r} is not an indexed HDRI");
        assert!(!why.is_empty());
    }
    assert_eq!(hdri_mood("Night City"), Some("polyhaven:hdri/hansaplatz"));
    assert_eq!(hdri_mood("nope"), None);
}
