//! Box layout (taffy): flexbox, grid and block, described like CSS.
//!
//! A [`BoxNode`] tree carries CSS properties (camelCase or kebab-case keys,
//! CSS value syntax: `"1fr 2fr"`, `"repeat(3, 1fr)"`, `"50%"`, `24`, `"24px 48px"`),
//! optional text (measured through parley) and children. [`layout_boxes`]
//! returns one absolute rectangle per node in pre-order, once, at preprocessing.
//!
//! ```ignore
//! let tree: BoxNode = serde_json::from_str(r#"{
//!   "style": { "display": "flex", "flexDirection": "column", "gap": 24, "padding": 64 },
//!   "children": [ { "id": "title", "text": { "text": "Hello" } }, { "style": { "flexGrow": 1 } } ]
//! }"#)?;
//! let rects = layout_boxes(&tree, &fonts, [1920.0, 1080.0])?;
//! ```

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use taffy::prelude::*;
use taffy::{GridTemplateComponent, GridTemplateTracks};

use crate::text::{FontStore, TextSpec};

/// One box: CSS style, optional text content, children.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoxNode {
    /// Name to find this box's rectangle by.
    pub id: Option<String>,
    /// CSS properties (see module docs).
    pub style: Map<String, Value>,
    /// Text content: the box sizes to the text (wrapping at its available width).
    pub text: Option<TextSpec>,
    pub children: Vec<BoxNode>,
}

/// A laid-out box in absolute coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct BoxRect {
    pub id: Option<String>,
    /// Index of the parent in the output (`None` for the root).
    pub parent: Option<usize>,
    pub depth: u32,
    /// Border box `[x, y, w, h]`.
    pub rect: [f32; 4],
    /// Content box (inside padding and border) `[x, y, w, h]`: place text here,
    /// laid out with `width = content[2]`.
    pub content: [f32; 4],
}

/// Find a box by id.
pub fn find<'a>(rects: &'a [BoxRect], id: &str) -> Option<&'a BoxRect> {
    rects.iter().find(|r| r.id.as_deref() == Some(id))
}

const PROPS: &[&str] = &[
    "display", "position", "boxSizing", "overflow", "width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight",
    "aspectRatio", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "padding", "paddingTop",
    "paddingRight", "paddingBottom", "paddingLeft", "border", "inset", "top", "right", "bottom", "left", "gap",
    "rowGap", "columnGap", "flexDirection", "flexWrap", "flexGrow", "flexShrink", "flexBasis", "flex", "alignItems",
    "alignSelf", "alignContent", "justifyItems", "justifySelf", "justifyContent", "gridTemplateColumns",
    "gridTemplateRows", "gridAutoColumns", "gridAutoRows", "gridAutoFlow", "gridColumn", "gridRow", "textAlign",
];

fn camel(k: &str) -> String {
    let mut out = String::with_capacity(k.len());
    let mut up = false;
    for c in k.chars() {
        if c == '-' || c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn did_you_mean(k: &str) -> String {
    let best = PROPS.iter().map(|p| (strsim(k, p), *p)).min_by_key(|x| x.0);
    match best {
        Some((d, p)) if d <= 3 => format!(" (did you mean \"{p}\"?)"),
        _ => String::new(),
    }
}

/// Levenshtein distance (case-insensitive).
fn strsim(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j] + (ca != cb) as usize).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// A CSS value as text (numbers become px).
fn css(v: &Value) -> String {
    match v {
        Value::Number(n) => format!("{}px", n.as_f64().unwrap_or(0.0)),
        Value::String(s) => s.trim().to_string(),
        Value::Array(a) => a.iter().map(css).collect::<Vec<_>>().join(" "),
        other => other.to_string(),
    }
}

fn num(v: &Value) -> Result<f32, String> {
    match v {
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0) as f32),
        Value::String(s) => s.trim().parse::<f32>().map_err(|_| format!("expected a number, got {s:?}")),
        _ => Err(format!("expected a number, got {v}")),
    }
}

fn parse<T: FromStr>(s: &str) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    T::from_str(s).map_err(|e| format!("{s:?}: {e}"))
}

/// Expand CSS 1–4 value shorthand into (top, right, bottom, left).
fn sides(s: &str) -> Vec<String> {
    let parts: Vec<String> = split_css(s);
    match parts.len() {
        1 => vec![parts[0].clone(); 4],
        2 => vec![parts[0].clone(), parts[1].clone(), parts[0].clone(), parts[1].clone()],
        3 => vec![parts[0].clone(), parts[1].clone(), parts[2].clone(), parts[1].clone()],
        _ => parts.into_iter().take(4).collect(),
    }
}

