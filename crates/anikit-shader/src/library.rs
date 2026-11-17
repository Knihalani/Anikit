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
    pub src: &'static str,
}

macro_rules! lib {
    ($( $name:literal, $use:ident, $about:literal; )*) => {
        pub const ALL: &[Entry] = &[ $( Entry { name: $name, use_: Use::$use, about: $about, src: include_str!(concat!("../library/", $name, ".glsl")) }, )* ];
    };
}

lib! {
    "sdf-scene", Image, "Portals at golden hour: concrete frames down a reflecting pool, backlit sun, soft shadows, AO, haze, slow dolly.";
    "liquid-chrome", Image, "Chrome metaballs reflecting a procedural studio.";
    "aurora", Image, "Aurora curtains over stars, mirrored in a lake.";
    "caustics", Image, "Pool-floor caustics from a moving water surface (ray density via derivatives).";
    "flow-ink", Image, "Two inks marbled by a curl-noise flow field on paper.";
    "turing", Image, "Reaction–diffusion spots and labyrinths (one-wavelength plane-wave sum, no simulation).";
    "film-burn", Filter, "Scorches the frame below from hot spots; animate `amount` 0 → 1.";
    "glitch", Filter, "Codec-style block displacement, row smears, RGB split and scanlines in bursts.";
    "burn", Transition, "Old scene burns away along a noisy ember edge.";
    "ripple", Transition, "A ring wave spreads from the centre carrying the new scene.";
    "pixelate", Transition, "Pixel blocks grow, cross-fade, then resolve.";
    "swirl", Transition, "A vortex twists the old scene and unwinds into the new one.";
    "ink-bleed", Transition, "The new scene soaks in like ink, with a dark wet edge.";
    "warp-wipe", Transition, "A soft angled wipe that smears pixels as it passes.";
    "luma-melt", Transition, "The old scene melts away darkest-first.";
    "zoom-blur", Transition, "Punch-in zoom blur with chromatic fringes; cuts at peak blur.";
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
    fn every_builtin_compiles() {
        for e in super::ALL {
            if let Err(err) = crate::Shader::compile(e.src) {
                panic!("{}: {err}", e.name);
            }
        }
    }
}
