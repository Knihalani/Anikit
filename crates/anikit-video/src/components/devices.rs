//! (in progress)
use super::{Component, Ctx, Prepare};
use crate::dl::Op;
use serde::Deserialize;
#[derive(Deserialize, Default)] #[serde(default)] pub struct Phone { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Phone {}
impl Component for Phone { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Laptop { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Laptop {}
impl Component for Laptop { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Browser { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Browser {}
impl Component for Browser { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
