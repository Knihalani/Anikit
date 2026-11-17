//! Standard MIDI File import and export.

use crate::notes::Note;
use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

/// A MIDI track as notes in beats.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MidiTrack {
    pub name: String,
    /// 0-based MIDI channel (9 = drums).
    pub channel: u8,
    /// Program (0-based GM program) from the first program change, if any.
    pub program: Option<u8>,
    pub notes: Vec<Note>,
}

/// An imported MIDI file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MidiSong {
    /// Tempo of the first tempo event (default 120).
    pub bpm: f64,
    /// Beats per bar from the first time signature (default 4).
    pub beats_per_bar: f64,
    pub tracks: Vec<MidiTrack>,
}

/// Parse a Standard MIDI File (format 0 or 1). Tracks are split by channel.
pub fn import(bytes: &[u8]) -> Result<MidiSong, String> {
    let smf = Smf::parse(bytes).map_err(|e| format!("MIDI: {e}"))?;
    let ppq = match smf.header.timing {
        Timing::Metrical(t) => t.as_int() as f64,
        Timing::Timecode(..) => return Err("MIDI: SMPTE timecode files are not supported (use PPQ timing)".into()),
    };
    let mut bpm = None;
    let mut bpb = None;
    let mut out: Vec<MidiTrack> = Vec::new();
    for track in &smf.tracks {
        let mut tick: u64 = 0;
        let mut name = String::new();
        let mut on: Vec<Option<(u64, u8)>> = vec![None; 16 * 128];
        let mut per_ch: Vec<MidiTrack> = (0..16).map(|c| MidiTrack { channel: c, ..Default::default() }).collect();
        for ev in track {
            tick += ev.delta.as_int() as u64;
            match ev.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(t)) if bpm.is_none() => bpm = Some(60_000_000.0 / t.as_int() as f64),
                TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, _, _)) if bpb.is_none() => {
                    bpb = Some(n as f64 * 4.0 / 2f64.powi(d as i32));
                }
                TrackEventKind::Meta(MetaMessage::TrackName(n)) => name = String::from_utf8_lossy(n).into_owned(),
                TrackEventKind::Midi { channel, message } => {
                    let ch = channel.as_int() as usize;
                    let mut end = |key: u8, tick: u64, per: &mut Vec<MidiTrack>| {
                        if let Some((t0, v)) = on[ch * 128 + key as usize].take() {
                            per[ch].notes.push(Note {
                                t: t0 as f64 / ppq,
                                len: (tick - t0) as f64 / ppq,
                                key: key as f32,
                                vel: v as f32 / 127.0,
                            });
                        }
                    };
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            end(key.as_int(), tick, &mut per_ch);
                            on[ch * 128 + key.as_int() as usize] = Some((tick, vel.as_int()));
                        }
                        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => end(key.as_int(), tick, &mut per_ch),
                        MidiMessage::ProgramChange { program } => {
                            per_ch[ch].program.get_or_insert(program.as_int());
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        for mut t in per_ch.into_iter().filter(|t| !t.notes.is_empty()) {
            t.notes.sort_by(|a, b| a.t.total_cmp(&b.t));
            t.name = if name.is_empty() { format!("ch{}", t.channel + 1) } else { name.clone() };
            out.push(t);
        }
    }
    Ok(MidiSong { bpm: bpm.unwrap_or(120.0), beats_per_bar: bpb.unwrap_or(4.0), tracks: out })
}

