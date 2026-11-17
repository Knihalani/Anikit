//! (stub, replaced by the sampler)
pub mod packs {
    pub fn fetch(name: &str) -> Result<std::path::PathBuf, String> {
        Err(format!("unknown pack '{name}'"))
    }
}
