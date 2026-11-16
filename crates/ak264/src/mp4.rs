//! Minimal MP4 (ISO BMFF) muxer for an ak264 Annex-B stream: `ftyp`, `moov`
//! (one `avc1` track with `avcC`), `mdat`. The `moov` comes first (fast start).

/// Split an Annex-B byte stream into NAL units (without start codes).
pub fn nal_units(s: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= s.len() {
        if s[i] == 0 && s[i + 1] == 0 && s[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (k, &st) in starts.iter().enumerate() {
        let mut end = if k + 1 < starts.len() { starts[k + 1] - 3 } else { s.len() };
        while end > st && s[end - 1] == 0 {
            end -= 1; // trailing zero of a 4-byte start code
        }
        out.push(&s[st..end]);
    }
    out
}

struct B(Vec<u8>);
impl B {
    fn u8(&mut self, v: u8) {
        self.0.push(v)
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_be_bytes())
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_be_bytes())
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_be_bytes())
    }
    fn bytes(&mut self, b: &[u8]) {
        self.0.extend_from_slice(b)
    }
    fn open(&mut self, t: &[u8; 4]) -> usize {
        let p = self.0.len();
        self.u32(0);
        self.bytes(t);
        p
    }
    fn full(&mut self, t: &[u8; 4], ver: u8, flags: u32) -> usize {
        let p = self.open(t);
        self.u32(((ver as u32) << 24) | flags);
        p
    }
    fn close(&mut self, p: usize) {
        let n = (self.0.len() - p) as u32;
        self.0[p..p + 4].copy_from_slice(&n.to_be_bytes());
    }
}

const MATRIX: [u32; 9] = [0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x4000_0000];

