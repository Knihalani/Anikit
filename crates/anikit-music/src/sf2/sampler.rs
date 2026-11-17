//! SoundFont (SF2 / SF3) instruments on top of the voice engine.

use super::engine::{cents_to_hz, Env, FilterKind, Lfo, LoopMode, Osc, OscParams, SampleBuf, SamplerVoice};
use super::interp::Interp;
use super::parse::{gen, ModInputs, Region, Sf2};
use crate::voice::{Instrument, NoteOn, Voice};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

/// SoundFont bytes, borrowed from the binary or owned.
pub enum Bytes {
    /// Embedded (e.g. `include_bytes!`).
    Static(&'static [u8]),
    /// Loaded at run time.
    Owned(Vec<u8>),
}

impl Deref for Bytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Bytes::Static(b) => b,
            Bytes::Owned(v) => v,
        }
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(v: Vec<u8>) -> Self {
        Bytes::Owned(v)
    }
}

impl From<&'static [u8]> for Bytes {
    fn from(v: &'static [u8]) -> Self {
        Bytes::Static(v)
    }
}

/// Decode an audio stream (WAV, FLAC, Ogg Vorbis) to planar `f32`.
pub fn decode_audio(src: Box<dyn symphonia::core::io::MediaSource>, ext: Option<&str>) -> Result<(Vec<Vec<f32>>, u32), String> {
    use symphonia::core::audio::{AudioBuffer, Signal};
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let mss = MediaSourceStream::new(src, Default::default());
    let mut hint = Hint::new();
    if let Some(e) = ext {
        hint.with_extension(e);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
        .map_err(|e| format!("unsupported or corrupt audio: {e}"))?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("audio file has no track")?;
    let tid = track.id;
    let rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let nframes = track.codec_params.n_frames;
    let mut dec = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("unsupported codec: {e}"))?;
    let mut chans: Vec<Vec<f32>> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("audio read error: {e}")),
        };
        if packet.track_id() != tid {
            continue;
        }
        let decoded = match dec.decode(&packet) {
            Ok(d) => d,
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("audio decode error: {e}")),
        };
        let spec = *decoded.spec();
        let n = decoded.frames();
        let mut buf = AudioBuffer::<f32>::new(decoded.capacity() as u64, spec);
        decoded.convert(&mut buf);
        let nc = spec.channels.count();
        if chans.is_empty() {
            chans = vec![Vec::new(); nc];
        }
        for (c, ch) in chans.iter_mut().enumerate().take(nc) {
            ch.extend_from_slice(buf.chan(c));
        }
    }
    if chans.is_empty() {
        return Err("audio file decoded to no samples".into());
    }
    if let Some(n) = nframes {
        for c in chans.iter_mut() {
            c.truncate(n as usize);
        }
    }
    Ok((chans, rate))
}

/// Magic of a compact Vorbis record (see [`decode_compact_vorbis`]).
pub const COMPACT_MAGIC: &[u8; 4] = b"AKV1";

/// Read an unsigned LEB128 varint.
pub fn read_varint(b: &[u8], pos: &mut usize) -> Result<u64, String> {
    let mut v = 0u64;
    let mut shift = 0;
    loop {
        let byte = *b.get(*pos).ok_or("truncated compact Vorbis record")?;
        *pos += 1;
        v |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(v);
        }
        shift += 7;
        if shift > 63 {
            return Err("bad varint in compact Vorbis record".into());
        }
    }
}

