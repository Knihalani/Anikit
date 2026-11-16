//! Shorthand forms, for token-efficient specs:
//! a layer may be a string (a text layer), a scene may be a string (a title
//! card), and `animate` may be a preset name or a single tween object.

use super::{Layer, PresetObj, Preset, Scene, StyleRef, Tween};
use schemars::JsonSchema;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

/// Schema helper: a layer or a string (text).
#[derive(JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum LayerItem {
    Text(String),
    Layer(Box<Layer>),
}

/// Schema helper: a scene or a string (a title card).
#[derive(JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum SceneItem {
    Title(String),
    Scene(Box<Scene>),
}

/// Schema helper: `animate` as a preset name, one tween or a list.
#[derive(JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum AnimateItem {
    Preset(String),
    One(Box<Tween>),
    Many(Vec<Tween>),
}

struct LayerV;
impl<'de> Visitor<'de> for LayerV {
    type Value = Layer;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a layer object or a string")
    }
    fn visit_str<E: de::Error>(self, s: &str) -> Result<Layer, E> {
        Ok(Layer { text: Some(s.to_string()), ..Default::default() })
    }
    fn visit_map<M: MapAccess<'de>>(self, m: M) -> Result<Layer, M::Error> {
        Layer::deserialize(de::value::MapAccessDeserializer::new(m))
    }
}
#[derive(Clone, Copy)]
struct LayerSeed;
impl<'de> DeserializeSeed<'de> for LayerSeed {
    type Value = Layer;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Layer, D::Error> {
        d.deserialize_any(LayerV)
    }
}

struct SceneV;
impl<'de> Visitor<'de> for SceneV {
    type Value = Scene;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a scene object or a string (a title card)")
    }
    fn visit_str<E: de::Error>(self, s: &str) -> Result<Scene, E> {
        // "Title" or "Title | subtitle".
        let mut it = s.splitn(2, '|');
        let title = it.next().unwrap_or("").trim().to_string();
        let mut layers = vec![Layer {
            text: Some(title),
            style: Some(StyleRef::Name("title".into())),
            enter: Some(Preset(PresetObj { preset: Some("fadeUp".into()), ..Default::default() })),
            ..Default::default()
        }];
        if let Some(sub) = it.next() {
            layers.push(Layer {
                text: Some(sub.trim().to_string()),
                style: Some(StyleRef::Name("subtitle".into())),
                y: Some(super::Num::Str("62%".into())),
                enter: Some(Preset(PresetObj { preset: Some("fadeUp".into()), delay: Some(super::Num::Num(0.25)), ..Default::default() })),
                ..Default::default()
            });
        }
        Ok(Scene { layers, ..Default::default() })
    }
    fn visit_map<M: MapAccess<'de>>(self, m: M) -> Result<Scene, M::Error> {
        Scene::deserialize(de::value::MapAccessDeserializer::new(m))
    }
}
#[derive(Clone, Copy)]
struct SceneSeed;
impl<'de> DeserializeSeed<'de> for SceneSeed {
    type Value = Scene;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Scene, D::Error> {
        d.deserialize_any(SceneV)
    }
}

struct SeqV<S>(S);
impl<'de, S: DeserializeSeed<'de> + Copy> Visitor<'de> for SeqV<S> {
    type Value = Vec<S::Value>;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a list")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
        let mut v = Vec::new();
        while let Some(x) = a.next_element_seed(self.0)? {
            v.push(x);
        }
        Ok(v)
    }
}

pub fn layers<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Layer>, D::Error> {
    d.deserialize_seq(SeqV(LayerSeed))
}
pub fn opt_layers<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<Layer>>, D::Error> {
    d.deserialize_seq(SeqV(LayerSeed)).map(Some)
}
pub fn scenes<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Scene>, D::Error> {
    d.deserialize_seq(SeqV(SceneSeed))
}

#[derive(Clone, Copy)]
struct TweenSeed;
impl<'de> DeserializeSeed<'de> for TweenSeed {
    type Value = Tween;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Tween, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Tween;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a tween object or a preset name")
            }
            fn visit_str<E: de::Error>(self, s: &str) -> Result<Tween, E> {
                Ok(Tween { preset: Some(s.to_string()), ..Default::default() })
            }
            fn visit_map<M: MapAccess<'de>>(self, m: M) -> Result<Tween, M::Error> {
                Tween::deserialize(de::value::MapAccessDeserializer::new(m))
            }
        }
        d.deserialize_any(V)
    }
}

/// `animate`: a preset name, a tween, or a list of either.
pub fn opt_tweens<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<Tween>>, D::Error> {
    struct V;
    impl<'de> Visitor<'de> for V {
        type Value = Vec<Tween>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "a preset name, a tween object or a list of tweens")
        }
        fn visit_str<E: de::Error>(self, s: &str) -> Result<Vec<Tween>, E> {
            Ok(vec![Tween { preset: Some(s.to_string()), ..Default::default() }])
        }
        fn visit_map<M: MapAccess<'de>>(self, m: M) -> Result<Vec<Tween>, M::Error> {
            Tween::deserialize(de::value::MapAccessDeserializer::new(m)).map(|t| vec![t])
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Vec<Tween>, A::Error> {
            let mut v = Vec::new();
            while let Some(x) = a.next_element_seed(TweenSeed)? {
                v.push(x);
            }
            Ok(v)
        }
    }
    d.deserialize_any(V).map(Some)
}
