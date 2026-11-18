//! Process memory readings for `--profile` and qbench: resident set size now,
//! and the peak since the last [`reset_peak`]. Linux only (`/proc/self`);
//! elsewhere every reading is `None`. Measurement only: nothing depends on it.

fn status_kb(key: &str) -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with(key))?;
    line[key.len()..].trim().trim_end_matches("kB").trim().parse().ok()
}

/// Resident set size now, in bytes.
pub fn rss() -> Option<u64> {
    status_kb("VmRSS:").map(|k| k * 1024)
}

/// Peak resident set size since the process started or the last [`reset_peak`], in bytes.
pub fn peak() -> Option<u64> {
    status_kb("VmHWM:").map(|k| k * 1024)
}

/// Restart the peak at the current RSS (Linux ≥ 4.0: `clear_refs` 5), so a
/// stage's own peak can be read. Returns false if the kernel refused.
pub fn reset_peak() -> bool {
    std::fs::write("/proc/self/clear_refs", "5").is_ok()
}

/// Bytes to MiB, for display.
pub fn mib(b: u64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
extern "C" {
    fn malloc_trim(pad: usize) -> i32;
}

/// Hand freed heap pages back to the OS. glibc keeps a render's freed frame
/// buffers in its per-thread arenas (hundreds of MiB, growing over repeated
/// renders in one process: the MCP server, previews, qbench); this returns
/// them. A no-op off glibc. Costs a few milliseconds; call it between renders,
/// never per frame.
pub fn trim() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    unsafe {
        malloc_trim(0);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "linux")]
    fn readings_and_trim() {
        let r = super::rss().expect("rss");
        assert!(r > 0);
        let big = vec![1u8; 64 << 20];
        assert!(super::peak().unwrap() >= r);
        if super::reset_peak() {
            assert!(super::peak().unwrap() >= super::rss().unwrap() - (1 << 20));
        }
        drop(std::hint::black_box(big));
        super::trim();
    }
}
