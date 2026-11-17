//! Synthesised drum machines keyed by General MIDI drum keys.
//!
//! Every hit is built from the same few analog-style parts, all optional:
//! up to four sine "tones" (kick body, snare shells, tom/taiko membrane modes)
//! sharing a pitch envelope; filtered noise with an attack, a multi-burst
//! pattern (claps) and a room tail; a six-square-oscillator "metal" bank
//! (808 hats/cymbals/cowbell); a click; then drive, filters and optional
//! 12-bit grit. Kits: `808`, `909`, `trap`, `lofi`, `house`, `techno`,
//! `cinematic`.
//!
//! Keys below 35 in the `808` and `trap` kits play a tuned 808 bass at the
//! key's pitch. The fractional part of any key detunes the hit (semitones).
//! Hi-hats (42, 44, 46) share exclusive class 1, so a closed hat chokes an open one.

use super::filter::{OnePole, Svf, SvfCoef};
use super::osc::{Lanes, Wave};
use super::util::{flush, peak, prewarp, sin2pi, t60_coef, tanh, vel_gain, Noise, Out, SILENCE};
use crate::voice::{key_to_freq, Instrument, NoteOn, Voice};
use std::sync::Arc;

/// Kit names accepted by [`kit`].
pub const KITS: &[&str] = &["808", "909", "trap", "lofi", "house", "techno", "cinematic"];

/// A drum machine by name.
pub fn kit(name: &str) -> Option<Arc<dyn Instrument>> {
    let k = match name {
        "808" => Kit::K808,
        "909" => Kit::K909,
        "trap" => Kit::Trap,
        "lofi" | "lo-fi" => Kit::Lofi,
        "house" => Kit::House,
        "techno" => Kit::Techno,
        "cinematic" => Kit::Cinematic,
        _ => return None,
    };
    Some(Arc::new(DrumKit(k)))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kit {
    K808,
    K909,
    Trap,
    Lofi,
    House,
    Techno,
    Cinematic,
}

/// A drum machine instrument.
pub struct DrumKit(Kit);

impl Instrument for DrumKit {
    fn voice(&self, n: &NoteOn) -> Box<dyn Voice> {
        let key = n.key.round();
        let detune = n.key - key;
        match sound(self.0, key as i32, n.key) {
            Some(mut d) => {
                d.freq *= super::util::semis(detune);
                d.metal_scale *= super::util::semis(detune);
                Box::new(DrumVoice::new(&d, n))
            }
            None => Box::new(Silent),
        }
    }
}

struct Silent;
impl Voice for Silent {
    fn render(&mut self, _: &mut [f32], _: &mut [f32]) -> bool {
        false
    }
    fn release(&mut self) {}
}

/// Noise filter mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NMode {
    Lp,
    Bp,
    Hp,
}

/// One drum sound.
#[derive(Clone, Copy, Debug)]
pub struct Drum {
    /// Base frequency of the tones (Hz).
    pub freq: f32,
    /// (ratio, amplitude, −60 dB time) per tone.
    pub tones: [(f32, f32, f32); 4],
    /// Pitch envelope: start ratio and time constant (s).
    pub pitch: (f32, f32),
    /// Saturation of the tone sum (0 = clean sine).
    pub tone_drive: f32,
    pub noise: f32,
    pub noise_t60: f32,
    pub noise_attack: f32,
    pub nmode: NMode,
    pub noise_fc: f32,
    pub noise_q: f32,
    /// Stereo decorrelation of the noise (0..1).
    pub width: f32,
    /// Clap bursts: count, spacing (s), per-burst −60 dB time.
    pub bursts: (u32, f32, f32),
    /// Room/body tail on the noise: amplitude, −60 dB time.
    pub tail: (f32, f32),
    pub metal: f32,
    pub metal_t60: f32,
    /// 2 = cowbell pair, 6 = full 808 metal bank.
    pub metal_n: usize,
    pub metal_scale: f32,
    pub metal_bp: f32,
    pub metal_hp: f32,
    pub click: f32,
    pub click_fc: f32,
    pub drive: f32,
    /// Final low-pass / high-pass (Hz, 0 = off).
    pub lp: f32,
    pub hp: f32,
    /// 12-bit grit + vinyl dust (lo-fi).
    pub grit: f32,
    pub gain: f32,
    pub pan: f32,
    pub excl: u32,
}

