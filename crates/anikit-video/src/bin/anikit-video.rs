//! `anikit-video`: the command-line renderer.
//!
//! Planned commands (see docs/video/ARCHITECTURE.md):
//!   anikit-video render spec.json -o out.mp4
//!   anikit-video still  spec.json --at 2.5 -o frame.png
//!   anikit-video sheet  spec.json -o contact.png
//!   anikit-video check  spec.json
//!   anikit-video new    <template> [--brand brand.json]
//!   anikit-video mcp    (Model Context Protocol server on stdio)

fn main() {
    eprintln!("anikit-video: under construction (see docs/video/ARCHITECTURE.md)");
    std::process::exit(2);
}