/// Append an unsigned LEB128 varint.
pub fn write_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// Decode a compact Vorbis record: the Ogg framing and the (large, shared)
/// setup header are factored out so hundreds of short samples do not each
/// carry ~4 KB of codebooks.
///
/// Layout: `"AKV1"`, varint frames, varint ident length + identification
/// header packet, varint offset + length of the setup header packet inside
/// `smpl`, varint packet count, then each audio packet as varint length + bytes.
pub fn decode_compact_vorbis(smpl: &[u8], rec: &[u8]) -> Result<Vec<f32>, String> {
    use symphonia::core::audio::{AudioBuffer, Signal};
    use symphonia::core::codecs::{CodecParameters, DecoderOptions, CODEC_TYPE_VORBIS};
    use symphonia::core::formats::Packet;
    let mut p = 4;
    let frames = read_varint(rec, &mut p)? as usize;
    let il = read_varint(rec, &mut p)? as usize;
    let ident = rec.get(p..p + il).ok_or("truncated ident header")?;
    p += il;
    let so = read_varint(rec, &mut p)? as usize;
    let sl = read_varint(rec, &mut p)? as usize;
    let setup = smpl.get(so..so + sl).ok_or("setup header out of range")?;
    let mut extra = Vec::with_capacity(il + sl);
    extra.extend_from_slice(ident);
    extra.extend_from_slice(setup);
    let mut params = CodecParameters::new();
    params.for_codec(CODEC_TYPE_VORBIS).with_extra_data(extra.into_boxed_slice());
    let mut dec = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions::default())
        .map_err(|e| format!("vorbis: {e}"))?;
    let n = read_varint(rec, &mut p)? as usize;
    let mut out = Vec::with_capacity(frames + 4096);
    let mut buf: Option<AudioBuffer<f32>> = None;
    for _ in 0..n {
        let l = read_varint(rec, &mut p)? as usize;
        let pk = rec.get(p..p + l).ok_or("truncated audio packet")?;
        p += l;
        let decoded = dec.decode(&Packet::new_from_slice(0, 0, 0, pk)).map_err(|e| format!("vorbis: {e}"))?;
        let spec = *decoded.spec();
        let cap = decoded.capacity();
        let b = match &mut buf {
            Some(b) if b.capacity() >= cap => b,
            _ => buf.insert(AudioBuffer::<f32>::new(cap as u64, spec)),
        };
        decoded.convert(b);
        out.extend_from_slice(b.chan(0));
    }
    out.truncate(frames);
    out.resize(frames, 0.0);
    Ok(out)
}

/// Where a sample's data comes from.
pub(crate) enum Source {
    /// 16-bit PCM in `smpl` (+ optional `sm24` low bytes).
    Pcm { off: usize, frames: usize, sm24: Option<usize> },
    /// An Ogg Vorbis stream (SF3) or a compact Vorbis record in `smpl`
    /// (`smpl` = the chunk's payload range, for shared setup headers).
    Vorbis { off: usize, len: usize, smpl: (usize, usize) },
    /// An audio file (SFZ).
    File(PathBuf),
}

/// A lazily decoded sample.
pub(crate) struct SampleSlot {
    pub bytes: Option<Arc<Bytes>>,
    pub src: Source,
    pub rate: u32,
    /// Loop to make seamless after lossy decoding (SF3).
    pub fix_loop: Option<(u32, u32)>,
    pub data: OnceLock<Result<Arc<SampleBuf>, String>>,
}

impl SampleSlot {
    pub fn new(bytes: Option<Arc<Bytes>>, src: Source, rate: u32) -> SampleSlot {
        SampleSlot { bytes, src, rate, fix_loop: None, data: OnceLock::new() }
    }

    /// Decode on first use (thread-safe).
    pub fn get(&self) -> Result<&Arc<SampleBuf>, String> {
        self.data.get_or_init(|| self.decode().map(Arc::new)).as_ref().map_err(|e| e.clone())
    }