const NONE4: [(f32, f32, f32); 4] = [(1.0, 0.0, 0.1); 4];

impl Drum {
    const fn new() -> Drum {
        Drum {
            freq: 100.0,
            tones: NONE4,
            pitch: (1.0, 0.01),
            tone_drive: 0.0,
            noise: 0.0,
            noise_t60: 0.2,
            noise_attack: 0.0,
            nmode: NMode::Hp,
            noise_fc: 1000.0,
            noise_q: 0.7,
            width: 0.0,
            bursts: (0, 0.0, 0.0),
            tail: (0.0, 0.1),
            metal: 0.0,
            metal_t60: 0.1,
            metal_n: 6,
            metal_scale: 1.0,
            metal_bp: 10000.0,
            metal_hp: 7000.0,
            click: 0.0,
            click_fc: 3000.0,
            drive: 0.0,
            lp: 0.0,
            hp: 0.0,
            grit: 0.0,
            gain: 1.0,
            pan: 0.0,
            excl: 0,
        }
    }
}

// ---------------------------------------------------------------- building blocks

fn kick(f: f32, start: f32, ptau: f32, t60: f32, click: f32, drive: f32) -> Drum {
    let mut d = Drum::new();
    d.freq = f;
    d.tones[0] = (1.0, 1.0, t60);
    d.pitch = (start, ptau);
    d.tone_drive = drive;
    d.click = click;
    d.click_fc = 3500.0;
    d.gain = 0.9;
    d
}

fn snare(f: f32, tone_t60: f32, noise: f32, noise_t60: f32, fc: f32) -> Drum {
    let mut d = Drum::new();
    d.freq = f;
    d.tones[0] = (1.0, 0.55, tone_t60);
    d.tones[1] = (1.78, 0.35, tone_t60 * 0.6);
    d.pitch = (1.18, 0.012);
    d.noise = noise;
    d.noise_t60 = noise_t60;
    d.nmode = NMode::Hp;
    d.noise_fc = fc;
    d.width = 0.25;
    d.click = 0.25;
    d.click_fc = 4000.0;
    d.gain = 0.55;
    d
}

fn clap(fc: f32, bursts: u32, spacing: f32, tail: f32, tail_t60: f32) -> Drum {
    let mut d = Drum::new();
    d.noise = 1.0;
    d.noise_t60 = 0.025;
    d.nmode = NMode::Bp;
    d.noise_fc = fc;
    d.noise_q = 1.6;
    d.bursts = (bursts, spacing, 0.03);
    d.tail = (tail, tail_t60);
    d.width = 0.6;
    d.gain = 0.7;
    d
}

fn hat(t60: f32, metal: f32, noise: f32, bp: f32, hp: f32) -> Drum {
    let mut d = Drum::new();
    d.metal = metal;
    d.metal_t60 = t60;
    d.metal_bp = bp;
    d.metal_hp = hp;
    d.noise = noise;
    d.noise_t60 = t60;
    d.nmode = NMode::Hp;
    d.noise_fc = hp;
    d.width = 0.3;
    d.excl = 1;
    d.pan = 0.25;
    d.gain = 0.45;
    d
}

fn cymbal(t60: f32, metal: f32, noise: f32, bp: f32, pan: f32) -> Drum {
    let mut d = hat(t60, metal, noise, bp, 5000.0);
    d.excl = 0;
    d.width = 0.7;
    d.pan = pan;
    d.noise_fc = 4500.0;
    d.gain = 0.35;
    d
}

fn tom(f: f32, t60: f32, pan: f32) -> Drum {
    let mut d = Drum::new();
    d.freq = f;
    d.tones[0] = (1.0, 1.0, t60);
    d.tones[1] = (1.5, 0.18, t60 * 0.4);
    d.pitch = (1.25, 0.06);
    d.noise = 0.12;
    d.noise_t60 = 0.08;
    d.nmode = NMode::Lp;
    d.noise_fc = 3000.0;
    d.click = 0.15;
    d.pan = pan;
    d.gain = 0.55;
    d
}

