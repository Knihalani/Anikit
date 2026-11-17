//! SoundFont 2 / SF3 parser.
//!
//! Reads the RIFF structure (`INFO`, `sdta`, `pdta`) and resolves every
//! preset into flat [`Region`]s: one per (preset zone × instrument zone)
//! pair whose key and velocity ranges intersect, with generators combined
//! as SF2.04 §9.4 prescribes (instrument values are absolute, preset values
//! are added; global zones supply defaults; local zones replace them) and
//! modulators merged as §9.5 prescribes (defaults, overridden by identical
//! instrument modulators; preset modulators add).
//!
//! SF3 follows the MuseScore convention: a sample header whose `sampleType`
//! has bit `0x10` set holds an Ogg Vorbis stream; its `start`/`end` are byte
//! offsets into `smpl`, and its loop points are decoded frames relative to
//! the sample's start.

/// Generator operators (SF2.04 §8.1.2).
#[allow(missing_docs)]
pub mod gen {
    pub const START_ADDRS_OFFSET: usize = 0;
    pub const END_ADDRS_OFFSET: usize = 1;
    pub const STARTLOOP_ADDRS_OFFSET: usize = 2;
    pub const ENDLOOP_ADDRS_OFFSET: usize = 3;
    pub const START_ADDRS_COARSE_OFFSET: usize = 4;
    pub const MOD_LFO_TO_PITCH: usize = 5;
    pub const VIB_LFO_TO_PITCH: usize = 6;
    pub const MOD_ENV_TO_PITCH: usize = 7;
    pub const INITIAL_FILTER_FC: usize = 8;
    pub const INITIAL_FILTER_Q: usize = 9;
    pub const MOD_LFO_TO_FILTER_FC: usize = 10;
    pub const MOD_ENV_TO_FILTER_FC: usize = 11;
    pub const END_ADDRS_COARSE_OFFSET: usize = 12;
    pub const MOD_LFO_TO_VOLUME: usize = 13;
    pub const CHORUS_SEND: usize = 15;
    pub const REVERB_SEND: usize = 16;
    pub const PAN: usize = 17;
    pub const DELAY_MOD_LFO: usize = 21;
    pub const FREQ_MOD_LFO: usize = 22;
    pub const DELAY_VIB_LFO: usize = 23;
    pub const FREQ_VIB_LFO: usize = 24;
    pub const DELAY_MOD_ENV: usize = 25;
    pub const ATTACK_MOD_ENV: usize = 26;
    pub const HOLD_MOD_ENV: usize = 27;
    pub const DECAY_MOD_ENV: usize = 28;
    pub const SUSTAIN_MOD_ENV: usize = 29;
    pub const RELEASE_MOD_ENV: usize = 30;
    pub const KEYNUM_TO_MOD_ENV_HOLD: usize = 31;
    pub const KEYNUM_TO_MOD_ENV_DECAY: usize = 32;
    pub const DELAY_VOL_ENV: usize = 33;
    pub const ATTACK_VOL_ENV: usize = 34;
    pub const HOLD_VOL_ENV: usize = 35;
    pub const DECAY_VOL_ENV: usize = 36;
    pub const SUSTAIN_VOL_ENV: usize = 37;
    pub const RELEASE_VOL_ENV: usize = 38;
    pub const KEYNUM_TO_VOL_ENV_HOLD: usize = 39;
    pub const KEYNUM_TO_VOL_ENV_DECAY: usize = 40;
    pub const INSTRUMENT: usize = 41;
    pub const KEY_RANGE: usize = 43;
    pub const VEL_RANGE: usize = 44;
    pub const STARTLOOP_ADDRS_COARSE_OFFSET: usize = 45;
    pub const KEYNUM: usize = 46;
    pub const VELOCITY: usize = 47;
    pub const INITIAL_ATTENUATION: usize = 48;
    pub const ENDLOOP_ADDRS_COARSE_OFFSET: usize = 50;
    pub const COARSE_TUNE: usize = 51;
    pub const FINE_TUNE: usize = 52;
    pub const SAMPLE_ID: usize = 53;
    pub const SAMPLE_MODES: usize = 54;
    pub const SCALE_TUNING: usize = 56;
    pub const EXCLUSIVE_CLASS: usize = 57;
    pub const OVERRIDING_ROOT_KEY: usize = 58;
    /// Not a stored generator: the destination the default pitch-wheel
    /// modulator targets ("initialPitch", cents).
    pub const INITIAL_PITCH: usize = 59;
    /// Number of generator slots.
    pub const COUNT: usize = 61;
}

