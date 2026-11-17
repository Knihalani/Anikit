//! Just enough of ZIP to unpack ambientCG downloads: the central directory,
//! stored and deflated entries (no zip64, no encryption).

use std::io::Read;

/// `(name, bytes)` for every file whose name passes `keep`.
pub fn extract(data: &[u8], keep: impl Fn(&str) -> bool) -> Result<Vec<(String, Vec<u8>)>, String> {
    let u16le = |o: usize| -> Result<usize, String> { data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or_else(|| "zip: truncated".to_string()) };
    let u32le = |o: usize| -> Result<usize, String> { data.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).ok_or_else(|| "zip: truncated".to_string()) };
    // End of central directory: the last `PK\x05\x06`.
    let lo = data.len().saturating_sub(22 + 65535);
    let eocd = (lo..data.len().saturating_sub(21)).rev().find(|&i| data[i..i + 4] == *b"PK\x05\x06").ok_or("zip: no end of central directory (not a zip file?)")?;
    let count = u16le(eocd + 10)?;
    let mut p = u32le(eocd + 16)?;
    let mut out = Vec::new();
    for _ in 0..count {
        if data.get(p..p + 4) != Some(b"PK\x01\x02") {
            return Err("zip: bad central directory".into());
        }
        let method = u16le(p + 10)?;
        let csize = u32le(p + 20)?;
        let usize_ = u32le(p + 24)?;
        let (nl, xl, cl) = (u16le(p + 28)?, u16le(p + 30)?, u16le(p + 32)?);
        let local = u32le(p + 42)?;
        let name = String::from_utf8_lossy(data.get(p + 46..p + 46 + nl).ok_or("zip: truncated")?).to_string();
        p += 46 + nl + xl + cl;
        if name.ends_with('/') || !keep(&name) {
            continue;
        }
        if data.get(local..local + 4) != Some(b"PK\x03\x04") {
            return Err(format!("zip: bad local header for {name}"));
        }
        let start = local + 30 + u16le(local + 26)? + u16le(local + 28)?;
        let raw = data.get(start..start + csize).ok_or("zip: truncated entry")?;
        let bytes = match method {
            0 => raw.to_vec(),
            8 => {
                let mut v = Vec::with_capacity(usize_);
                flate2::read::DeflateDecoder::new(raw).read_to_end(&mut v).map_err(|e| format!("zip: {name}: {e}"))?;
                v
            }
            m => return Err(format!("zip: {name}: unsupported compression method {m}")),
        };
        out.push((name, bytes));
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) fn make(files: &[(&str, &[u8])]) -> Vec<u8> {
    // Stored entries only (for tests).
    let mut out = Vec::new();
    let mut cd = Vec::new();
    for (name, body) in files {
        let off = out.len() as u32;
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(body);
        cd.extend_from_slice(b"PK\x01\x02");
        cd.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        cd.extend_from_slice(&(body.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(body.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
        cd.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        cd.extend_from_slice(&off.to_le_bytes());
        cd.extend_from_slice(name.as_bytes());
    }
    let cdo = out.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
    out.extend_from_slice(&cdo.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip_stored() {
        let z = super::make(&[("a/Color.jpg", b"hello"), ("b.usda", b"x")]);
        let got = super::extract(&z, |n| n.ends_with(".jpg")).unwrap();
        assert_eq!(got, vec![("a/Color.jpg".to_string(), b"hello".to_vec())]);
    }
}
