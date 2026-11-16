//! (in progress)
use super::{Component, Ctx, Prepare};
use crate::dl::Op;
use serde::Deserialize;
#[derive(Deserialize, Default)] #[serde(default)] pub struct Card { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Card {}
impl Component for Card { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Button { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Button {}
impl Component for Button { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Toggle { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Toggle {}
impl Component for Toggle { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Badge { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Badge {}
impl Component for Badge { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Avatar { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Avatar {}
impl Component for Avatar { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct AvatarStack { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for AvatarStack {}
impl Component for AvatarStack { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct ProgressBar { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for ProgressBar {}
impl Component for ProgressBar { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct ProgressRing { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for ProgressRing {}
impl Component for ProgressRing { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Toast { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Toast {}
impl Component for Toast { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Cursor { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Cursor {}
impl Component for Cursor { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Tooltip { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Tooltip {}
impl Component for Tooltip { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Callout { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Callout {}
impl Component for Callout { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct Arrow { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for Arrow {}
impl Component for Arrow { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct LowerThird { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for LowerThird {}
impl Component for LowerThird { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
#[derive(Deserialize, Default)] #[serde(default)] pub struct TitleCard { #[allow(dead_code)] values: Vec<f32> }
impl Prepare for TitleCard {}
impl Component for TitleCard { fn eval(&self, _t: f32, _c: &Ctx) -> Vec<Op> { Vec::new() } }