/// Default generator values (SF2.04 §8.1.3).
pub fn default_gens() -> [i32; gen::COUNT] {
    let mut g = [0i32; gen::COUNT];
    g[gen::INITIAL_FILTER_FC] = 13500;
    for i in [
        gen::DELAY_MOD_LFO,
        gen::DELAY_VIB_LFO,
        gen::DELAY_MOD_ENV,
        gen::ATTACK_MOD_ENV,
        gen::HOLD_MOD_ENV,
        gen::DECAY_MOD_ENV,
        gen::RELEASE_MOD_ENV,
        gen::DELAY_VOL_ENV,
        gen::ATTACK_VOL_ENV,
        gen::HOLD_VOL_ENV,
        gen::DECAY_VOL_ENV,
        gen::RELEASE_VOL_ENV,
    ] {
        g[i] = -12000;
    }
    g[gen::KEY_RANGE] = 127 << 8;
    g[gen::VEL_RANGE] = 127 << 8;
    g[gen::KEYNUM] = -1;
    g[gen::VELOCITY] = -1;
    g[gen::SCALE_TUNING] = 100;
    g[gen::OVERRIDING_ROOT_KEY] = -1;
    g
}

/// Generators that may not appear at preset level (§8.1.2: "instrument only").
fn instrument_only(op: usize) -> bool {
    matches!(
        op,
        gen::START_ADDRS_OFFSET
            | gen::END_ADDRS_OFFSET
            | gen::STARTLOOP_ADDRS_OFFSET
            | gen::ENDLOOP_ADDRS_OFFSET
            | gen::START_ADDRS_COARSE_OFFSET
            | gen::END_ADDRS_COARSE_OFFSET
            | gen::STARTLOOP_ADDRS_COARSE_OFFSET
            | gen::ENDLOOP_ADDRS_COARSE_OFFSET
            | gen::KEYNUM
            | gen::VELOCITY
            | gen::SAMPLE_MODES
            | gen::EXCLUSIVE_CLASS
            | gen::OVERRIDING_ROOT_KEY
    )
}

/// A modulator record (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modulator {
    /// Primary source enumerator.
    pub src: u16,
    /// Destination generator (bit 15 = link to another modulator).
    pub dest: u16,
    /// Amount.
    pub amount: i16,
    /// Amount (secondary) source enumerator.
    pub amt_src: u16,
    /// Transform (0 = linear, 2 = absolute value).
    pub trans: u16,
}

impl Modulator {
    fn same_identity(&self, o: &Modulator) -> bool {
        self.src == o.src && self.dest == o.dest && self.amt_src == o.amt_src && self.trans == o.trans
    }
}

/// The SF2.04 default modulators (§8.4) that can matter for an offline
/// renderer. Controller defaults are evaluated by [`ModInputs`].
pub const DEFAULT_MODULATORS: &[Modulator] = &[
    // 8.4.1 velocity → initial attenuation (negative concave, 960 cB)
    Modulator { src: 0x0502, dest: gen::INITIAL_ATTENUATION as u16, amount: 960, amt_src: 0, trans: 0 },
    // 8.4.2 velocity → filter cutoff (negative linear, −2400 cents)
    Modulator { src: 0x0102, dest: gen::INITIAL_FILTER_FC as u16, amount: -2400, amt_src: 0, trans: 0 },
    // 8.4.3 channel pressure → vibrato LFO pitch depth (50 cents)
    Modulator { src: 0x000D, dest: gen::VIB_LFO_TO_PITCH as u16, amount: 50, amt_src: 0, trans: 0 },
    // 8.4.4 CC1 (mod wheel) → vibrato LFO pitch depth (50 cents)
    Modulator { src: 0x0081, dest: gen::VIB_LFO_TO_PITCH as u16, amount: 50, amt_src: 0, trans: 0 },
    // 8.4.5 CC7 (volume) → attenuation
    Modulator { src: 0x0587, dest: gen::INITIAL_ATTENUATION as u16, amount: 960, amt_src: 0, trans: 0 },
    // 8.4.6 CC10 (pan) → pan
    Modulator { src: 0x028A, dest: gen::PAN as u16, amount: 1000, amt_src: 0, trans: 0 },
    // 8.4.7 CC11 (expression) → attenuation
    Modulator { src: 0x058B, dest: gen::INITIAL_ATTENUATION as u16, amount: 960, amt_src: 0, trans: 0 },
    // 8.4.8 CC91 → reverb send, 8.4.9 CC93 → chorus send
    Modulator { src: 0x00DB, dest: gen::REVERB_SEND as u16, amount: 200, amt_src: 0, trans: 0 },
    Modulator { src: 0x00DD, dest: gen::CHORUS_SEND as u16, amount: 200, amt_src: 0, trans: 0 },
    // 8.4.10 pitch wheel → initial pitch, scaled by pitch-wheel sensitivity
    Modulator { src: 0x020E, dest: gen::INITIAL_PITCH as u16, amount: 12700, amt_src: 0x0010, trans: 0 },
];

