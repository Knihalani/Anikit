//! (in progress)
use super::{Component, Ctx, Prepare};
use crate::dl::Op;
use serde::Deserialize;
#[derive(Deserialize, Default)] #[serde(default)] pub struct BarChart { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for BarChart {}
impl Component for BarChart { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct LineChart { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for LineChart {}
impl Component for LineChart { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct AreaChart { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for AreaChart {}
impl Component for AreaChart { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct DonutChart { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for DonutChart {}
impl Component for DonutChart { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct PieChart { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for PieChart {}
impl Component for PieChart { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Counter { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Counter {}
impl Component for Counter { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Stat { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Stat {}
impl Component for Stat { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