    fn decode(&self) -> Result<SampleBuf, String> {
        match &self.src {
            Source::Pcm { off, frames, sm24 } => {
                let b = self.bytes.as_ref().ok_or("no sample data")?;
                let end = (off + frames * 2).min(b.len());
                let pcm = &b[*off..end];
                let n = pcm.len() / 2;
                let mut v = Vec::with_capacity(n);
                match sm24 {
                    Some(lo) if lo + n <= b.len() => {
                        let low = &b[*lo..lo + n];
                        for i in 0..n {
                            let hi = i16::from_le_bytes([pcm[2 * i], pcm[2 * i + 1]]) as i32;
                            v.push(((hi << 8) | low[i] as i32) as f32 * (1.0 / 8_388_608.0));
                        }
                    }
                    _ => {
                        for i in 0..n {
                            v.push(i16::from_le_bytes([pcm[2 * i], pcm[2 * i + 1]]) as f32 * (1.0 / 32_768.0));
                        }
                    }
                }
                Ok(SampleBuf::new(vec![v], self.rate))
            }
            Source::Vorbis { off, len, smpl } => {
                let b = self.bytes.as_ref().ok_or("no sample data")?;
                let end = (off + len).min(b.len());
                let rec = &b[*off..end];
                let mut chans = if rec.starts_with(COMPACT_MAGIC) {
                    let s = &b[smpl.0..(smpl.0 + smpl.1).min(b.len())];
                    vec![decode_compact_vorbis(s, rec)?]
                } else {
                    decode_audio(Box::new(std::io::Cursor::new(rec.to_vec())), Some("ogg"))?.0
                };
                chans.truncate(1);
                if let Some((ls, le)) = self.fix_loop {
                    seam_loop(&mut chans[0], ls as usize, le as usize);
                }
                Ok(SampleBuf::new(chans, self.rate))
            }
            Source::File(p) => {
                let f = std::fs::File::open(p).map_err(|e| format!("cannot open sample {}: {e}", p.display()))?;
                let ext = p.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase());
                let (chans, rate) = decode_audio(Box::new(f), ext.as_deref()).map_err(|e| format!("{}: {e}", p.display()))?;
                let mut buf = SampleBuf::new(chans.into_iter().take(2).collect(), rate);
                if ext.as_deref() == Some("wav") {
                    buf.file_loop = wav_loop(p);
                }
                Ok(buf)
            }
        }
    }
}

/// Lossy coding leaves a small step where the loop wraps. Crossfade the
/// last frames of the loop into the frames that precede the loop start, so
/// `x[le−1] → x[ls]` continues exactly like `x[ls−1] → x[ls]`.
fn seam_loop(x: &mut [f32], ls: usize, le: usize) {
    if le > x.len() || ls >= le {
        return;
    }
    let n = 64.min(ls).min((le - ls) / 2);
    if n < 4 {
        return;
    }
    for k in 0..n {
        let w = (k + 1) as f32 / n as f32; // → 1 at the seam
        let w = 0.5 - 0.5 * (std::f32::consts::PI * w).cos();
        let i = le - n + k;
        let j = ls - n + k;
        x[i] = x[i] * (1.0 - w) + x[j] * w;
    }
}

/// Loop points from a WAV file's `smpl` chunk (first loop).
pub fn wav_loop(p: &Path) -> Option<(u32, u32)> {
    let b = std::fs::read(p).ok()?;
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let mut off = 12;
    while off + 8 <= b.len() {
        let id = &b[off..off + 4];
        let len = u32::from_le_bytes(b[off + 4..off + 8].try_into().ok()?) as usize;
        let p = off + 8;
        if id == b"smpl" && len >= 36 + 24 && p + 60 <= b.len() {
            let n = u32::from_le_bytes(b[p + 28..p + 32].try_into().ok()?);
            if n == 0 {
                return None;
            }
            let l = p + 36;
            let s = u32::from_le_bytes(b[l + 8..l + 12].try_into().ok()?);
            let e = u32::from_le_bytes(b[l + 12..l + 16].try_into().ok()?);
            // smpl loop end is inclusive.
            return (e > s).then_some((s, e + 1));
        }
        off = p + len + (len & 1);
    }
    None
}

/// A parsed SoundFont with lazily decoded samples.
pub struct SoundFont {
    /// The parsed file.
    pub sf: Sf2,
    slots: Vec<SampleSlot>,
    inputs: ModInputs,
    interp: AtomicU8,
}

impl std::fmt::Debug for SoundFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundFont").field("name", &self.sf.name).field("presets", &self.sf.presets.len()).finish()
    }
}

/// `2^(tc/1200)` seconds.
#[inline]
fn tc(v: f32, lo: f32, hi: f32) -> f32 {
    (v.clamp(lo, hi) / 1200.0).exp2()
}

/// Timecents for delays: the minimum means "none".
#[inline]
fn tc_delay(v: f32) -> f32 {
    if v <= -12000.0 {
        0.0
    } else {
        tc(v, -12000.0, 5000.0)
    }
}