/// Controller state used to evaluate modulators at note-on. The renderer is
/// offline with no live controllers, so these are the MIDI reset defaults
/// (volume and expression fully up, pan centred, wheel centred).
#[derive(Debug, Clone)]
pub struct ModInputs {
    /// MIDI CC values 0..=127.
    pub cc: [u8; 128],
    /// 14-bit pitch wheel (8192 = centre).
    pub pitch_wheel: u16,
    /// Pitch-wheel sensitivity in semitones.
    pub wheel_range: u8,
    /// Channel pressure 0..=127.
    pub channel_pressure: u8,
}

impl Default for ModInputs {
    fn default() -> Self {
        let mut cc = [0u8; 128];
        cc[7] = 127;
        cc[10] = 64;
        cc[11] = 127;
        cc[8] = 64; // balance
        ModInputs { cc, pitch_wheel: 8192, wheel_range: 2, channel_pressure: 0 }
    }
}

#[inline]
fn concave(x: f32) -> f32 {
    if x >= 1.0 {
        1.0
    } else if x <= 0.0 {
        0.0
    } else {
        // −20/96 · log10((1 − x)²), §8.2.1 type 1
        (-(40.0 / 96.0) * (1.0 - x).log10()).min(1.0)
    }
}

#[inline]
fn convex(x: f32) -> f32 {
    1.0 - concave(1.0 - x)
}

