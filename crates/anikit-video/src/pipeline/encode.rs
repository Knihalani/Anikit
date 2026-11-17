//! Encoders. [`FfmpegEncoder`] pipes planar YUV 4:2:0 into ffmpeg (x264, x265,
//! VP9, AV1, ProRes, GIF, PNG sequence) and muxes audio in the same process.

use crate::render::YuvFrame;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

/// Output codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    H264,
    H265,
    Vp9,
    Av1,
    ProRes,
    Gif,
    Png,
    /// Our renderer-aware H.264 encoder (MP4 via ffmpeg `-c copy`).
    Ak264,
}

impl Codec {
    /// Pick a codec from the output path.
    pub fn from_path(p: &Path) -> Codec {
        let s = p.to_string_lossy().to_ascii_lowercase();
        if s.ends_with(".webm") {
            Codec::Vp9
        } else if s.ends_with(".gif") {
            Codec::Gif
        } else if s.ends_with(".png") {
            Codec::Png
        } else if s.ends_with(".mov") {
            Codec::ProRes
        } else if s.ends_with(".mkv") {
            Codec::Av1
        } else {
            Codec::H264
        }
    }
    pub fn parse(s: &str) -> Option<Codec> {
        Some(match s.to_ascii_lowercase().as_str() {
            "h264" | "x264" | "avc" => Codec::H264,
            "h265" | "x265" | "hevc" => Codec::H265,
            "vp9" => Codec::Vp9,
            "av1" => Codec::Av1,
            "prores" => Codec::ProRes,
            "gif" => Codec::Gif,
            "png" => Codec::Png,
            "ak264" => Codec::Ak264,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Codec::H264 => "h264",
            Codec::H265 => "h265",
            Codec::Vp9 => "vp9",
            Codec::Av1 => "av1",
            Codec::ProRes => "prores",
            Codec::Gif => "gif",
            Codec::Png => "png",
            Codec::Ak264 => "ak264",
        }
    }
}

/// Speed / size trade-off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Draft,
    Good,
    Best,
}

impl Quality {
    pub fn parse(s: &str) -> Option<Quality> {
        Some(match s {
            "draft" => Quality::Draft,
            "good" | "standard" | "normal" => Quality::Good,
            "best" | "high" | "max" => Quality::Best,
            _ => return None,
        })
    }
}

/// Consumes frames in order.
pub trait Encoder: Send {
    /// Append a frame. `repeat` is true when it's identical to the previous one;
    /// `changed` (if given) flags the 16×16 macroblocks that may differ from it.
    fn write(&mut self, f: &YuvFrame, repeat: bool, changed: Option<Vec<bool>>) -> Result<(), String>;
    /// Whether `changed` maps are useful (computing them costs a little).
    fn wants_hints(&self) -> bool {
        false
    }
    /// Finish; returns the output size in bytes.
    fn finish(&mut self) -> Result<u64, String>;
    fn output(&self) -> &str;
    fn name(&self) -> &str;
    /// True if the encoder takes straight RGBA8 frames ([`Encoder::write_rgba`]) instead of YUV.
    fn wants_rgba(&self) -> bool {
        false
    }
    /// Append a straight-alpha RGBA8 frame (`w × h × 4`, the encoder's size).
    fn write_rgba(&mut self, _px: &[u8]) -> Result<(), String> {
        Err("this encoder takes YUV frames".into())
    }
}

/// ffmpeg via a raw YUV pipe.
pub struct FfmpegEncoder {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    out: PathBuf,
    w: u32,
    h: u32,
    buf: Vec<u8>,
    name: String,
    ak: Option<ak264::Encoder>,
    rgba: bool,
}

