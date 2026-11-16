//! Pluggable components: `{ "type": "component", "component": "barChart", "props": {…} }`.
//!
//! A [`ComponentFactory`] builds a [`Component`] once (at `Evaluator::new`)
//! from its JSON props; the component then emits display-list ops for any
//! time. Ops are in the component's **local box** (origin top-left, size
//! [`Component::size`]); the evaluator applies the layer's transform,
//! opacity, filters and animation on top.

use super::theme::Theme;
use crate::dl::Op;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Context for building a component.
pub struct BuildCtx<'a> {
    /// Output frame size in px.
    pub frame: [f32; 2],
    pub fps: f32,
    /// Resolved theme (colours, fonts, text styles).
    pub theme: &'a Theme,
    /// `width` / `height` given on the layer, already resolved to px.
    pub size: [Option<f32>; 2],
    /// The layer's visible duration in seconds (`None` = until scene end, unknown).
    pub duration: Option<f32>,
    /// Directory relative paths in props resolve against.
    pub base_dir: &'a Path,
    /// Text layout, to measure and shape text inside the component.
    pub text: &'a dyn super::text::TextEngine,
}

/// Per-frame context.
#[derive(Clone, Copy, Debug)]
pub struct EvalCtx {
    /// Seconds since the layer's start.
    pub t: f32,
    /// The layer's `progress` property (0..1 over its lifetime unless animated).
    pub progress: f32,
    /// The layer's visible duration in seconds.
    pub duration: f32,
    pub fps: f32,
}

/// A built component instance. Must be pure: same `EvalCtx` → same ops.
pub trait Component: Send + Sync {
    /// Size of the local box in px (used for anchoring and layout).
    fn size(&self) -> [f32; 2];
    /// Append ops (local coordinates) for this moment.
    fn eval(&self, cx: &EvalCtx, out: &mut Vec<Op>);
    /// Natural duration, if it has one (used when the layer has no `end`).
    fn duration(&self) -> Option<f32> {
        None
    }
    /// True if the output is identical for every time in `[t0, t1]`
    /// with constant `progress` (lets the pipeline skip frames).
    fn is_static(&self, _t0: f32, _t1: f32) -> bool {
        false
    }
}

/// Builds components of one kind.
pub trait ComponentFactory: Send + Sync {
    /// The `component` name, camelCase (`"barChart"`).
    fn name(&self) -> &str;
    /// One-line description for `anikit-video info --components`.
    fn describe(&self) -> &str {
        ""
    }
    /// JSON Schema of `props` (for docs and validation), if available.
    fn props_schema(&self) -> Option<serde_json::Value> {
        None
    }
    /// Build an instance. Errors are reported at `…props` with the message as given.
    fn build(&self, props: &serde_json::Value, cx: &BuildCtx) -> Result<Box<dyn Component>, String>;
}

/// The set of available components.
#[derive(Clone, Default)]
pub struct Registry {
    map: HashMap<String, Arc<dyn ComponentFactory>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }
    /// Register (or replace) a factory under its name.
    pub fn register(&mut self, f: impl ComponentFactory + 'static) {
        self.map.insert(f.name().to_string(), Arc::new(f));
    }
    pub fn get(&self, name: &str) -> Option<&Arc<dyn ComponentFactory>> {
        self.map.get(name)
    }
    pub fn names(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.map.keys().map(|s| s.as_str()).collect();
        v.sort();
        v
    }
}