impl ModInputs {
    /// Value of a source enumerator for a note.
    pub fn source(&self, src: u16, key: u8, vel: u8) -> f32 {
        let index = (src & 0x7f) as usize;
        let cc = src & 0x80 != 0;
        let dir = src & 0x100 != 0;
        let bipolar = src & 0x200 != 0;
        let kind = src >> 10;
        // 7-bit sources: unipolar maps 0..127 onto 0..1 exactly; bipolar
        // divides by 128 so the centre (64, 8192) maps exactly to 0.5.
        let r7 = if bipolar { 128.0 } else { 127.0 };
        let (raw, range) = if cc {
            (self.cc[index] as f32, r7)
        } else {
            match index {
                0 => return 1.0, // no controller
                2 => (vel as f32, r7),
                3 => (key as f32, r7),
                10 => (0.0, r7),
                13 => (self.channel_pressure as f32, r7),
                14 => (self.pitch_wheel as f32, 16384.0),
                16 => (self.wheel_range as f32, r7),
                _ => return 0.0, // link (127) and unknown sources
            }
        };
        let mut x = (raw / range).min(1.0);
        if dir {
            x = 1.0 - x;
        }
        match (kind, bipolar) {
            (0, false) => x,
            (0, true) => 2.0 * x - 1.0,
            (1, false) => concave(x),
            (1, true) => {
                if x > 0.5 {
                    concave(2.0 * x - 1.0)
                } else {
                    -concave(1.0 - 2.0 * x)
                }
            }
            (2, false) => convex(x),
            (2, true) => {
                if x > 0.5 {
                    convex(2.0 * x - 1.0)
                } else {
                    -convex(1.0 - 2.0 * x)
                }
            }
            (3, false) => {
                if x >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            (3, true) => {
                if x >= 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            _ => 0.0,
        }
    }

    /// Contribution of a modulator to its destination.
    pub fn eval(&self, m: &Modulator, key: u8, vel: u8) -> f32 {
        let mut v = m.amount as f32 * self.source(m.src, key, vel) * self.source(m.amt_src, key, vel);
        if m.trans == 2 {
            v = v.abs();
        }
        v
    }
}

/// A sample header (§7.10). Addresses are in sample frames relative to the
/// sample's own start (for SF2 as well as SF3), so the sampler can address
/// every sample's decoded buffer the same way.
#[derive(Debug, Clone)]
pub struct SampleHeader {
    /// Name.
    pub name: String,
    /// SF2: first frame index in `smpl`. SF3: byte offset of the Ogg stream.
    pub start: u32,
    /// SF2: one past the last frame. SF3: byte end of the Ogg stream.
    pub end: u32,
    /// Loop start, frames relative to the sample's start.
    pub loop_start: u32,
    /// Loop end (exclusive), frames relative to the sample's start.
    pub loop_end: u32,
    /// Sample rate in Hz.
    pub rate: u32,
    /// MIDI key of the recorded pitch.
    pub root: u8,
    /// Pitch correction in cents.
    pub correction: i8,
    /// Linked sample index (stereo partner).
    pub link: u16,
    /// Sample type bits (1 mono, 2 right, 4 left, 8 linked, 0x10 Vorbis, 0x8000 ROM).
    pub kind: u16,
}

impl SampleHeader {
    /// Whether the sample data is an Ogg Vorbis stream (SF3).
    pub fn is_vorbis(&self) -> bool {
        self.kind & 0x10 != 0
    }
    /// Length in frames for PCM samples (unknown for Vorbis before decoding).
    pub fn pcm_len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }
}

/// A playable zone: one sample with its fully combined generators.
#[derive(Debug, Clone)]
pub struct Region {
    /// Inclusive key range.
    pub keys: (u8, u8),
    /// Inclusive velocity range.
    pub vels: (u8, u8),
    /// Index into [`Sf2::samples`].
    pub sample: usize,
    /// Combined generator values (instrument absolute + preset relative).
    pub gens: [i32; gen::COUNT],
    /// Instrument modulators (defaults merged with the zone's) followed by
    /// preset modulators; all contributions are summed.
    pub mods: Vec<Modulator>,
}

impl Region {
    /// Whether a key/velocity pair falls in this region.
    #[inline]
    pub fn matches(&self, key: u8, vel: u8) -> bool {
        key >= self.keys.0 && key <= self.keys.1 && vel >= self.vels.0 && vel <= self.vels.1
    }
}

/// A resolved preset.
#[derive(Debug, Clone)]
pub struct Preset {
    /// Preset name.
    pub name: String,
    /// MIDI bank (128 = percussion).
    pub bank: u16,
    /// MIDI program.
    pub program: u16,
    /// Flattened regions.
    pub regions: Vec<Region>,
}

/// Where the sample data lives inside the file.
#[derive(Debug, Clone, Copy, Default)]
pub struct SampleData {
    /// Byte offset of `smpl` payload.
    pub smpl: usize,
    /// Byte length of `smpl` payload.
    pub smpl_len: usize,
    /// Byte offset/length of `sm24` payload (24-bit low bytes), if present and valid.
    pub sm24: Option<(usize, usize)>,
}

/// A parsed SoundFont file.
#[derive(Debug, Clone)]
pub struct Sf2 {
    /// Bank name (`INAM`).
    pub name: String,
    /// Format version (`ifil`), e.g. (2, 1) or (3, 0).
    pub version: (u16, u16),
    /// Other INFO strings (chunk id, text).
    pub info: Vec<(String, String)>,
    /// Presets, sorted by (bank, program).
    pub presets: Vec<Preset>,
    /// Sample headers (without the terminal EOS record).
    pub samples: Vec<SampleHeader>,
    /// Location of the sample data.
    pub data: SampleData,
}

fn err<T>(msg: impl Into<String>) -> Result<T, String> {
    Err(msg.into())
}

#[inline]
fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
#[inline]
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn name20(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim_end().to_string()
}

/// A raw RIFF chunk: id, payload offset, payload length.
#[derive(Debug, Clone, Copy)]
pub struct Chunk {
    /// Four-character id.
    pub id: [u8; 4],
    /// Payload offset in the file.
    pub off: usize,
    /// Payload length.
    pub len: usize,
}

/// Iterate the sub-chunks of a payload region.
pub fn chunks(b: &[u8], mut off: usize, end: usize) -> Result<Vec<Chunk>, String> {
    let mut out = Vec::new();
    let end = end.min(b.len());
    while off + 8 <= end {
        let id = [b[off], b[off + 1], b[off + 2], b[off + 3]];
        let len = u32le(b, off + 4) as usize;
        let p = off + 8;
        if p + len > end {
            // Tolerate a truncated final chunk (seen in the wild) by clamping.
            out.push(Chunk { id, off: p, len: end - p });
            break;
        }
        out.push(Chunk { id, off: p, len });
        off = p + len + (len & 1);
    }
    Ok(out)
}

/// Top-level `LIST` chunks of an SF2 file: (INFO, sdta, pdta) payload ranges.
pub fn lists(b: &[u8]) -> Result<Vec<(String, Chunk)>, String> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"sfbk" {
        return err("not a SoundFont (missing RIFF/sfbk header)");
    }
    let riff_len = u32le(b, 4) as usize;
    let end = (8 + riff_len).min(b.len());
    let mut out = Vec::new();
    for c in chunks(b, 12, end)? {
        if &c.id == b"LIST" && c.len >= 4 {
            let kind = String::from_utf8_lossy(&b[c.off..c.off + 4]).to_string();
            out.push((kind, Chunk { id: c.id, off: c.off + 4, len: c.len - 4 }));
        }
    }
    Ok(out)
}

