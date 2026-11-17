//! Built-in reference shaders: docs examples and agent starting points.
//!
//! Each is original, deterministic, and parameterised by uniforms with
//! `// = default` comments (gl-transitions convention) so a spec can tune
//! them by name: `{"shader": "aurora", "uniforms": {"intensity": 1.4}}`.

/// What a built-in is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// Generates an image (a background or a layer).
    Image,
    /// Processes the layers below (`iChannel0`); use with `"filter": true`.
    Filter,
    /// A scene transition (`progress`, `getFromColor`, `getToColor`).
    Transition,
}

pub struct Entry {
    pub name: &'static str,
    pub use_: Use,
    pub about: &'static str,
    /// Measured ms/frame at 1920×1080 on 4 cores (AVX2), no supersampling:
    /// `cargo run --release -p anikit-shader --example library_bench`.
    pub ms_1080p: u32,
    pub src: &'static str,
}

impl Entry {
    /// The uniforms a spec can set, with defaults and docs.
    pub fn uniforms(&self) -> Vec<crate::DeclaredUniform> {
        crate::front::declared_uniforms(self.src)
    }

    /// Anti-aliased by default as a layer (image shaders without `// anikit:aa off`).
    pub fn aa_default(&self) -> bool {
        self.use_ == Use::Image && !crate::aa_off(self.src)
    }

    /// `"layer"`, `"filter"` or `"transition"`: how a spec uses it.
    pub fn use_name(&self) -> &'static str {
        match self.use_ {
            Use::Image => "layer",
            Use::Filter => "filter",
            Use::Transition => "transition",
        }
    }
}

macro_rules! lib {
    ($( $name:literal, $use:ident, $ms:literal, $about:literal; )*) => {
        pub const ALL: &[Entry] = &[ $( Entry { name: $name, use_: Use::$use, about: $about, ms_1080p: $ms, src: include_str!(concat!("../library/", $name, ".glsl")) }, )* ];
    };
}

lib! {
    "sdf-scene", Image, 265, "Portals at golden hour: concrete frames down a reflecting pool, backlit sun, soft shadows, AO, haze, slow dolly.";
    "liquid-chrome", Image, 125, "Liquid metal: a heavy mass with drops that merge and tear away, mirror chrome in a studio or night sky; cut-out with background = 0.";
    "aurora", Image, 499, "Aurora curtains over a ridge, mirrored in a lake. Heavy; lake = 0 + lake-mirror is faster.";
    "hero", Image, 48, "Product shot: one object (torus, cube, pill, gem, sphere, twisted bar) on a glossy studio floor, plastic or metal.";
    "mesh-gradient", Image, 32, "Soft animated mesh gradient: four colours drifting and folding into each other, with film grain. Backgrounds for titles and products.";
    "ocean", Image, 191, "Open sea to the horizon under a low sun: swell and chop, Fresnel sky reflection, glitter path, crest glow, foam.";
    "caustics", Image, 51, "Pool-floor caustics from a moving water surface (ray density via derivatives).";
    "flow-ink", Image, 151, "Two inks marbled by a curl-noise flow field on paper.";
    "turing", Image, 25, "Reaction–diffusion spots and labyrinths (one-wavelength plane-wave sum, no simulation).";
    "light-shafts", Image, 91, "Sun through a window into a hazy room: pane-shaped beams with mullions and leaves, drifting haze, glinting dust.";
    "particles", Image, 98, "Glowing particles drifting in depth with bokeh (depth of field), one pass.";
    "glass", Filter, 80, "The layers below seen through fluted, ribbed, rippled or pebbled glass: refraction, colour fringes, frost.";
    "halftone", Filter, 60, "Printed dots: CMYK screens at classic angles (or one black screen) on paper.";
    "risograph", Filter, 28, "Two or three spot inks, out of register, with stencil grain: the risograph look.";
    "linocut", Filter, 19, "Carved block print: solid ink, tone-following gouges, bold outlines, paper tooth.";
    "watercolour", Filter, 80, "Washes that bleed and pool dark at their edges, with paper granulation.";
    "crosshatch", Filter, 25, "Pen or pencil hatching: more directions for darker tones, hand wobble, outlines.";
    "film", Filter, 25, "Photochemical stock: filmic curve, lifted blacks, red halation, gate weave, midtone grain.";
    "dither", Filter, 18, "Ordered (Bayer) dither into a 2- or 3-colour palette at a chosen pixel size.";
    "screenprint", Filter, 30, "Flat poster colour: tones as 1–3 ink layers, off register, with ink texture.";
    "print-offset", Filter, 33, "Cheap print: cyan, magenta and yellow plates drift apart on fibrous paper.";
    "film-burn", Filter, 77, "Scorches the frame below from hot spots; animate `amount` 0 → 1.";
    "lake-mirror", Filter, 31, "Reflects everything above a horizon in still, rippled water below it (pairs with aurora lake = 0).";
    "glitch", Filter, 27, "Codec-style block displacement, row smears, RGB split and scanlines in bursts.";
    "burn", Transition, 52, "Old scene burns away along a noisy ember edge.";
    "ripple", Transition, 34, "A ring wave spreads from the centre carrying the new scene.";
    "pixelate", Transition, 22, "Pixel blocks grow, cross-fade, then resolve.";
    "swirl", Transition, 38, "A vortex twists the old scene and unwinds into the new one.";
    "ink-bleed", Transition, 59, "The new scene soaks in like ink, with a dark wet edge.";
    "warp-wipe", Transition, 29, "A soft angled wipe that smears pixels as it passes.";
    "luma-melt", Transition, 20, "The old scene melts away darkest-first.";
    "zoom-blur", Transition, 129, "Punch-in zoom blur with chromatic fringes; cuts at peak blur.";
}

/// A built-in by name.
pub fn get(name: &str) -> Option<&'static Entry> {
    ALL.iter().find(|e| e.name == name)
}

/// All names, for suggestions.
pub fn names() -> Vec<&'static str> {
    ALL.iter().map(|e| e.name).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_uniform_has_a_default_and_a_doc() {
        for e in super::ALL {
            for u in e.uniforms() {
                assert!(u.default.is_some(), "{}.{}: no `// = default`", e.name, u.name);
                assert!(!u.doc.is_empty(), "{}.{}: no doc after the default", e.name, u.name);
            }
        }
        let u = crate::front::declared_uniforms("uniform vec3 tint; // = vec3(1.0, 0.5, 0.25) — water colour\nuniform float k; // = 2.5 how hard\n");
        assert_eq!((u[0].default.as_deref(), u[0].doc.as_str()), (Some(&[1.0, 0.5, 0.25][..]), "water colour"));
        assert_eq!((u[1].default.as_deref(), u[1].doc.as_str()), (Some(&[2.5][..]), "how hard"));
    }

    #[test]
    fn every_builtin_compiles() {
        for e in super::ALL {
            if let Err(err) = crate::Shader::compile(e.src) {
                panic!("{}: {err}", e.name);
            }
        }
    }
}
