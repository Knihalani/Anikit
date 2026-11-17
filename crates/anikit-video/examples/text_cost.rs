//! Per-frame text cost: `cargo run --release -p anikit-video --example text_cost`.
//! Reports µs/frame and heap allocations/frame for op generation (layout and
//! plans are built once, as in `eval`).
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use anikit_video::dl::{Op, IDENTITY};
use anikit_video::text::captions::{parse, CaptionStyle, CaptionTrack};
use anikit_video::text::code::{highlight, CodeSpec};
use anikit_video::text::kinetic::*;
use anikit_video::text::*;

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

/// Run `f` for 300 frames over `secs` (after a warm-up pass); print µs and allocations per frame.
fn frame(label: &str, secs: f32, mut f: impl FnMut(f32, &mut Vec<Op>)) {
    let mut ops = Vec::with_capacity(4096);
    let n = 300;
    for i in 0..n {
        anikit_video::text::recycle(&mut ops);
        f(secs * i as f32 / n as f32, &mut ops);
    }
    let a0 = ALLOCS.load(Ordering::Relaxed);
    let t = Instant::now();
    for i in 0..n {
        anikit_video::text::recycle(&mut ops);
        f(secs * i as f32 / n as f32, &mut ops);
    }
    let us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    let allocs = (ALLOCS.load(Ordering::Relaxed) - a0) as f64 / n as f64;
    println!("| {label:<34} | {us:>8.2} | {allocs:>8.1} | {:>5} |", ops.len());
}

fn main() {
    let fonts = FontStore::new();
    println!("| case | µs/frame | allocs/frame | ops |\n|---|---:|---:|---:|");
    let title = fonts.layout(&TextSpec::rich("Programmatic video, ==rendered natively==.", TextStyle { font: "800 104px Inter Display".into(), ..Default::default() }));
    frame("title, static", 1.0, |_, o| title.ops(IDENTITY, o));
    let words = "Agents are best at writing structured data so the video is a spec of scenes text shapes and timing while a native renderer draws every frame";
    let w: Vec<&str> = words.split(' ').collect();
    let para: String = (0..200).map(|i| w[i % w.len()]).collect::<Vec<_>>().join(" ");
    let p = fonts.layout(&TextSpec { text: para, width: Some(1600.0), style: TextStyle { size: 32.0, ..Default::default() }, ..Default::default() });
    frame("paragraph 200 words, static", 1.0, |_, o| p.ops(IDENTITY, o));
    let k = KineticPlan::new(&fonts, title.clone(), &"pop by char".parse().unwrap());
    frame("title, pop by char (whole anim)", k.duration(), |t, o| k.ops(t, IDENTITY, o));
    let k2 = KineticPlan::new(&fonts, p.clone(), &"rise by word 0.6s 20ms".parse().unwrap());
    frame("paragraph, rise by word", k2.duration(), |t, o| k2.ops(t, IDENTITY, o));
    let k3 = KineticPlan::new(&fonts, title.clone(), &"wave".parse().unwrap());
    frame("title, wave (every char moves)", 2.0, |t, o| k3.ops(t, IDENTITY, o));
    let srt = "1\n00:00:00,000 --> 00:00:04,000\nAgents write the spec and Rust renders every frame in parallel.\n\n2\n00:00:04,000 --> 00:00:08,000\nNo browser in the loop, and the encoder knows what changed.\n";
    let cap = CaptionTrack::new(&fonts, &parse(srt), &CaptionStyle::default());
    frame("captions, TikTok pop", 8.0, |t, o| cap.ops(t, IDENTITY, o));
    let code = "const film = video({ size: \"1080p\", fps: 60 })\n  .scene(\"intro\", s => s.text(\"Ship faster\", { animate: \"rise\" }))\n  .scene(\"demo\", s => s.code(src, { highlight: [2, 4] }))\n  .render(\"launch.mp4\");\n".repeat(4);
    let hl = highlight(&CodeSpec { code, language: "ts".into(), ..Default::default() });
    let cl = fonts.layout(&hl.text_spec(24.0, 1.6));
    frame("code block 16 lines, static", 1.0, |_, o| cl.ops(IDENTITY, o));
    let ck = KineticPlan::new(&fonts, cl.clone(), &"typewriter 10ms".parse().unwrap());
    frame("code block, typewriter", ck.duration(), |t, o| ck.ops(t, IDENTITY, o));
    let c = CounterPlan::new(&fonts, &Counter { to: 1_284_503.0, prefix: "$".into(), ..Default::default() });
    frame("counter", 1.5, |t, o| c.ops(t, IDENTITY, o));
}
