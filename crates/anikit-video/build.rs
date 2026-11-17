//! Embed the git commit (and whether the tree was dirty) for `anikit-video --version`.
fn main() {
    let git = |args: &[&str]| std::process::Command::new("git").args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let hash = git(&["rev-parse", "--short=10", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    let date = git(&["log", "-1", "--format=%cs"]).unwrap_or_default();
    println!("cargo:rustc-env=ANIKIT_GIT={hash}{}", if dirty { "-dirty" } else { "" });
    println!("cargo:rustc-env=ANIKIT_GIT_DATE={date}");
    // Rebuild when HEAD moves (a new commit or a checkout).
    for p in ["../../.git/HEAD", "../../.git/logs/HEAD"] {
        if std::path::Path::new(p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }
}
