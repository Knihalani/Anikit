//! SoundFont 2 / SF3 and SFZ sampler.
pub mod bank;
pub mod engine;
pub mod interp;
pub mod parse;
pub mod sampler;
#[cfg(test)]
mod tests;
/// Optional larger sample packs (download to a cache).
pub mod packs {
    pub fn fetch(name: &str) -> Result<std::path::PathBuf, String> {
        Err(format!("pack '{name}': downloadable packs are not available in this build yet"))
    }
}
