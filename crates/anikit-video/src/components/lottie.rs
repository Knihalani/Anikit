//! (in progress)
use super::{Component, Ctx, Prepare};
use crate::dl::Op;
use serde::Deserialize;
#[derive(Deserialize, Default)] #[serde(default)] pub struct Lottie { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Lottie {}
impl Component for Lottie { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