impl SoundFont {
    /// Parse a SoundFont 2 or SF3 file held in memory. Samples are decoded on first use.
    pub fn from_bytes(b: impl Into<Bytes>) -> Result<Arc<SoundFont>, String> {
        let bytes = Arc::new(b.into());
        let sf = Sf2::parse(&bytes)?;
        let mut slots = Vec::with_capacity(sf.samples.len());
        for h in &sf.samples {
            let src = if h.is_vorbis() {
                Source::Vorbis {
                    off: sf.data.smpl + h.start as usize,
                    len: h.end.saturating_sub(h.start) as usize,
                    smpl: (sf.data.smpl, sf.data.smpl_len),
                }
            } else {
                let frames = (h.end.min((sf.data.smpl_len / 2) as u32)).saturating_sub(h.start) as usize;
                Source::Pcm {
                    off: sf.data.smpl + 2 * h.start as usize,
                    frames,
                    sm24: sf.data.sm24.map(|(o, _)| o + h.start as usize),
                }
            };
            let mut slot = SampleSlot::new(Some(bytes.clone()), src, h.rate);
            if h.is_vorbis() && h.loop_end > h.loop_start {
                slot.fix_loop = Some((h.loop_start, h.loop_end));
            }
            slots.push(slot);
        }
        Ok(Arc::new(SoundFont { sf, slots, inputs: ModInputs::default(), interp: AtomicU8::new(super::interp::interp() as u8) }))
    }