/// Circular-membrane modes (Bessel zeros ratios) for taiko / cinematic toms.
fn membrane(f: f32, t60: f32, pan: f32) -> Drum {
    let mut d = Drum::new();
    d.freq = f;
    d.tones = [
        (1.0, 1.0, t60),
        (1.594, 0.55, t60 * 0.6),
        (2.136, 0.32, t60 * 0.45),
        (2.296, 0.25, t60 * 0.35),
    ];
    d.pitch = (1.35, 0.07);
    d.tone_drive = 0.4;
    d.noise = 0.35;
    d.noise_t60 = 0.25;
    d.nmode = NMode::Lp;
    d.noise_fc = 500.0;
    d.click = 0.3;
    d.click_fc = 1200.0;
    d.pan = pan;
    d.gain = 0.8;
    d
}

fn cowbell() -> Drum {
    let mut d = Drum::new();
    d.metal = 1.0;
    d.metal_n = 2;
    d.metal_t60 = 0.35;
    d.metal_bp = 900.0;
    d.metal_hp = 300.0;
    d.pan = -0.2;
    d.gain = 0.2;
    d
}

fn shaker(t60: f32, attack: f32, fc: f32) -> Drum {
    let mut d = Drum::new();
    d.noise = 1.0;
    d.noise_t60 = t60;
    d.noise_attack = attack;
    d.nmode = NMode::Hp;
    d.noise_fc = fc;
    d.width = 0.4;
    d.pan = 0.35;
    d.gain = 0.8;
    d
}

fn tamb() -> Drum {
    let mut d = shaker(0.25, 0.002, 7000.0);
    d.metal = 0.6;
    d.metal_t60 = 0.3;
    d.metal_scale = 2.1;
    d.metal_bp = 9000.0;
    d.metal_hp = 6000.0;
    d.bursts = (3, 0.012, 0.08);
    d.pan = -0.35;
    d.gain = 0.55;
    d
}

fn wood(f: f32, t60: f32, pan: f32) -> Drum {
    let mut d = Drum::new();
    d.freq = f;
    d.tones = [
        (1.0, 1.0, t60),
        (2.57, 0.3, t60 * 0.5),
        (4.1, 0.12, t60 * 0.3),
        (1.0, 0.0, 0.1),
    ];
    d.click = 0.3;
    d.click_fc = 5000.0;
    d.pan = pan;
    d.gain = 0.3;
    d
}

fn rim(f: f32) -> Drum {
    let mut d = Drum::new();
    d.freq = f;
    d.tones = [
        (1.0, 0.7, 0.05),
        (3.4, 0.5, 0.025),
        (1.0, 0.0, 0.1),
        (1.0, 0.0, 0.1),
    ];
    d.click = 0.6;
    d.click_fc = 4000.0;
    d.noise = 0.15;
    d.noise_t60 = 0.03;
    d.nmode = NMode::Bp;
    d.noise_fc = 2500.0;
    d.hp = 250.0;
    d.gain = 0.55;
    d
}

fn bass808(key: f32) -> Drum {
    let mut f = key_to_freq(key);
    while f < 27.0 {
        f *= 2.0;
    }
    let mut d = kick(f, 1.7, 0.03, 2.4, 0.08, 1.4);
    d.gain = 0.8;
    d
}

// ---------------------------------------------------------------- kits