struct Hydra<'a> {
    phdr: &'a [u8],
    pbag: &'a [u8],
    pmod: &'a [u8],
    pgen: &'a [u8],
    inst: &'a [u8],
    ibag: &'a [u8],
    imod: &'a [u8],
    igen: &'a [u8],
    shdr: &'a [u8],
}

/// A zone as read from the file: generators set explicitly plus modulators.
#[derive(Debug, Clone, Default)]
struct Zone {
    set: Vec<(usize, i32)>,
    mods: Vec<Modulator>,
}

impl Zone {
    fn get(&self, op: usize) -> Option<i32> {
        self.set.iter().rev().find(|(o, _)| *o == op).map(|&(_, v)| v)
    }
}

fn gen_value(op: usize, raw: [u8; 2]) -> i32 {
    match op {
        gen::KEY_RANGE | gen::VEL_RANGE => ((raw[1] as i32) << 8) | raw[0] as i32,
        gen::INSTRUMENT | gen::SAMPLE_ID | gen::SAMPLE_MODES => u16::from_le_bytes(raw) as i32,
        _ => i16::from_le_bytes(raw) as i32,
    }
}

/// Read the zones of item `i` (preset or instrument) given its bag index range.
fn read_zones(bag: &[u8], gens: &[u8], mods: &[u8], b0: usize, b1: usize) -> Vec<Zone> {
    let nbag = bag.len() / 4;
    let ngen = gens.len() / 4;
    let nmod = mods.len() / 10;
    let mut zones = Vec::new();
    for z in b0..b1.min(nbag.saturating_sub(1)) {
        let (g0, m0) = (u16le(bag, z * 4) as usize, u16le(bag, z * 4 + 2) as usize);
        let (g1, m1) = (u16le(bag, z * 4 + 4) as usize, u16le(bag, z * 4 + 6) as usize);
        let mut zone = Zone::default();
        for g in g0..g1.min(ngen) {
            let op = u16le(gens, g * 4) as usize;
            if op >= gen::COUNT {
                continue;
            }
            let raw = [gens[g * 4 + 2], gens[g * 4 + 3]];
            zone.set.push((op, gen_value(op, raw)));
        }
        for m in m0..m1.min(nmod) {
            let o = m * 10;
            zone.mods.push(Modulator {
                src: u16le(mods, o),
                dest: u16le(mods, o + 2),
                amount: u16le(mods, o + 4) as i16,
                amt_src: u16le(mods, o + 6),
                trans: u16le(mods, o + 8),
            });
        }
        zones.push(zone);
    }
    zones
}

/// Overlay modulators: an identical modulator replaces, others are appended (§9.5.1).
fn overlay(list: &mut Vec<Modulator>, add: &[Modulator]) {
    for m in add {
        if let Some(e) = list.iter_mut().find(|e| e.same_identity(m)) {
            *e = *m;
        } else {
            list.push(*m);
        }
    }
}

fn range(v: i32) -> (u8, u8) {
    let lo = (v & 0xff) as u8;
    let hi = ((v >> 8) & 0xff) as u8;
    (lo.min(127), hi.min(127))
}

fn intersect(a: (u8, u8), b: (u8, u8)) -> Option<(u8, u8)> {
    let lo = a.0.max(b.0);
    let hi = a.1.min(b.1);
    (lo <= hi).then_some((lo, hi))
}

