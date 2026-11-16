//! # Anikit Video
//!
//! Programmatic video for agents: a JSON [`spec`] in, a video out, rendered
//! natively and in parallel. There is no browser in the loop.
//!
//! Pipeline: `spec` (serde types) → `eval` (spec at time t → [`dl::DisplayList`])
//! → `render` (display list → pixels) → `pipeline` (parallel frames, YUV,
//! encoder, audio). `text` shapes and rasterises type; `components` provides
//! high-level building blocks (code, charts, device mockups, particles, 3D);
//! `templates` holds the agent-facing templates and prompts.
//!
//! See `docs/video/ARCHITECTURE.md` for ownership and contracts.

pub mod audio;
pub mod components;
pub mod dl;
pub mod agent_cli;
pub mod critique;
pub mod mcp;
pub mod prompts;
pub mod templates;
pub mod spec;
pub mod eval;
pub mod render;
pub mod layout;
pub mod text;
