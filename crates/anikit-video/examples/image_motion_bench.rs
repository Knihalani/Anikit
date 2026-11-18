//! Scaled-image motion benchmark and frame-output hash check.
//!
//! `cargo run --release -p anikit-video --example image_motion_bench`

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anikit_video::dl::{DisplayList, ImageId, Op, Rgba};
use anikit_video::render::{Assets, Quality, Renderer};

struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FRAMES: usize = 120;
const WARMUP: usize = 8;

fn image(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let noise = ((x.wrapping_mul(37) ^ y.wrapping_mul(73) ^ x.wrapping_mul(y)) & 31) as u8;
            let alpha = 160 + ((x * 13 + y * 7) % 96) as u8;
            pixels.extend_from_slice(&[
                (x * 255 / width) as u8 ^ noise,
                (y * 255 / height) as u8 ^ noise.wrapping_mul(3),
                ((x + y) * 255 / (width + height)) as u8 ^ noise.wrapping_mul(5),
                alpha,
            ]);
        }
    }
    pixels
}

fn lists(image: ImageId, source_width: u32, source_height: u32) -> Vec<DisplayList> {
    (0..FRAMES + WARMUP)
        .map(|i| {
            let frame = i as f32;
            let scale = if source_width == WIDTH {
                1.0 + frame * 0.15 / FRAMES as f32
            } else {
                4.8 + frame * 0.15 / FRAMES as f32
            };
            let x = 0.17 + frame * 0.037;
            let y = 0.61 + frame * 0.023;
            DisplayList {
                width: WIDTH,
                height: HEIGHT,
                background: Rgba([0.025, 0.035, 0.05, 1.0]),
                ops: vec![Op::Image {
                    image,
                    dst: [0.0, 0.0, source_width as f32, source_height as f32],
                    opacity: 1.0,
                    transform: [scale, 0.0, 0.0, scale, x, y],
                }],
            }
        })
        .collect()
}

fn frame_hash(hash: &mut u64, bytes: &[u8]) {
    for &byte in bytes {
        *hash ^= byte as u64;
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

fn run_case(
    name: &str,
    image: ImageId,
    source_width: u32,
    source_height: u32,
    assets: &Arc<Assets>,
    quality: Quality,
) {
    let lists = lists(image, source_width, source_height);
    let mut renderer = Renderer::new(assets.clone());
    renderer.set_quality(quality);
    for list in lists.iter().take(WARMUP) {
        renderer.render(list);
    }

    let (allocs_before, bytes_before) = (
        ALLOCS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    );
    let start = Instant::now();
    let mut hash = 0xcbf29ce484222325;
    for list in lists.iter().skip(WARMUP) {
        let frame = renderer.render(list);
        frame_hash(&mut hash, frame.data_as_u8_slice());
    }
    let elapsed = start.elapsed();
    let allocs = ALLOCS.load(Ordering::Relaxed) - allocs_before;
    let bytes = BYTES.load(Ordering::Relaxed) - bytes_before;
    println!(
        "{name} / {quality:?}: {:.3} ms/frame, {:.1} allocations/frame, {:.2} MB allocated/frame, hash {hash:016x}",
        elapsed.as_secs_f64() * 1000.0 / FRAMES as f64,
        allocs as f64 / FRAMES as f64,
        bytes as f64 / FRAMES as f64 / 1_000_000.0,
    );
}

fn main() {
    let mut assets = Assets::default();
    let motion = ImageId(1);
    let enlargement = ImageId(2);
    assets
        .images
        .insert_rgba8(motion, WIDTH, HEIGHT, image(WIDTH, HEIGHT))
        .unwrap();
    assets
        .images
        .insert_rgba8(enlargement, 400, 225, image(400, 225))
        .unwrap();
    let assets = Arc::new(assets);

    for quality in [Quality::Standard, Quality::Reference] {
        run_case("1920x1080 motion", motion, WIDTH, HEIGHT, &assets, quality);
        run_case(
            "400x225 enlarged to 1920x1080",
            enlargement,
            400,
            225,
            &assets,
            quality,
        );
    }
}
