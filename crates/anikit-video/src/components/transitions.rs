//! (in progress)
use super::{Component, Ctx, Prepare};
use crate::dl::Op;
use serde::Deserialize;
#[derive(Deserialize, Default)] #[serde(default)] pub struct Wipe { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Wipe {}
impl Component for Wipe { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Iris { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Iris {}
impl Component for Iris { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Glitch { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Glitch {}
impl Component for Glitch { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