/// Write a format-1 Standard MIDI File (480 PPQ). Track 0 holds tempo and meter.
pub fn export(song: &MidiSong) -> Vec<u8> {
    const PPQ: f64 = 480.0;
    let mut tracks: Vec<Vec<TrackEvent>> = Vec::new();
    let tempo = (60_000_000.0 / song.bpm.max(1.0)).round() as u32;
    let (num, den_pow) = match song.beats_per_bar {
        b if (b - 3.0).abs() < 1e-9 => (3u8, 2u8),
        b if (b - 1.5).abs() < 1e-9 => (3, 3),
        b if (b - 3.0 / 2.0 * 2.0).abs() < 1e-9 => (6, 3),
        b if b.fract() == 0.0 && b > 0.0 => (b as u8, 2),
        b if (b * 2.0).fract() == 0.0 => ((b * 2.0) as u8, 3),
        _ => (4, 2),
    };
    tracks.push(vec![
        TrackEvent { delta: 0.into(), kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(tempo))) },
        TrackEvent { delta: 0.into(), kind: TrackEventKind::Meta(MetaMessage::TimeSignature(num, den_pow, 24, 8)) },
        TrackEvent { delta: 0.into(), kind: TrackEventKind::Meta(MetaMessage::EndOfTrack) },
    ]);
    let names: Vec<Vec<u8>> = song.tracks.iter().map(|t| t.name.as_bytes().to_vec()).collect();
    for (t, name) in song.tracks.iter().zip(&names) {
        let ch = u4::new(t.channel.min(15));
        // (tick, order, event): offs sort before ons at the same tick.
        let mut evs: Vec<(u64, u8, MidiMessage)> = Vec::new();
        for n in &t.notes {
            let k = u7::new((n.key.round() as i32).clamp(0, 127) as u8);
            let v = u7::new(((n.vel * 127.0).round() as i32).clamp(1, 127) as u8);
            let t0 = (n.t * PPQ).round().max(0.0) as u64;
            let t1 = ((n.t + n.len) * PPQ).round().max(t0 as f64 + 1.0) as u64;
            evs.push((t0, 1, MidiMessage::NoteOn { key: k, vel: v }));
            evs.push((t1, 0, MidiMessage::NoteOff { key: k, vel: u7::new(0) }));
        }
        evs.sort_by_key(|e| (e.0, e.1));
        let mut tr = vec![TrackEvent { delta: 0.into(), kind: TrackEventKind::Meta(MetaMessage::TrackName(name)) }];
        if let Some(p) = t.program {
            tr.push(TrackEvent { delta: 0.into(), kind: TrackEventKind::Midi { channel: ch, message: MidiMessage::ProgramChange { program: u7::new(p.min(127)) } } });
        }
        let mut last = 0u64;
        for (tick, _, m) in evs {
            tr.push(TrackEvent { delta: u28::new((tick - last) as u32), kind: TrackEventKind::Midi { channel: ch, message: m } });
            last = tick;
        }
        tr.push(TrackEvent { delta: 0.into(), kind: TrackEventKind::Meta(MetaMessage::EndOfTrack) });
        tracks.push(tr);
    }
    let smf = Smf { header: Header::new(Format::Parallel, Timing::Metrical(u15::new(PPQ as u16))), tracks };
    let mut out = Vec::new();
    smf.write(&mut out).expect("writing to a Vec cannot fail");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let song = MidiSong {
            bpm: 96.0,
            beats_per_bar: 3.0,
            tracks: vec![
                MidiTrack {
                    name: "piano".into(),
                    channel: 0,
                    program: Some(0),
                    notes: vec![
                        Note { t: 0.0, len: 1.0, key: 60.0, vel: 100.0 / 127.0 },
                        Note { t: 0.0, len: 0.5, key: 64.0, vel: 80.0 / 127.0 },
                        Note { t: 1.5, len: 0.25, key: 67.0, vel: 1.0 },
                    ],
                },
                MidiTrack { name: "drums".into(), channel: 9, program: None, notes: vec![Note { t: 0.0, len: 0.1, key: 36.0, vel: 1.0 }] },
            ],
        };
        let back = import(&export(&song)).unwrap();
        assert!((back.bpm - 96.0).abs() < 1e-3);
        assert_eq!(back.beats_per_bar, 3.0);
        assert_eq!(back.tracks.len(), 2);
        assert_eq!(back.tracks[0].name, "piano");
        assert_eq!(back.tracks[0].program, Some(0));
        let mut a = song.tracks[0].notes.clone();
        a.sort_by(|x, y| x.t.total_cmp(&y.t).then(x.key.total_cmp(&y.key)));
        let mut b = back.tracks[0].notes.clone();
        b.sort_by(|x, y| x.t.total_cmp(&y.t).then(x.key.total_cmp(&y.key)));
        for (x, y) in a.iter().zip(&b) {
            assert!((x.t - y.t).abs() < 1e-9 && (x.len - y.len).abs() < 1e-9 && x.key == y.key && (x.vel - y.vel).abs() < 1e-6, "{x:?} {y:?}");
        }
        assert_eq!(back.tracks[1].channel, 9);
        assert!(import(b"nope").is_err());
    }
}
