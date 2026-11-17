//! Allocations per `eval_into` in steady state (a counting global allocator).
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(l.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(n as u64, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

fn main() {
    use anikit_video::eval::{Evaluator, Options};
    let path = std::env::args().nth(1).unwrap_or_else(|| "docs/video/examples/showcase.json".into());
    let p = anikit_video::spec::parse_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let base = std::path::Path::new(&path).parent().unwrap().to_path_buf();
    let ev = Evaluator::new(&p.spec, Options { base_dir: base, ..Default::default() }).unwrap();
    let mut dl = Default::default();
    // Warm up: every frame once (caches, lazily made images and layouts).
    for i in 0..ev.frames() {
        ev.eval_into(i as f32 / ev.fps, &mut dl);
    }
    let (a0, b0) = (ALLOCS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed));
    let mut ops = 0;
    for i in 0..ev.frames() {
        ev.eval_into(i as f32 / ev.fps, &mut dl);
        ops += dl.ops.len();
    }
    let n = ev.frames() as u64;
    println!(
        "{}: {:.1} allocations / frame ({:.1} KB), {:.1} ops / frame",
        path,
        (ALLOCS.load(Ordering::Relaxed) - a0) as f64 / n as f64,
        (BYTES.load(Ordering::Relaxed) - b0) as f64 / n as f64 / 1024.0,
        ops as f64 / n as f64
    );
}