/// Split on top-level whitespace (not inside parentheses).
fn split_css(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if c.is_whitespace() && depth == 0 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn rect4<T: FromStr>(s: &str) -> Result<Rect<T>, String>
where
    T::Err: std::fmt::Display,
{
    let v = sides(s);
    Ok(Rect { top: parse(&v[0])?, right: parse(&v[1])?, bottom: parse(&v[2])?, left: parse(&v[3])? })
}

type Tracks = (Vec<GridTemplateComponent<String>>, Vec<Vec<String>>);

fn tracks(s: &str) -> Result<Tracks, String> {
    let t: GridTemplateTracks<String, GridTemplateComponent<String>> = parse(s)?;
    Ok((t.tracks, t.line_names))
}

/// Convert CSS properties to a taffy style, collecting readable errors.
pub fn parse_style(map: &Map<String, Value>, path: &str, errors: &mut Vec<String>) -> Style {
    let mut st = Style::default();
    // Unlike the web, a box with no `display` but children is a flex row (Taffy default).
    for (k, v) in map {
        let key = camel(k);
        let s = css(v);
        let r: Result<(), String> = (|| {
            match key.as_str() {
                "display" => st.display = parse(&s)?,
                "position" => st.position = parse(&s)?,
                "boxSizing" => st.box_sizing = parse(&s)?,
                "overflow" => {
                    let o = parse(&s)?;
                    st.overflow = taffy::geometry::Point { x: o, y: o }
                }
                "width" => st.size.width = parse(&s)?,
                "height" => st.size.height = parse(&s)?,
                "minWidth" => st.min_size.width = parse(&s)?,
                "minHeight" => st.min_size.height = parse(&s)?,
                "maxWidth" => st.max_size.width = parse(&s)?,
                "maxHeight" => st.max_size.height = parse(&s)?,
                "aspectRatio" => {
                    st.aspect_ratio = Some(match s.split_once('/') {
                        Some((a, b)) => num(&Value::String(a.into()))? / num(&Value::String(b.into()))?,
                        None => num(v)?,
                    })
                }
                "margin" => st.margin = rect4(&s)?,
                "marginTop" => st.margin.top = parse(&s)?,
                "marginRight" => st.margin.right = parse(&s)?,
                "marginBottom" => st.margin.bottom = parse(&s)?,
                "marginLeft" => st.margin.left = parse(&s)?,
                "padding" => st.padding = rect4(&s)?,
                "paddingTop" => st.padding.top = parse(&s)?,
                "paddingRight" => st.padding.right = parse(&s)?,
                "paddingBottom" => st.padding.bottom = parse(&s)?,
                "paddingLeft" => st.padding.left = parse(&s)?,
                "border" | "borderWidth" => st.border = rect4(&s)?,
                "inset" => st.inset = rect4(&s)?,
                "top" => st.inset.top = parse(&s)?,
                "right" => st.inset.right = parse(&s)?,
                "bottom" => st.inset.bottom = parse(&s)?,
                "left" => st.inset.left = parse(&s)?,
                "gap" => {
                    let p = split_css(&s);
                    st.gap = Size { height: parse(&p[0])?, width: parse(p.get(1).unwrap_or(&p[0]))? }
                }
                "rowGap" => st.gap.height = parse(&s)?,
                "columnGap" => st.gap.width = parse(&s)?,
                "flexDirection" => st.flex_direction = parse(&s)?,
                "flexWrap" => st.flex_wrap = parse(&s)?,
                "flexGrow" => st.flex_grow = num(v)?,
                "flexShrink" => st.flex_shrink = num(v)?,
                "flexBasis" => st.flex_basis = parse(&s)?,
                "flex" => {
                    // `flex: <grow> [<shrink>] [<basis>]` or a single number.
                    let p = split_css(&s.replace("px", "px "));
                    let p: Vec<&str> = p.iter().map(|x| x.as_str()).collect();
                    match p.as_slice() {
                        ["none"] => {
                            st.flex_grow = 0.0;
                            st.flex_shrink = 0.0;
                        }
                        ["auto"] => {
                            st.flex_grow = 1.0;
                            st.flex_shrink = 1.0;
                        }
                        [g] => {
                            st.flex_grow = g.parse().map_err(|_| format!("bad flex {s:?}"))?;
                            st.flex_basis = Dimension::percent(0.0);
                        }
                        [g, b] if b.parse::<f32>().is_err() => {
                            st.flex_grow = g.parse().map_err(|_| format!("bad flex {s:?}"))?;
                            st.flex_basis = parse(b)?;
                        }
                        [g, sh, rest @ ..] => {
                            st.flex_grow = g.parse().map_err(|_| format!("bad flex {s:?}"))?;
                            st.flex_shrink = sh.parse().map_err(|_| format!("bad flex {s:?}"))?;
                            if let Some(b) = rest.first() {
                                st.flex_basis = parse(b)?;
                            }
                        }
                        _ => return Err(format!("bad flex {s:?}")),
                    }
                }
                "alignItems" => st.align_items = parse(&s)?,
                "alignSelf" => st.align_self = Some(parse(&s)?),
                "alignContent" => st.align_content = parse(&s)?,
                "justifyItems" => st.justify_items = parse(&s)?,
                "justifySelf" => st.justify_self = Some(parse(&s)?),
                "justifyContent" => st.justify_content = parse(&s)?,
                "gridTemplateColumns" => {
                    let (t, n) = tracks(&s)?;
                    st.grid_template_columns = t;
                    st.grid_template_column_names = n;
                }
                "gridTemplateRows" => {
                    let (t, n) = tracks(&s)?;
                    st.grid_template_rows = t;
                    st.grid_template_row_names = n;
                }
                "gridAutoColumns" => st.grid_auto_columns = parse::<taffy::GridAutoTracks>(&s)?.0,
                "gridAutoRows" => st.grid_auto_rows = parse::<taffy::GridAutoTracks>(&s)?.0,
                "gridAutoFlow" => st.grid_auto_flow = parse(&s)?,
                "gridColumn" | "gridRow" => {
                    let (a, b) = match s.split_once('/') {
                        Some((a, b)) => (parse(a.trim())?, parse(b.trim())?),
                        None => (parse(&s)?, GridPlacement::Auto),
                    };
                    let line = Line { start: a, end: b };
                    if key == "gridColumn" {
                        st.grid_column = line
                    } else {
                        st.grid_row = line
                    }
                }
                "textAlign" => st.text_align = parse(&s)?,
                _ => return Err(format!("unknown property{}", did_you_mean(&key))),
            }
            Ok(())
        })();
        if let Err(e) = r {
            errors.push(format!("{path}.style.{k}: {e}"));
        }
    }
    st
}

/// Lay out a box tree inside a `size = [w, h]` viewport; the root fills it
/// unless its style sets a size. Returns rectangles in pre-order, or every
/// style error found.
pub fn layout_boxes(root: &BoxNode, fonts: &FontStore, size: [f32; 2]) -> Result<Vec<BoxRect>, Vec<String>> {
    let mut tree: TaffyTree<Option<TextSpec>> = TaffyTree::new();
    let mut errors = Vec::new();
    let mut order: Vec<(NodeId, Option<String>, Option<usize>, u32)> = Vec::new();

    fn build(
        n: &BoxNode,
        path: String,
        parent: Option<usize>,
        depth: u32,
        tree: &mut TaffyTree<Option<TextSpec>>,
        order: &mut Vec<(NodeId, Option<String>, Option<usize>, u32)>,
        errors: &mut Vec<String>,
    ) -> NodeId {
        let style = parse_style(&n.style, &path, errors);
        let id = tree.new_leaf_with_context(style, n.text.clone()).expect("taffy");
        let me = order.len();
        order.push((id, n.id.clone(), parent, depth));
        for (i, c) in n.children.iter().enumerate() {
            let cp = format!("{path}.children[{i}]");
            let child = build(c, cp, Some(me), depth + 1, tree, order, errors);
            tree.add_child(id, child).expect("taffy");
        }
        id
    }

    let mut root_node = root.clone();
    if !root_node.style.contains_key("width") {
        root_node.style.insert("width".into(), Value::from(size[0]));
    }
    if !root_node.style.contains_key("height") {
        root_node.style.insert("height".into(), Value::from(size[1]));
    }
    let root_id = build(&root_node, "root".into(), None, 0, &mut tree, &mut order, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    tree.compute_layout_with_measure(
        root_id,
        Size { width: AvailableSpace::Definite(size[0]), height: AvailableSpace::Definite(size[1]) },
        |input, _node, ctx, _style| {
            let known = input.known_dimensions;
            let available = input.available_space;
            if let Size { width: Some(w), height: Some(h) } = known {
                return taffy::LayoutOutput::from_outer_size(Size { width: w, height: h });
            }
            let Some(Some(spec)) = ctx else {
                return taffy::LayoutOutput::from_outer_size(Size {
                    width: known.width.unwrap_or(0.0),
                    height: known.height.unwrap_or(0.0),
                });
            };
            let width = known.width.or(match available.width {
                AvailableSpace::Definite(w) => Some(w),
                AvailableSpace::MinContent => Some(0.0),
                AvailableSpace::MaxContent => None,
            });
            let mut s = spec.clone();
            if s.width.is_none() && s.fit.is_none() {
                s.width = width;
            }
            let l = fonts.layout(&s);
            taffy::LayoutOutput::from_outer_size(Size {
                width: known.width.unwrap_or(l.width.ceil()),
                height: known.height.unwrap_or(l.height.ceil()),
            })
        },
    )
    .map_err(|e| vec![e.to_string()])?;

    let mut out: Vec<BoxRect> = Vec::with_capacity(order.len());
    for (id, name, parent, depth) in order {
        let l = tree.layout(id).expect("taffy");
        let (px, py) = parent.map_or((0.0, 0.0), |p| (out[p].rect[0], out[p].rect[1]));
        let x = px + l.location.x;
        let y = py + l.location.y;
        let (w, h) = (l.size.width, l.size.height);
        let cx = x + l.padding.left + l.border.left;
        let cy = y + l.padding.top + l.border.top;
        let cw = w - l.padding.left - l.padding.right - l.border.left - l.border.right;
        let ch = h - l.padding.top - l.padding.bottom - l.border.top - l.border.bottom;
        out.push(BoxRect { id: name, parent, depth, rect: [x, y, w, h], content: [cx, cy, cw.max(0.0), ch.max(0.0)] });
    }
    Ok(out)
}
