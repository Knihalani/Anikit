use anikit_video::text::*;

fn store() -> &'static FontStore {
    static S: std::sync::OnceLock<FontStore> = std::sync::OnceLock::new();
    S.get_or_init(FontStore::new)
}

#[test]
fn families_and_basic_layout() {
    let s = store();
    let names = s.family_names();
    eprintln!("{names:?}");
    assert!(s.resolve_family("inter display").is_some());
    let l = s.layout(&TextSpec::plain("Hello world", TextStyle::default()));
    eprintln!("w {} h {} runs {} chars {} words {} lines {}", l.width, l.height, l.runs.len(), l.chars.len(), l.words.len(), l.lines.len());
    eprintln!("{:?}", l.words);
    assert_eq!(l.chars.len(), 10);
    assert_eq!(l.words.len(), 2);
    assert_eq!(l.lines.len(), 1);
    assert!(l.glyphs.iter().all(|g| l.runs[g.run as usize].glyphs[g.index as usize].id != 0));
}
