use anikit_video::layout::*;
use anikit_video::text::FontStore;

fn node(json: &str) -> BoxNode {
    serde_json::from_str(json).unwrap()
}

#[test]
fn flex_column_with_text_and_grid() {
    let fonts = FontStore::new();
    let tree = node(r#"{
      "style": { "display": "flex", "flex-direction": "column", "padding": "60px 80px", "gap": 40 },
      "children": [
        { "id": "title", "text": { "text": "Three things agents love about rendering native video in Rust", "style": { "size": 72, "weight": 800 } } },
        { "id": "grid", "style": { "display": "grid", "gridTemplateColumns": "repeat(3, 1fr)", "gap": 24, "flexGrow": 1 },
          "children": [ { "id": "a" }, { "id": "b", "style": { "gridColumn": "span 2" } }, { "id": "c" } ] },
        { "id": "footer", "style": { "height": 80, "justifyContent": "space-between", "alignItems": "center" },
          "children": [ { "id": "l", "style": { "width": 200, "height": 40 } }, { "id": "r", "style": { "width": "25%", "height": 40 } } ] }
      ]
    }"#);
    let r = layout_boxes(&tree, &fonts, [1920.0, 1080.0]).unwrap();
    let get = |id| find(&r, id).unwrap();
    assert_eq!(r[0].rect, [0.0, 0.0, 1920.0, 1080.0]);
    let title = get("title");
    assert_eq!(title.rect[0], 80.0);
    assert_eq!(title.rect[1], 60.0);
    assert_eq!(title.rect[2], 1760.0);
    // 72px text, 1760 wide: the title wraps to 2 lines (~173 px tall).
    assert!(title.rect[3] > 150.0 && title.rect[3] < 200.0, "{:?}", title.rect);
    let footer = get("footer");
    assert_eq!(footer.rect[1] + footer.rect[3], 1080.0 - 60.0);
    let grid = get("grid");
    assert!((grid.rect[1] - (title.rect[1] + title.rect[3] + 40.0)).abs() < 0.6);
    assert!((grid.rect[3] - (footer.rect[1] - 40.0 - grid.rect[1])).abs() < 0.6);
    let (a, b, c) = (get("a"), get("b"), get("c"));
    let col = (1760.0 - 48.0) / 3.0;
    assert!((a.rect[2] - col).abs() < 1.0);
    assert!((b.rect[2] - (2.0 * col + 24.0)).abs() < 1.0);
    assert!(c.rect[1] > a.rect[1], "c wraps to row 2");
    let (l, rr) = (get("l"), get("r"));
    eprintln!("footer {:?} l {:?} r {:?}", footer.rect, l.rect, rr.rect);
    assert_eq!(l.rect[0], 80.0);
    assert_eq!(rr.rect[0] + rr.rect[2], 1840.0);
    assert_eq!(rr.rect[2], 440.0);
    assert_eq!(l.rect[1], footer.rect[1] + 20.0);
    assert_eq!(l.parent, Some(r.iter().position(|x| x.id.as_deref() == Some("footer")).unwrap()));
}

#[test]
fn style_errors_are_helpful() {
    let fonts = FontStore::new();
    let tree = node(r#"{ "style": { "flexDirecton": "column", "width": "12qq", "gridTemplateColumns": "1fr 2fr" },
                         "children": [ { "style": { "justify-content": "middle" } } ] }"#);
    let e = layout_boxes(&tree, &fonts, [100.0, 100.0]).unwrap_err();
    eprintln!("{e:#?}");
    assert_eq!(e.len(), 3);
    assert!(e.iter().any(|m| m.contains("did you mean \"flexDirection\"")));
    assert!(e.iter().any(|m| m.contains("root.style.width")));
    assert!(e.iter().any(|m| m.contains("root.children[0].style.justify-content")));
}