/// Wrap an Annex-B stream (as produced by [`crate::Encoder`] / [`crate::encode_chunks`])
/// into an MP4 file.
pub fn mux(annexb: &[u8], width: u32, height: u32, fps: f64) -> Vec<u8> {
    let nals = nal_units(annexb);
    let mut sps: Option<&[u8]> = None;
    let mut pps: Option<&[u8]> = None;
    // samples: (offset in mdat payload, size, key)
    let mut samples: Vec<(usize, usize, bool)> = Vec::new();
    let mut mdat: Vec<u8> = Vec::with_capacity(annexb.len() + nals.len() * 4);
    for n in &nals {
        if n.is_empty() {
            continue;
        }
        let t = n[0] & 0x1f;
        match t {
            7 => {
                sps.get_or_insert(n);
            }
            8 => {
                pps.get_or_insert(n);
            }
            1 | 5 => {
                // first_mb_in_slice == 0 ⇔ first bit of the slice header is 1
                let first = n.len() > 1 && n[1] & 0x80 != 0;
                if first || samples.is_empty() {
                    samples.push((mdat.len(), 0, t == 5));
                }
                mdat.extend_from_slice(&(n.len() as u32).to_be_bytes());
                mdat.extend_from_slice(n);
                let last = samples.last_mut().unwrap();
                last.1 = mdat.len() - last.0;
            }
            _ => {}
        }
    }
    let sps = sps.expect("mp4: stream has no SPS");
    let pps = pps.expect("mp4: stream has no PPS");
    let (timescale, delta) = if (fps - fps.round()).abs() < 1e-6 {
        (fps.round() as u32 * 1000, 1000u32)
    } else {
        ((fps * 1001.0).round() as u32, 1001u32)
    };
    let n = samples.len() as u64;
    let dur_ts = n * delta as u64;
    let dur_ms = dur_ts * 1000 / timescale as u64;
    let big = mdat.len() as u64 + 4096 > u32::MAX as u64;

    let build = |chunk_off: u64| -> Vec<u8> {
        let mut b = B(Vec::new());
        let moov = b.open(b"moov");
        let p = b.full(b"mvhd", 0, 0);
        b.u32(0);
        b.u32(0);
        b.u32(1000);
        b.u32(dur_ms as u32);
        b.u32(0x10000);
        b.u16(0x100);
        b.bytes(&[0; 10]);
        MATRIX.iter().for_each(|&m| b.u32(m));
        b.bytes(&[0; 24]);
        b.u32(2);
        b.close(p);
        let trak = b.open(b"trak");
        let p = b.full(b"tkhd", 0, 3);
        b.u32(0);
        b.u32(0);
        b.u32(1);
        b.u32(0);
        b.u32(dur_ms as u32);
        b.u64(0);
        b.u16(0);
        b.u16(0);
        b.u16(0);
        b.u16(0);
        MATRIX.iter().for_each(|&m| b.u32(m));
        b.u32(width << 16);
        b.u32(height << 16);
        b.close(p);
        let mdia = b.open(b"mdia");
        let p = b.full(b"mdhd", 0, 0);
        b.u32(0);
        b.u32(0);
        b.u32(timescale);
        b.u32(dur_ts as u32);
        b.u16(0x55c4);
        b.u16(0);
        b.close(p);
        let p = b.full(b"hdlr", 0, 0);
        b.u32(0);
        b.bytes(b"vide");
        b.bytes(&[0; 12]);
        b.bytes(b"VideoHandler\0");
        b.close(p);
        let minf = b.open(b"minf");
        let p = b.full(b"vmhd", 0, 1);
        b.bytes(&[0; 8]);
        b.close(p);
        let dinf = b.open(b"dinf");
        let dref = b.full(b"dref", 0, 0);
        b.u32(1);
        let p = b.full(b"url ", 0, 1);
        b.close(p);
        b.close(dref);
        b.close(dinf);
        let stbl = b.open(b"stbl");
        let stsd = b.full(b"stsd", 0, 0);
        b.u32(1);
        let avc1 = b.open(b"avc1");
        b.bytes(&[0; 6]);
        b.u16(1);
        b.bytes(&[0; 16]);
        b.u16(width as u16);
        b.u16(height as u16);
        b.u32(0x48_0000);
        b.u32(0x48_0000);
        b.u32(0);
        b.u16(1);
        let mut name = [0u8; 32];
        name[0] = 5;
        name[1..6].copy_from_slice(b"ak264");
        b.bytes(&name);
        b.u16(0x18);
        b.u16(0xffff);
        let avcc = b.open(b"avcC");
        b.u8(1);
        b.u8(sps[1]);
        b.u8(sps[2]);
        b.u8(sps[3]);
        b.u8(0xff);
        b.u8(0xe1);
        b.u16(sps.len() as u16);
        b.bytes(sps);
        b.u8(1);
        b.u16(pps.len() as u16);
        b.bytes(pps);
        b.close(avcc);
        b.close(avc1);
        b.close(stsd);
        let p = b.full(b"stts", 0, 0);
        b.u32(1);
        b.u32(n as u32);
        b.u32(delta);
        b.close(p);
        let keys: Vec<u32> = samples.iter().enumerate().filter(|s| s.1 .2).map(|(i, _)| i as u32 + 1).collect();
        if keys.len() < samples.len() {
            let p = b.full(b"stss", 0, 0);
            b.u32(keys.len() as u32);
            keys.iter().for_each(|&k| b.u32(k));
            b.close(p);
        }
        let p = b.full(b"stsc", 0, 0);
        b.u32(1);
        b.u32(1);
        b.u32(n as u32);
        b.u32(1);
        b.close(p);
        let p = b.full(b"stsz", 0, 0);
        b.u32(0);
        b.u32(n as u32);
        samples.iter().for_each(|s| b.u32(s.1 as u32));
        b.close(p);
        if big {
            let p = b.full(b"co64", 0, 0);
            b.u32(1);
            b.u64(chunk_off);
            b.close(p);
        } else {
            let p = b.full(b"stco", 0, 0);
            b.u32(1);
            b.u32(chunk_off as u32);
            b.close(p);
        }
        b.close(stbl);
        b.close(minf);
        b.close(mdia);
        b.close(trak);
        b.close(moov);
        b.0
    };

    let mut ftyp = B(Vec::new());
    let p = ftyp.open(b"ftyp");
    ftyp.bytes(b"isom");
    ftyp.u32(0x200);
    ftyp.bytes(b"isomiso2avc1mp41");
    ftyp.close(p);
    let moov_len = build(0).len();
    let mdat_hdr = if big { 16 } else { 8 };
    let off = (ftyp.0.len() + moov_len + mdat_hdr) as u64;
    let moov = build(off);
    let mut out = ftyp.0;
    out.extend_from_slice(&moov);
    if big {
        out.extend_from_slice(&1u32.to_be_bytes());
        out.extend_from_slice(b"mdat");
        out.extend_from_slice(&(mdat.len() as u64 + 16).to_be_bytes());
    } else {
        out.extend_from_slice(&(mdat.len() as u32 + 8).to_be_bytes());
        out.extend_from_slice(b"mdat");
    }
    out.extend_from_slice(&mdat);
    out
}

/// Mux with ffmpeg (`-c copy`), for when audio or other containers are needed.
/// Returns ffmpeg's stderr on failure.
pub fn mux_with_ffmpeg(h264: &std::path::Path, out: &std::path::Path, fps: f64) -> Result<(), String> {
    let o = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-r"])
        .arg(format!("{fps}"))
        .args(["-f", "h264", "-i"])
        .arg(h264)
        .args(["-c", "copy", "-movflags", "+faststart"])
        .arg(out)
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).into_owned())
    }
}
