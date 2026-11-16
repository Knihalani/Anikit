//! (in progress)
use super::{Component, Ctx, Prepare};
use crate::dl::Op;
use serde::Deserialize;
#[derive(Deserialize, Default)] #[serde(default)] pub struct Confetti { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Confetti {}
impl Component for Confetti { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Sparks { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Sparks {}
impl Component for Sparks { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Snow { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Snow {}
impl Component for Snow { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Dust { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Dust {}
impl Component for Dust { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
