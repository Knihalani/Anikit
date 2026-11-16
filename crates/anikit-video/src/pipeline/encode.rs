//! Encoders. [`FfmpegEncoder`] pipes planar YUV 4:2:0 into ffmpeg (x264, x265,
//! VP9, AV1, ProRes, GIF, PNG sequence) and muxes audio in the same process.

use crate::eval::AudioClip;
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
            "good" => Quality::Good,
            "best" => Quality::Best,
            _ => return None,
        })
    }
}

/// Consumes frames in order.
pub trait Encoder {
    /// Append a frame. `repeat` is true when it's identical to the previous one.
    fn write(&mut self, f: &YuvFrame, repeat: bool) -> Result<(), String>;
    /// Finish; returns the output size in bytes.
    fn finish(&mut self) -> Result<u64, String>;
    fn output(&self) -> &str;
    fn name(&self) -> &str;
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
}

fn secs(x: f32) -> String {
    format!("{:.4}", x.max(0.0))
}

impl FfmpegEncoder {
    /// Start ffmpeg. Video size is cropped to even dimensions for 4:2:0.
    /// `audio` clips are placed relative to `t0` (the first frame's time) and cut at `dur`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(out: &Path, w: u32, h: u32, fps: f32, codec: Codec, q: Quality, audio: &[AudioClip], t0: f32, dur: f32) -> Result<Self, String> {
        let (w, h) = (w & !1, h & !1);
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "error", "-y"]);
        cmd.args(["-f", "rawvideo", "-pix_fmt", "yuv420p", "-s", &format!("{w}x{h}"), "-r", &format!("{fps}"), "-i", "-"]);
        let use_audio = !audio.is_empty() && !matches!(codec, Codec::Gif | Codec::Png);
        let mut filters = Vec::new();
        if use_audio {
            for (k, a) in audio.iter().enumerate() {
                if a.looped {
                    cmd.args(["-stream_loop", "-1"]);
                }
                cmd.arg("-i").arg(&a.src);
                // Position in output time.
                let rel = a.at - t0;
                let mut start = a.trim_start;
                let mut delay = rel;
                if rel < 0.0 {
                    start -= rel;
                    delay = 0.0;
                }
                let mut f = format!("[{}:a]atrim=start={}", k + 1, secs(start));
                if let Some(d) = a.duration {
                    f += &format!(":duration={}", secs(d - (start - a.trim_start)));
                }
                f += &format!(",asetpts=PTS-STARTPTS,volume={:.4}", a.volume);
                if a.fade_in > 0.0 {
                    f += &format!(",afade=t=in:d={}", secs(a.fade_in));
                }
                if a.fade_out > 0.0 {
                    let end = a.duration.map(|d| d - (start - a.trim_start)).unwrap_or(dur - delay).min(dur - delay);
                    f += &format!(",afade=t=out:st={}:d={}", secs(end - a.fade_out), secs(a.fade_out));
                }
                let ms = (delay * 1000.0).round() as i64;
                f += &format!(",adelay={ms}|{ms}[a{k}]");
                filters.push(f);
            }
            let ins: String = (0..audio.len()).map(|k| format!("[a{k}]")).collect();
            filters.push(format!("{ins}amix=inputs={}:normalize=0:dropout_transition=0,atrim=0:{}[aout]", audio.len(), secs(dur)));
            cmd.args(["-filter_complex", &filters.join(";")]);
            cmd.args(["-map", "0:v", "-map", "[aout]", "-c:a", "aac", "-b:a", "192k"]);
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
                if use_audio {
                    cmd.args(["-c:a", "libopus"]);
                }
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
            Codec::Png => {}
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
        cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("can't start ffmpeg (is it installed?): {e}"))?;
        let stdin = child.stdin.take();
        Ok(FfmpegEncoder { child: Some(child), stdin, out: out.to_path_buf(), w, h, buf: Vec::new(), name: codec.name().into() })
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
    fn write(&mut self, f: &YuvFrame, repeat: bool) -> Result<(), String> {
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