impl FfmpegEncoder {
    /// Start ffmpeg. Video size is cropped to even dimensions for 4:2:0.
    /// `audio` is a mixed WAV aligned with the first frame.
    #[allow(clippy::too_many_arguments)]
    pub fn new(out: &Path, w: u32, h: u32, fps: f32, codec: Codec, q: Quality, audio: Option<&Path>) -> Result<Self, String> {
        let (w, h) = (w & !1, h & !1);
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "error", "-y"]);
        if codec == Codec::Ak264 {
            cmd.args(["-f", "h264", "-r", &format!("{fps}"), "-i", "-"]);
        } else {
            if matches!(codec, Codec::Png | Codec::Gif) {
                // Stills and GIFs get straight RGBA: no 4:2:0 subsampling, no matrix guesswork.
                cmd.args(["-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-r", &format!("{fps}"), "-i", "-"]);
            } else {
                // Our YUV is BT.709, limited range: say so, so conversions don't assume BT.601.
                cmd.args(["-f", "rawvideo", "-pix_fmt", "yuv420p", "-s", &format!("{w}x{h}"), "-r", &format!("{fps}")]);
                cmd.args(["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv", "-i", "-"]);
            }
        }
        let use_audio = audio.is_some() && !matches!(codec, Codec::Gif | Codec::Png);
        if let (true, Some(a)) = (use_audio, audio) {
            cmd.arg("-i").arg(a);
            cmd.args(["-map", "0:v", "-map", "1:a"]);
            let ac = if codec == Codec::Vp9 { "libopus" } else if codec == Codec::ProRes { "pcm_s16le" } else { "aac" };
            cmd.args(["-c:a", ac, "-b:a", "192k"]);
        }
        if codec == Codec::Ak264 {
            cmd.args(["-c:v", "copy", "-movflags", "+faststart"]);
            if !use_audio {
                cmd.args(["-map", "0:v"]);
            }
        }
        let (preset, crf) = match q {
            Quality::Draft => ("ultrafast", 26),
            Quality::Good => ("veryfast", 20),
            Quality::Best => ("slow", 16),
        };
        match codec {
            Codec::H264 => {
                cmd.args(["-c:v", "libx264", "-preset", preset, "-crf", &crf.to_string(), "-pix_fmt", "yuv420p", "-movflags", "+faststart"]);
            }
            Codec::H265 => {
                cmd.args(["-c:v", "libx265", "-preset", preset, "-crf", &(crf + 4).to_string(), "-tag:v", "hvc1", "-x265-params", "log-level=error"]);
            }
            Codec::Vp9 => {
                let speed = match q {
                    Quality::Draft => "8",
                    Quality::Good => "5",
                    Quality::Best => "2",
                };
                cmd.args(["-c:v", "libvpx-vp9", "-b:v", "0", "-crf", &(crf + 12).to_string(), "-row-mt", "1", "-deadline", "realtime", "-cpu-used", speed]);
            }
            Codec::Av1 => {
                let p = match q {
                    Quality::Draft => "12",
                    Quality::Good => "8",
                    Quality::Best => "4",
                };
                cmd.args(["-c:v", "libsvtav1", "-preset", p, "-crf", &(crf + 12).to_string()]);
            }
            Codec::ProRes => {
                cmd.args(["-c:v", "prores_ks", "-profile:v", "3", "-pix_fmt", "yuv422p10le"]);
            }
            Codec::Gif => {
                let gf = fps.min(25.0);
                cmd.args(["-vf", &format!("fps={gf},split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4")]);
            }
            Codec::Png | Codec::Ak264 => {}
        }
        if matches!(codec, Codec::H264 | Codec::H265 | Codec::Vp9 | Codec::Av1 | Codec::ProRes) {
            cmd.args(["-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709", "-color_range", "tv"]);
        }
        let out_s = if codec == Codec::Png && !out.to_string_lossy().contains('%') {
            // `frames.png` → `frames_00001.png`.
            let s = out.to_string_lossy();
            PathBuf::from(format!("{}_%05d.png", s.trim_end_matches(".png")))
        } else {
            out.to_path_buf()
        };
        if let Some(dir) = out_s.parent() {
            if !dir.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(dir);
            }
        }
        cmd.arg(&out_s);
        if std::env::var_os("ANIKIT_DEBUG").is_some() {
            eprintln!("{cmd:?}");
        }
        cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("can't start ffmpeg (is it installed?): {e}"))?;
        let stdin = child.stdin.take();
        let ak = (codec == Codec::Ak264).then(|| {
            let qp = match q {
                Quality::Draft => 28,
                Quality::Good => 23,
                Quality::Best => 19,
            };
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
            ak264::Encoder::new(ak264::EncoderConfig { width: w, height: h, fps: fps as f64, qp, gop: (fps * 4.0) as u32, deblock: true, threads, full_range: false, cabac: q != Quality::Draft, subpel: q != Quality::Draft, ..Default::default() })
        });
        Ok(FfmpegEncoder { child: Some(child), stdin, out: out.to_path_buf(), w, h, buf: Vec::new(), name: codec.name().into(), ak, rgba: matches!(codec, Codec::Png | Codec::Gif) })
    }

    fn pack(&mut self, f: &YuvFrame) {
        let (w, h) = (self.w as usize, self.h as usize);
        self.buf.clear();
        for y in 0..h {
            self.buf.extend_from_slice(&f.y[y * f.y_stride..y * f.y_stride + w]);
        }
        for plane in [&f.u, &f.v] {
            for y in 0..h / 2 {
                self.buf.extend_from_slice(&plane[y * f.c_stride..y * f.c_stride + w / 2]);
            }
        }
    }
}