fn sound(kit: Kit, key: i32, fkey: f32) -> Option<Drum> {
    use Kit::*;
    let toms = |base: f32, t60: f32| -> Option<Drum> {
        let (f, pan) = match key {
            41 => (base, -0.4),
            43 => (base * 1.19, -0.25),
            45 => (base * 1.41, -0.1),
            47 => (base * 1.68, 0.05),
            48 => (base * 2.0, 0.2),
            50 => (base * 2.38, 0.35),
            _ => (base * 2.6, 0.3),
        };
        Some(if kit == Cinematic {
            membrane(f, t60, pan)
        } else {
            tom(f, t60, pan)
        })
    };
    let mut d = match (kit, key) {
        // ------------------------------------------------ 808
        (K808 | Trap, k) if k < 35 => bass808(fkey),
        (K808, 36) => kick(49.0, 2.6, 0.035, 1.5, 0.12, 0.6),
        (K808, 35) => kick(55.0, 2.4, 0.03, 0.7, 0.12, 0.5),
        (K808, 38 | 40) => snare(180.0, 0.22, 0.75, 0.28, 1800.0),
        (K808, 37) => rim(480.0),
        (K808, 39) => clap(1100.0, 4, 0.0095, 0.5, 0.33),
        (K808, 42) => hat(0.09, 1.0, 0.15, 10000.0, 7000.0),
        (K808, 44) => hat(0.14, 1.0, 0.15, 10000.0, 7000.0),
        (K808, 46) => hat(0.75, 1.0, 0.15, 10000.0, 7000.0),
        (K808, 49 | 52 | 55 | 57) => {
            cymbal(2.4, 1.0, 0.6, 8000.0, if key == 57 { 0.4 } else { -0.4 })
        }
        (K808, 51 | 53 | 59) => cymbal(1.6, 1.0, 0.2, 7000.0, 0.45),
        (K808, 75) => wood(2500.0, 0.07, 0.3),
        // ------------------------------------------------ 909
        (K909 | House | Techno, 36) | (K909, 35) => {
            let (f, t60, drive) = match kit {
                House => (51.0, 0.75, 0.8),
                Techno => (47.0, 0.65, 2.0),
                _ => (53.0, 0.5, 1.0),
            };
            let mut d = kick(f, 4.5, 0.011, t60, 0.45, drive);
            d.noise = 0.12;
            d.noise_t60 = 0.03;
            d.nmode = NMode::Hp;
            d.noise_fc = 3000.0;
            d
        }
        (House | Techno, 35) => kick(56.0, 3.5, 0.012, 0.35, 0.3, 0.7),
        (K909 | House | Techno, 38 | 40) => {
            let mut d = snare(190.0, 0.16, 0.85, 0.3, 1200.0);
            d.noise_q = 0.6;
            if kit == Techno {
                d.tail = (0.25, 0.4);
            }
            d
        }
        (K909 | House | Techno, 37) => rim(520.0),
        (K909 | House, 39) => clap(
            1200.0,
            3,
            0.011,
            0.45,
            if kit == House { 0.45 } else { 0.3 },
        ),
        (Techno, 39) => clap(1000.0, 4, 0.012, 0.6, 0.7),
        (K909 | House | Techno, 42) => hat(0.07, 0.7, 0.6, 9500.0, 8000.0),
        (K909 | House | Techno, 44) => hat(0.11, 0.7, 0.6, 9500.0, 8000.0),
        (K909 | House | Techno, 46) => hat(0.55, 0.7, 0.6, 9500.0, 8000.0),
        (K909 | House | Techno, 49 | 52 | 55 | 57) => {
            cymbal(2.2, 0.7, 0.9, 7500.0, if key == 57 { 0.4 } else { -0.4 })
        }
        (K909 | House | Techno, 51 | 53 | 59) => {
            let mut d = cymbal(2.0, 1.0, 0.25, 6000.0, 0.45);
            d.metal_scale = 1.6;
            d
        }
        // ------------------------------------------------ trap
        (Trap, 36 | 35) => {
            let mut d = kick(
                52.0,
                3.2,
                0.025,
                if key == 36 { 1.2 } else { 0.5 },
                0.3,
                1.2,
            );
            d.click_fc = 5000.0;
            d
        }
        (Trap, 38 | 40) => {
            let mut d = snare(200.0, 0.18, 1.0, 0.35, 1500.0);
            d.tail = (0.35, 0.35);
            d.gain = 0.42;
            d
        }
        (Trap, 37) => rim(560.0),
        (Trap, 39) => clap(1300.0, 4, 0.008, 0.65, 0.45),
        (Trap, 42) => hat(0.045, 0.9, 0.4, 11000.0, 8000.0),
        (Trap, 44) => hat(0.08, 0.9, 0.4, 11000.0, 8000.0),
        (Trap, 46) => hat(0.45, 0.9, 0.4, 11000.0, 8000.0),
        (Trap, 49 | 52 | 55 | 57) => cymbal(2.0, 0.8, 0.8, 8000.0, -0.4),
        (Trap, 51 | 53 | 59) => cymbal(1.4, 1.0, 0.2, 7000.0, 0.45),
        // ------------------------------------------------ lofi
        (Lofi, 36 | 35) => {
            let mut d = kick(56.0, 2.2, 0.02, 0.45, 0.15, 1.2);
            d.noise = 0.08;
            d.noise_t60 = 0.04;
            d.nmode = NMode::Lp;
            d
        }
        (Lofi, 38 | 40) => {
            let mut d = snare(210.0, 0.14, 0.7, 0.22, 900.0);
            d.nmode = NMode::Bp;
            d.noise_fc = 2200.0;
            d.noise_q = 0.8;
            d.tail = (0.2, 0.25);
            d
        }
        (Lofi, 37) => rim(450.0),
        (Lofi, 39) => clap(1000.0, 3, 0.01, 0.35, 0.25),
        (Lofi, 42) => hat(0.06, 0.4, 0.8, 7500.0, 6000.0),
        (Lofi, 44) => hat(0.1, 0.4, 0.8, 7500.0, 6000.0),
        (Lofi, 46) => hat(0.4, 0.4, 0.8, 7500.0, 6000.0),
        (Lofi, 49 | 52 | 55 | 57) => cymbal(1.6, 0.5, 0.9, 6000.0, -0.4),
        (Lofi, 51 | 53 | 59) => cymbal(1.2, 0.8, 0.4, 5500.0, 0.45),
        // ------------------------------------------------ cinematic
        (Cinematic, 36) => {
            let mut d = membrane(58.0, 1.8, 0.0);
            d.gain = 0.9;
            d.tail = (0.2, 1.2);
            d
        }
        (Cinematic, 35) => {
            let mut d = kick(33.0, 2.6, 0.08, 4.5, 0.05, 0.8);
            d.noise = 0.3;
            d.noise_t60 = 0.6;
            d.nmode = NMode::Lp;
            d.noise_fc = 150.0;
            d
        }
        (Cinematic, 38 | 40) => {
            let mut d = snare(165.0, 0.4, 0.9, 0.9, 900.0);
            d.tail = (0.3, 1.6);
            d.gain = 0.42;
            d.width = 0.5;
            d
        }
        (Cinematic, 37) => {
            let mut d = rim(380.0);
            d.tail = (0.1, 0.8);
            d.noise_fc = 1800.0;
            d
        }
        (Cinematic, 39) => clap(900.0, 5, 0.012, 0.6, 1.4),
        (Cinematic, 42) => hat(0.08, 0.7, 0.5, 8000.0, 6500.0),
        (Cinematic, 44) => hat(0.13, 0.7, 0.5, 8000.0, 6500.0),
        (Cinematic, 46) => hat(0.9, 0.7, 0.5, 8000.0, 6500.0),
        (Cinematic, 49 | 52 | 55 | 57) => {
            cymbal(4.5, 0.8, 0.9, 7000.0, if key == 57 { 0.4 } else { -0.4 })
        }
        (Cinematic, 51 | 53 | 59) => cymbal(3.0, 1.0, 0.3, 6500.0, 0.45),
        // ------------------------------------------------ shared
        (Cinematic, 41 | 43 | 45 | 47 | 48 | 50) => toms(70.0, 2.2)?,
        (K808 | Trap, 41 | 43 | 45 | 47 | 48 | 50) => toms(82.0, 0.9)?,
        (_, 41 | 43 | 45 | 47 | 48 | 50) => toms(95.0, 0.55)?,
        (_, 56) => cowbell(),
        (_, 54) => tamb(),
        (_, 69 | 70 | 82) => shaker(0.09, 0.012, 6500.0),
        (_, 75) => wood(2500.0, 0.07, 0.3),
        (_, 76) => wood(880.0, 0.12, -0.3),
        (_, 77) => wood(660.0, 0.12, -0.3),
        // Bongos / congas / timbales: small membranes.
        (_, 60..=68) => {
            let f = [
                380.0, 290.0, 330.0, 270.0, 200.0, 520.0, 400.0, 900.0, 760.0,
            ][(key - 60) as usize];
            let mut d = membrane(f, 0.35, if key % 2 == 0 { 0.3 } else { -0.3 });
            d.tone_drive = 0.0;
            d.noise_fc = 2500.0;
            d.gain = 0.55;
            d
        }
        _ => return None,
    };
    if kit == Lofi {
        d.lp = if d.lp > 0.0 { d.lp.min(5500.0) } else { 5500.0 };
        d.grit = 1.0;
        d.drive = d.drive.max(0.6);
        d.metal_bp *= 0.8;
        if d.excl == 1 || d.metal > 0.0 {
            d.gain *= 1.6;
        }
    }
    if kit == Techno && d.excl == 0 && d.tones[0].1 > 0.0 {
        d.drive = d.drive.max(0.4);
    }
    Some(d)
}

