//! (in progress)
use super::{Component, Ctx, Prepare};
use crate::dl::Op;
use serde::Deserialize;
#[derive(Deserialize, Default)] #[serde(default)] pub struct CodeBlock { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for CodeBlock {}
impl Component for CodeBlock { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Terminal { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Terminal {}
impl Component for Terminal { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