impl Encoder for FfmpegEncoder {
    fn wants_hints(&self) -> bool {
        self.ak.is_some()
    }

    fn wants_rgba(&self) -> bool {
        self.rgba
    }

    fn write_rgba(&mut self, px: &[u8]) -> Result<(), String> {
        let r = self.stdin.as_mut().ok_or("encoder finished")?.write_all(px);
        if r.is_err() {
            return Err(self.finish().err().unwrap_or_else(|| "ffmpeg closed its input".into()));
        }
        Ok(())
    }

    fn write(&mut self, f: &YuvFrame, repeat: bool, changed: Option<Vec<bool>>) -> Result<(), String> {
        if let Some(ak) = &mut self.ak {
            let fr = ak264::YuvFrame { width: self.w, height: self.h, y: &f.y, u: &f.u, v: &f.v, y_stride: f.y_stride, c_stride: f.c_stride };
            let hints = if repeat {
                let (mw, mh) = ak.mb_size();
                ak264::FrameHints { changed: Some(vec![false; mw * mh]), motions: Vec::new(), ..Default::default() }
            } else {
                let (mw, mh) = ak.mb_size();
                ak264::FrameHints { changed: changed.filter(|c| c.len() == mw * mh), motions: Vec::new(), ..Default::default() }
            };
            let bytes = ak.encode(&fr, &hints);
            let r = self.stdin.as_mut().ok_or("encoder finished")?.write_all(bytes);
            if r.is_err() {
                return Err(self.finish().err().unwrap_or_else(|| "ffmpeg closed its input".into()));
            }
            return Ok(());
        }
        if !repeat || self.buf.is_empty() {
            self.pack(f);
        }
        let r = self.stdin.as_mut().ok_or("encoder finished")?.write_all(&self.buf);
        if r.is_err() {
            // ffmpeg died; surface its error.
            return Err(self.finish().err().unwrap_or_else(|| "ffmpeg closed its input".into()));
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<u64, String> {
        drop(self.stdin.take());
        if let Some(c) = self.child.take() {
            let o = c.wait_with_output().map_err(|e| e.to_string())?;
            if !o.status.success() {
                return Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&o.stderr).trim()));
            }
        }
        Ok(std::fs::metadata(&self.out).map(|m| m.len()).unwrap_or(0))
    }

    fn output(&self) -> &str {
        self.out.to_str().unwrap_or("")
    }
    fn name(&self) -> &str {
        &self.name
    }
}