// ---------------------------------------------------------------- voice

const MF: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

/// A sounding drum hit.
pub struct DrumVoice {
    rate: f32,
    // tones
    tph: [f32; 4],
    tdt: [f32; 4],
    tamp: [f32; 4],
    tc: [f32; 4],
    pe: f32,
    pc: f32,
    tdrive: f32,
    // noise
    nz: [Noise; 2],
    namp: f32,
    nenv: f32,
    natt: f32,
    nc: f32,
    nac: f32,
    nf: [Svf; 2],
    ncoef: SvfCoef,
    nmode: NMode,
    width: f32,
    burst_left: u32,
    burst_every: u32,
    burst_count: u32,
    benv: f32,
    bc: f32,
    tail: f32,
    tailc: f32,
    // metal
    metal: Lanes,
    menv: f32,
    mc: f32,
    mbp: Svf,
    mcoef: SvfCoef,
    mhp: [OnePole; 2],
    mhg: f32,
    // click
    click: Vec<f32>,
    cpos: usize,
    // post
    drive: f32,
    lp: [[OnePole; 2]; 2],
    lpg: f32,
    hp: [OnePole; 2],
    hpg: f32,
    grit: f32,
    dust: Noise,
    excl: u32,
    out: Out,
    released: bool,
}

impl DrumVoice {
    /// Start a hit of `d` for note `n`.
    pub fn new(d: &Drum, n: &NoteOn) -> DrumVoice {
        let rate = n.rate;
        let v = n.vel.clamp(0.0, 1.0);
        let mut tdt = [0.0; 4];
        let mut tamp = [0.0; 4];
        let mut tc = [0.0; 4];
        for k in 0..4 {
            let (ratio, a, t60) = d.tones[k];
            tdt[k] = (d.freq * ratio / rate).min(0.45);
            tamp[k] = if d.freq * ratio < 0.45 * rate { a } else { 0.0 };
            tc[k] = t60_coef(t60 * (0.85 + 0.15 * v), rate);
        }
        let mut ph = crate::voice::Rng::new(n.seed);
        let mut metal = Lanes::new(Wave::Pulse);
        if d.metal > 0.0 {
            let freqs: &[f32] = if d.metal_n == 2 { &[540.0, 800.0] } else { &MF };
            for (k, f) in freqs.iter().enumerate() {
                metal.ratio[k] = *f;
                metal.gl[k] = 1.0 / (freqs.len() as f32).sqrt();
                metal.gr[k] = metal.gl[k];
                metal.ph[k] = ph.f32();
            }
            // Metallic noise: aliasing is inaudible in it, so no oversampling.
            metal.oversample(false);
            metal.set_freq(d.metal_scale, rate, &[1.0; 8]);
        }
        // Click: a few ms of band-passed noise with a sharp decay.
        let clen = (0.004 * rate) as usize;
        let mut click = Vec::new();
        if d.click > 0.0 {
            click.reserve(clen);
            let mut nz = Noise::new(n.seed ^ 0xC1);
            let mut f = Svf::default();
            let c = SvfCoef::new(prewarp(d.click_fc * (0.6 + 0.6 * v), rate), 0.8);
            for i in 0..clen {
                let e = (-(i as f32) / (0.0007 * rate)).exp();
                click.push(
                    f.bp(nz.next() + if i == 0 { 4.0 } else { 0.0 }, &c)
                        * e
                        * d.click
                        * (0.4 + 0.6 * v)
                        * 2.0,
                );
            }
        }
        let bright = (0.5 * (v - 0.8)).exp2();
        let burst_every = (d.bursts.1 * rate) as u32;
        let has_noise = d.noise > 0.0 || d.tail.0 > 0.0;
        DrumVoice {
            rate,
            tph: [0.0; 4],
            tdt,
            tamp,
            tc,
            pe: (d.pitch.0 - 1.0) * (0.7 + 0.3 * v),
            pc: super::util::pole(d.pitch.1, rate),
            tdrive: d.tone_drive,
            nz: [Noise::new(n.seed ^ 0x11), Noise::new(n.seed ^ 0x22)],
            namp: if has_noise { d.noise } else { 0.0 },
            nenv: 1.0,
            natt: if d.noise_attack > 0.0 { 1.0 } else { 0.0 },
            nc: t60_coef(d.noise_t60, rate),
            nac: super::util::pole(d.noise_attack, rate),
            nf: [Svf::default(); 2],
            ncoef: SvfCoef::new(prewarp((d.noise_fc * bright).min(18000.0), rate), d.noise_q),
            nmode: d.nmode,
            width: d.width,
            burst_left: d.bursts.0.saturating_sub(1),
            burst_every,
            burst_count: burst_every,
            benv: if d.bursts.0 > 0 { 1.0 } else { 0.0 },
            bc: t60_coef(d.bursts.2.max(1e-3), rate),
            tail: d.tail.0,
            tailc: t60_coef(d.tail.1, rate),
            metal,
            menv: d.metal,
            mc: t60_coef(d.metal_t60 * (0.8 + 0.2 * v), rate),
            mbp: Svf::default(),
            mcoef: SvfCoef::new(prewarp(d.metal_bp * bright.sqrt(), rate), 1.1),
            mhp: [OnePole::default(); 2],
            mhg: OnePole::coef(prewarp(d.metal_hp, rate)),
            click,
            cpos: 0,
            drive: d.drive,
            lp: [[OnePole::default(); 2]; 2],
            lpg: if d.lp > 0.0 {
                OnePole::coef(prewarp(d.lp, rate))
            } else {
                0.0
            },
            hp: [OnePole::default(); 2],
            hpg: if d.hp > 0.0 {
                OnePole::coef(prewarp(d.hp, rate))
            } else {
                0.0
            },
            grit: d.grit,
            dust: Noise::new(n.seed ^ 0xD057),
            excl: d.excl,
            out: Out::new(vel_gain(v) * d.gain * 0.7, d.pan),
            released: false,
        }
    }
}