    /// Read and parse a SoundFont file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Arc<SoundFont>, String> {
        let p = path.as_ref();
        let b = std::fs::read(p).map_err(|e| format!("cannot read {}: {e}", p.display()))?;
        Self::from_bytes(b)
    }

    /// Interpolation for voices started from now on (defaults to the global
    /// [`super::interp::interp`] setting at load time).
    pub fn set_interp(&self, i: Interp) {
        self.interp.store(i as u8, Ordering::Relaxed);
    }

    /// The interpolation this font's voices use.
    pub fn interp(&self) -> Interp {
        if self.interp.load(Ordering::Relaxed) == Interp::Cubic as u8 {
            Interp::Cubic
        } else {
            Interp::Sinc
        }
    }

    /// A decoded sample (decoding it now if needed).
    pub fn sample(&self, index: usize) -> Result<Arc<SampleBuf>, String> {
        self.slots.get(index).ok_or("no such sample")?.get().cloned()
    }

    /// Bank name.
    pub fn name(&self) -> &str {
        &self.sf.name
    }

    /// All presets as (bank, program, name), sorted by bank then program.
    pub fn presets(&self) -> Vec<(u16, u16, String)> {
        self.sf.presets.iter().map(|p| (p.bank, p.program, p.name.clone())).collect()
    }

    /// The instrument for a bank/program pair.
    pub fn instrument(self: &Arc<Self>, bank: u16, program: u16) -> Option<Arc<dyn Instrument>> {
        let i = self.sf.presets.iter().position(|p| p.bank == bank && p.program == program)?;
        Some(Arc::new(Sf2Instrument { font: self.clone(), preset: i }))
    }

    /// The first preset whose name contains `name` (case-insensitive); an
    /// exact match wins, then lower banks.
    pub fn instrument_by_name(self: &Arc<Self>, name: &str) -> Option<Arc<dyn Instrument>> {
        let q = name.trim().to_lowercase();
        let ps = &self.sf.presets;
        let i = ps
            .iter()
            .position(|p| p.name.to_lowercase() == q)
            .or_else(|| ps.iter().position(|p| p.name.to_lowercase().contains(&q)))?;
        Some(Arc::new(Sf2Instrument { font: self.clone(), preset: i }))
    }

    /// Decode every sample now (in parallel) instead of on first use.
    pub fn preload(&self) {
        use rayon::prelude::*;
        self.slots.par_iter().for_each(|s| {
            let _ = s.get();
        });
    }

    /// Decode the samples one preset uses.
    pub fn preload_preset(&self, bank: u16, program: u16) {
        use rayon::prelude::*;
        if let Some(p) = self.sf.preset(bank, program) {
            let mut ids: Vec<usize> = p.regions.iter().map(|r| r.sample).collect();
            ids.sort_unstable();
            ids.dedup();
            ids.par_iter().for_each(|&i| {
                let _ = self.slots[i].get();
            });
        }
    }

    fn osc(&self, r: &Region, note: &NoteOn, key: u8, vel: u8) -> Option<Osc> {
        let mut g = [0f32; gen::COUNT];
        for (d, s) in g.iter_mut().zip(r.gens.iter()) {
            *d = *s as f32;
        }
        let (mut key, mut keyf, mut vel) = (key, note.key, vel);
        if r.gens[gen::KEYNUM] >= 0 {
            key = r.gens[gen::KEYNUM].clamp(0, 127) as u8;
            keyf = key as f32;
        }
        if r.gens[gen::VELOCITY] > 0 {
            vel = r.gens[gen::VELOCITY].clamp(1, 127) as u8;
        }
        for m in &r.mods {
            let d = m.dest as usize;
            if m.dest & 0x8000 == 0 && d < gen::COUNT {
                g[d] += self.inputs.eval(m, key, vel);
            }
        }
        let h = &self.sf.samples[r.sample];
        let buf = self.slots[r.sample].get().ok()?.clone();
        let len = buf.len as i64;
        let gi = |i: usize| g[i].round() as i64; // modulators may move addresses
        let start = (gi(gen::START_ADDRS_OFFSET) + 32768 * gi(gen::START_ADDRS_COARSE_OFFSET)).clamp(0, len);
        let end = (len + gi(gen::END_ADDRS_OFFSET) + 32768 * gi(gen::END_ADDRS_COARSE_OFFSET)).clamp(start, len);
        let ls = (h.loop_start as i64 + gi(gen::STARTLOOP_ADDRS_OFFSET) + 32768 * gi(gen::STARTLOOP_ADDRS_COARSE_OFFSET))
            .clamp(0, len);
        let le = (h.loop_end as i64 + gi(gen::ENDLOOP_ADDRS_OFFSET) + 32768 * gi(gen::ENDLOOP_ADDRS_COARSE_OFFSET))
            .clamp(0, len);
        let loop_mode = match r.gens[gen::SAMPLE_MODES] & 3 {
            1 => LoopMode::Continuous,
            3 => LoopMode::UntilRelease,
            _ => LoopMode::None,
        };
        let root = if r.gens[gen::OVERRIDING_ROOT_KEY] >= 0 {
            r.gens[gen::OVERRIDING_ROOT_KEY] as f32
        } else if h.root <= 127 {
            h.root as f32
        } else {
            60.0
        };
        let cents = (keyf - root) * g[gen::SCALE_TUNING].clamp(0.0, 1200.0)
            + g[gen::COARSE_TUNE].clamp(-120.0, 120.0) * 100.0
            + g[gen::FINE_TUNE].clamp(-99.0, 99.0)
            + h.correction as f32
            + g[gen::INITIAL_PITCH];
        let ratio = (cents as f64 / 1200.0).exp2() * buf.rate as f64 / note.rate as f64;

        let kd = 60.0 - key as f32;
        let vol_env = Env {
            delay: tc_delay(g[gen::DELAY_VOL_ENV]),
            attack: tc(g[gen::ATTACK_VOL_ENV], -12000.0, 8000.0),
            hold: tc(g[gen::HOLD_VOL_ENV] + g[gen::KEYNUM_TO_VOL_ENV_HOLD] * kd, -12000.0, 5000.0),
            decay: tc(g[gen::DECAY_VOL_ENV] + g[gen::KEYNUM_TO_VOL_ENV_DECAY] * kd, -12000.0, 8000.0),
            sustain: g[gen::SUSTAIN_VOL_ENV].clamp(0.0, 1440.0),
            release: tc(g[gen::RELEASE_VOL_ENV], -12000.0, 8000.0),
        };
        let mod_env = Env {
            delay: tc_delay(g[gen::DELAY_MOD_ENV]),
            attack: tc(g[gen::ATTACK_MOD_ENV], -12000.0, 8000.0),
            hold: tc(g[gen::HOLD_MOD_ENV] + g[gen::KEYNUM_TO_MOD_ENV_HOLD] * kd, -12000.0, 5000.0),
            decay: tc(g[gen::DECAY_MOD_ENV] + g[gen::KEYNUM_TO_MOD_ENV_DECAY] * kd, -12000.0, 8000.0),
            sustain: 1.0 - g[gen::SUSTAIN_MOD_ENV].clamp(0.0, 1000.0) / 1000.0,
            release: tc(g[gen::RELEASE_MOD_ENV], -12000.0, 8000.0),
        };
        let lfo = |d: usize, f: usize| Lfo { delay: tc_delay(g[d]), freq: cents_to_hz(g[f].clamp(-16000.0, 4500.0)) };
        let mod_env_fc = g[gen::MOD_ENV_TO_FILTER_FC].clamp(-12000.0, 12000.0);
        let mod_lfo_fc = g[gen::MOD_LFO_TO_FILTER_FC].clamp(-12000.0, 12000.0);
        let fc = g[gen::INITIAL_FILTER_FC].clamp(1500.0, 13500.0);
        let q_cb = g[gen::INITIAL_FILTER_Q].clamp(0.0, 960.0);
        let q = 10f32.powf((q_cb / 10.0 - 3.01) / 20.0);
        let filter_on = fc < 13500.0 || q_cb > 0.0 || mod_env_fc != 0.0 || mod_lfo_fc != 0.0;
        Some(Osc::new(OscParams {
            sample: buf,
            start: start as u32,
            end: end as u32,
            loop_start: ls as u32,
            loop_end: le as u32,
            loop_mode,
            ratio,
            rate: note.rate,
            atten: g[gen::INITIAL_ATTENUATION].clamp(0.0, 1440.0),
            gain: if q > 1.0 { 1.0 / q.sqrt() } else { 1.0 },
            pan: g[gen::PAN].clamp(-500.0, 500.0) / 500.0,
            vol_env,
            mod_env,
            mod_env_pitch: g[gen::MOD_ENV_TO_PITCH].clamp(-12000.0, 12000.0),
            mod_env_fc,
            mod_lfo: lfo(gen::DELAY_MOD_LFO, gen::FREQ_MOD_LFO),
            mod_lfo_pitch: g[gen::MOD_LFO_TO_PITCH].clamp(-12000.0, 12000.0),
            mod_lfo_fc,
            mod_lfo_vol: -g[gen::MOD_LFO_TO_VOLUME].clamp(-960.0, 960.0),
            vib_lfo: lfo(gen::DELAY_VIB_LFO, gen::FREQ_VIB_LFO),
            vib_lfo_pitch: g[gen::VIB_LFO_TO_PITCH].clamp(-12000.0, 12000.0),
            filter: if filter_on { FilterKind::Lp } else { FilterKind::Off },
            fc,
            q,
            exclusive: r.gens[gen::EXCLUSIVE_CLASS].max(0) as u32,
            interp: self.interp(),
        }))
    }
}

/// One SoundFont preset as an [`Instrument`]. Every region that matches the
/// key and velocity sounds (SF2 layers and stereo pairs play together).
pub struct Sf2Instrument {
    font: Arc<SoundFont>,
    preset: usize,
}

impl Sf2Instrument {
    /// Preset name.
    pub fn name(&self) -> &str {
        &self.font.sf.presets[self.preset].name
    }
}

impl Instrument for Sf2Instrument {
    fn voice(&self, note: &NoteOn) -> Box<dyn Voice> {
        let key = note.key.round().clamp(0.0, 127.0) as u8;
        let vel = (note.vel * 127.0).round().clamp(1.0, 127.0) as u8;
        let p = &self.font.sf.presets[self.preset];
        let oscs: Vec<Osc> =
            p.regions.iter().filter(|r| r.matches(key, vel)).filter_map(|r| self.font.osc(r, note, key, vel)).collect();
        Box::new(SamplerVoice::new(oscs, Vec::new()))
    }
}