impl Sf2 {
    /// Parse a SoundFont 2 or SF3 file.
    pub fn parse(b: &[u8]) -> Result<Sf2, String> {
        let lists = lists(b)?;
        let find = |k: &str| lists.iter().find(|(n, _)| n == k).map(|(_, c)| *c);
        let info = find("INFO");
        let sdta = find("sdta").ok_or("SoundFont has no sdta list")?;
        let pdta = find("pdta").ok_or("SoundFont has no pdta list")?;

        let mut name = String::new();
        let mut version = (2, 1);
        let mut infos = Vec::new();
        if let Some(info) = info {
            for c in chunks(b, info.off, info.off + info.len)? {
                let id = String::from_utf8_lossy(&c.id).to_string();
                let p = &b[c.off..c.off + c.len];
                if &c.id == b"ifil" && c.len >= 4 {
                    version = (u16le(p, 0), u16le(p, 2));
                } else {
                    let end = p.iter().position(|&x| x == 0).unwrap_or(p.len());
                    let text = String::from_utf8_lossy(&p[..end]).to_string();
                    if &c.id == b"INAM" {
                        name = text.clone();
                    }
                    infos.push((id, text));
                }
            }
        }

        let mut data = SampleData::default();
        for c in chunks(b, sdta.off, sdta.off + sdta.len)? {
            match &c.id {
                b"smpl" => {
                    data.smpl = c.off;
                    data.smpl_len = c.len;
                }
                b"sm24" => data.sm24 = Some((c.off, c.len)),
                _ => {}
            }
        }
        // sm24 must hold one byte per smpl frame (rounded to even) to be usable.
        if let Some((_, l)) = data.sm24 {
            let frames = data.smpl_len / 2;
            if l < frames || version.0 >= 3 {
                data.sm24 = None;
            }
        }

        let pchunks = chunks(b, pdta.off, pdta.off + pdta.len)?;
        let get = |id: &[u8; 4], rec: usize| -> Result<&[u8], String> {
            let c = pchunks
                .iter()
                .find(|c| &c.id == id)
                .ok_or_else(|| format!("SoundFont pdta has no {} chunk", String::from_utf8_lossy(id)))?;
            let len = c.len / rec * rec;
            Ok(&b[c.off..c.off + len])
        };
        let h = Hydra {
            phdr: get(b"phdr", 38)?,
            pbag: get(b"pbag", 4)?,
            pmod: get(b"pmod", 10)?,
            pgen: get(b"pgen", 4)?,
            inst: get(b"inst", 22)?,
            ibag: get(b"ibag", 4)?,
            imod: get(b"imod", 10)?,
            igen: get(b"igen", 4)?,
            shdr: get(b"shdr", 46)?,
        };

        // Samples (drop the terminal "EOS" record).
        let nsamp = (h.shdr.len() / 46).saturating_sub(1);
        let mut samples = Vec::with_capacity(nsamp);
        for i in 0..nsamp {
            let r = &h.shdr[i * 46..i * 46 + 46];
            let kind = u16le(r, 44);
            let start = u32le(r, 20);
            let end = u32le(r, 24);
            let mut ls = u32le(r, 28);
            let mut le = u32le(r, 32);
            if kind & 0x10 == 0 {
                // SF2: loop points are absolute; make them relative.
                ls = ls.saturating_sub(start);
                le = le.saturating_sub(start);
            }
            samples.push(SampleHeader {
                name: name20(&r[0..20]),
                start,
                end,
                loop_start: ls,
                loop_end: le,
                rate: u32le(r, 36).max(1),
                root: r[40],
                correction: r[41] as i8,
                link: u16le(r, 42),
                kind,
            });
        }

        // Instruments → zones (global zone first if it has no sampleID).
        let ninst = (h.inst.len() / 22).saturating_sub(1);
        let mut instruments: Vec<(Option<Zone>, Vec<Zone>)> = Vec::with_capacity(ninst);
        for i in 0..ninst {
            let b0 = u16le(h.inst, i * 22 + 20) as usize;
            let b1 = u16le(h.inst, i * 22 + 42) as usize;
            let mut zones = read_zones(h.ibag, h.igen, h.imod, b0, b1);
            let mut global = None;
            if !zones.is_empty() && zones[0].get(gen::SAMPLE_ID).is_none() {
                global = Some(zones.remove(0));
            }
            zones.retain(|z| z.get(gen::SAMPLE_ID).is_some_and(|s| (s as usize) < nsamp));
            instruments.push((global, zones));
        }

        // Presets.
        let npre = (h.phdr.len() / 38).saturating_sub(1);
        let mut presets = Vec::with_capacity(npre);
        for p in 0..npre {
            let r = &h.phdr[p * 38..p * 38 + 38];
            let b0 = u16le(r, 24) as usize;
            let b1 = u16le(h.phdr, (p + 1) * 38 + 24) as usize;
            let mut zones = read_zones(h.pbag, h.pgen, h.pmod, b0, b1);
            let mut pglobal = Zone::default();
            if !zones.is_empty() && zones[0].get(gen::INSTRUMENT).is_none() {
                pglobal = zones.remove(0);
            }
            let mut regions = Vec::new();
            for pz in zones.iter() {
                let Some(ii) = pz.get(gen::INSTRUMENT).map(|v| v as usize) else { continue };
                let Some((iglobal, izones)) = instruments.get(ii) else { continue };
                // Preset zone: relative values, local replaces global.
                let mut prel = [0i32; gen::COUNT];
                let mut pset = [false; gen::COUNT];
                for &(op, v) in pglobal.set.iter().chain(pz.set.iter()) {
                    prel[op] = v;
                    pset[op] = true;
                }
                let pkeys = if pset[gen::KEY_RANGE] { range(prel[gen::KEY_RANGE]) } else { (0, 127) };
                let pvels = if pset[gen::VEL_RANGE] { range(prel[gen::VEL_RANGE]) } else { (0, 127) };
                let mut pmods = pglobal.mods.clone();
                overlay(&mut pmods, &pz.mods);

                for iz in izones {
                    let mut g = default_gens();
                    if let Some(gz) = iglobal {
                        for &(op, v) in &gz.set {
                            g[op] = v;
                        }
                    }
                    for &(op, v) in &iz.set {
                        g[op] = v;
                    }
                    let Some(keys) = intersect(pkeys, range(g[gen::KEY_RANGE])) else { continue };
                    let Some(vels) = intersect(pvels, range(g[gen::VEL_RANGE])) else { continue };
                    for op in 0..gen::COUNT {
                        if pset[op]
                            && !instrument_only(op)
                            && !matches!(op, gen::KEY_RANGE | gen::VEL_RANGE | gen::INSTRUMENT | gen::SAMPLE_ID)
                        {
                            g[op] += prel[op];
                        }
                    }
                    let mut mods = DEFAULT_MODULATORS.to_vec();
                    if let Some(gz) = iglobal {
                        overlay(&mut mods, &gz.mods);
                    }
                    overlay(&mut mods, &iz.mods);
                    mods.extend_from_slice(&pmods);
                    // Drop modulators that can never contribute (amount 0).
                    mods.retain(|m| m.amount != 0);
                    regions.push(Region { keys, vels, sample: g[gen::SAMPLE_ID] as usize, gens: g, mods });
                }
            }
            presets.push(Preset { name: name20(&r[0..20]), program: u16le(r, 20), bank: u16le(r, 22), regions });
        }
        presets.sort_by_key(|p| (p.bank, p.program));

        Ok(Sf2 { name, version, info: infos, presets, samples, data })
    }

    /// Find a preset by bank and program.
    pub fn preset(&self, bank: u16, program: u16) -> Option<&Preset> {
        self.presets.iter().find(|p| p.bank == bank && p.program == program)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sf2_concave_law() {
        let m = ModInputs::default();
        // Velocity 64 → −400·log10(64/127) cB = 119.05 cB.
        let v = m.eval(&DEFAULT_MODULATORS[0], 60, 64);
        assert!((v - 119.05).abs() < 0.2, "{v}");
        // Full velocity → ~0.
        assert!(m.eval(&DEFAULT_MODULATORS[0], 60, 127).abs() < 1e-3);
        // Default controller modulators contribute nothing.
        for d in &DEFAULT_MODULATORS[2..] {
            assert!(m.eval(d, 60, 100).abs() < 1e-3, "{d:?}");
        }
    }

    #[test]
    fn sf2_rejects_garbage() {
        assert!(Sf2::parse(b"RIFF\0\0\0\0WAVE").is_err());
        assert!(Sf2::parse(&[]).is_err());
    }
}