impl Voice for DrumVoice {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if crate::synth::util::avx() {
            // SAFETY: AVX support was checked at run time.
            return unsafe { self.render_avx(l, r) };
        }
        self.render_body(l, r)
    }

    fn release(&mut self) {
        // Drums ring out; note-off is ignored (chokes use `kill`).
        self.released = true;
    }

    fn exclusive(&self) -> u32 {
        self.excl
    }

    fn kill(&mut self) {
        self.out.kill(self.rate);
    }
}

impl DrumVoice {
    /// The render body compiled with AVX (8-wide vector loops).
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx")]
    fn render_avx(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        self.render_body(l, r)
    }

    #[inline(always)]
    fn render_body(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len().min(r.len());
        let mut xl = [0.0f32; crate::voice::BLOCK];
        let mut xr = [0.0f32; crate::voice::BLOCK];
        let mut bound = 0.0f32;
        // Tones with a shared pitch envelope.
        if self.tamp.iter().any(|&a| a > 1e-6) {
            let (mut ph, dt, mut amp, c) = (self.tph, self.tdt, self.tamp, self.tc);
            let (mut pe, pc) = (self.pe, self.pc);
            for x in xl.iter_mut().take(n) {
                let m = 1.0 + pe;
                pe *= pc;
                let mut s = 0.0;
                for k in 0..4 {
                    let mut t = ph[k] + dt[k] * m;
                    t -= (t >= 1.0) as i32 as f32;
                    ph[k] = t;
                    s += sin2pi(t) * amp[k];
                    amp[k] *= c[k];
                }
                *x = s;
            }
            if self.tdrive > 0.0 {
                let d = 1.0 + self.tdrive * 2.0;
                let post = 1.0 / tanh(d);
                for x in xl.iter_mut().take(n) {
                    *x = tanh(*x * d) * post;
                }
            }
            for a in &mut amp {
                if *a < 1e-7 {
                    *a = 0.0;
                }
            }
            bound += amp.iter().sum::<f32>();
            self.tph = ph;
            self.tamp = amp;
            self.pe = flush(pe);
            xr[..n].copy_from_slice(&xl[..n]);
        }
        // Metal bank → band-pass → 2× high-pass.
        if self.menv > 1e-6 {
            let mut sq = [0.0f32; crate::voice::BLOCK];
            self.metal.render_mono(&mut sq[..n]);
            let mut e = self.menv;
            let c = self.mc;
            let g = self.mhg;
            for (i, &s) in sq.iter().enumerate().take(n) {
                let b = self.mbp.bp(s, &self.mcoef);
                let h0 = self.mhp[0].hp(b, g);
                let h = self.mhp[1].hp(h0, g);
                let y = h * e * 2.0;
                e *= c;
                xl[i] += y;
                xr[i] += y;
            }
            self.menv = if e < 1e-7 { 0.0 } else { e };
            bound += e;
        }
        // Noise with bursts, attack and tail.
        if self.namp > 0.0 {
            let (a, w) = (self.namp, self.width);
            let (mut e, mut att, mut be, mut tail) = (self.nenv, self.natt, self.benv, self.tail);
            let (c, ac, bc, tc) = (self.nc, self.nac, self.bc, self.tailc);
            let bursts = self.bc > 0.0 && self.burst_every > 0;
            for i in 0..n {
                if bursts {
                    self.burst_count -= 1;
                    if self.burst_count == 0 {
                        if self.burst_left > 0 {
                            self.burst_left -= 1;
                            be = 1.0;
                        }
                        self.burst_count = self.burst_every;
                    }
                }
                let env = a * if bursts { be } else { e * (1.0 - att) } + tail;
                e *= c;
                att *= ac;
                be *= bc;
                tail *= tc;
                let n0 = self.nz[0].next();
                let n1 = self.nz[1].next();
                let (sl, sr) = (n0, n0 + (n1 - n0) * w);
                let (yl, yr) = match self.nmode {
                    NMode::Lp => (
                        self.nf[0].lp(sl, &self.ncoef),
                        self.nf[1].lp(sr, &self.ncoef),
                    ),
                    NMode::Bp => (
                        self.nf[0].bp(sl, &self.ncoef) * 1.5,
                        self.nf[1].bp(sr, &self.ncoef) * 1.5,
                    ),
                    NMode::Hp => (
                        self.nf[0].hp(sl, &self.ncoef),
                        self.nf[1].hp(sr, &self.ncoef),
                    ),
                };
                xl[i] += yl * env;
                xr[i] += yr * env;
            }
            self.nenv = flush(e);
            self.natt = flush(att);
            self.benv = flush(be);
            self.tail = flush(tail);
            bound += a * e + tail + a * (be + self.burst_left as f32);
            if bound < 1e-7 {
                self.namp = 0.0;
            }
        }
        // Click.
        if self.cpos < self.click.len() {
            let end = (self.cpos + n).min(self.click.len());
            for (i, v) in self.click[self.cpos..end].iter().enumerate() {
                xl[i] += v;
                xr[i] += v;
            }
            self.cpos = end;
            bound += 1.0;
        }
        let (bl, br) = (&mut xl[..n], &mut xr[..n]);
        // Post: drive, filters, grit.
        if self.drive > 0.0 {
            let d = 1.0 + self.drive * 2.0;
            let post = 1.0 / tanh(d);
            for x in bl.iter_mut().chain(br.iter_mut()) {
                *x = tanh(*x * d) * post;
            }
        }
        if self.hpg > 0.0 {
            let g = self.hpg;
            for i in 0..n {
                bl[i] = self.hp[0].hp(bl[i], g);
                br[i] = self.hp[1].hp(br[i], g);
            }
        }
        if self.lpg > 0.0 {
            let g = self.lpg;
            for (c, buf) in [&mut *bl, &mut *br].into_iter().enumerate() {
                let [a, b] = &mut self.lp[c];
                for x in buf.iter_mut() {
                    *x = b.lp(a.lp(*x, g), g);
                }
            }
        }
        if self.grit > 0.0 {
            // 12-bit quantisation plus sparse vinyl dust.
            for i in 0..n {
                let z = self.dust.next();
                let dust = if z > 0.9993 {
                    (z - 0.9993) * 300.0
                } else {
                    0.0
                };
                bl[i] = ((bl[i] * 2048.0) as i32 as f32) * (1.0 / 2048.0) + dust;
                br[i] = ((br[i] * 2048.0) as i32 as f32) * (1.0 / 2048.0) + dust;
            }
        }
        let p = peak(bl).max(peak(br)) * self.out.g;
        self.out.stereo(bl, br, l, r);
        let _ = self.released;
        !((bound * self.out.g < SILENCE * 0.3 && p < SILENCE) || self.out.dead())
    }
}
