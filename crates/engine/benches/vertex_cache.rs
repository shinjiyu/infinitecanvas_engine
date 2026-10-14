//! CPU 侧顶点缓存基准。不创建 GPU，也不报告 FPS。
//!
//! ```text
//! cargo bench -p infinitecanvas-engine --bench vertex_cache
//! ```

use infinitecanvas_engine::math::{Vec2, Vertex};
use infinitecanvas_engine::{rebuild_all_vertices, Rectangle, Scene, VertexCache};
use std::time::Instant;

const ELEMENTS: u64 = 100_000;
const SAMPLES: usize = 8;

fn main() {
    let mut scene = scene_with_rects(ELEMENTS);
    println!("CPU vertex benchmark (no GPU)");
    println!("software-render FPS is not measured and does not count");
    println!("elements: {ELEMENTS} rectangles");

    let rebuild_ms = median_ms_per_call(1, || {
        let geometry = rebuild_all_vertices(&scene);
        std::hint::black_box(geometry.fill.len());
    });
    let rebuild_bytes = rebuild_all_vertices(&scene).fill.len() * std::mem::size_of::<Vertex>();
    println!(
        "before  full rebuild each frame: {rebuild_ms:.3} ms/frame, {rebuild_bytes} vertex bytes rewritten"
    );

    let mut cache = VertexCache::new();
    let cold_started = Instant::now();
    let cold = cache.sync(&mut scene);
    let cold_ms = cold_started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "after   cold sync (first frame): {cold_ms:.3} ms, regenerated {}",
        cold.regenerated
    );

    let steady_ns = median_ms_per_call(10_000, || {
        std::hint::black_box(steady_once(&mut cache, &mut scene));
    }) * 1_000_000.0;
    println!(
        "after   steady pan/zoom (no dirty): {steady_ns:.1} ns/frame, 0 vertices rewritten (10k calls timed together)"
    );

    let dirty_us = median_ms_per_call(2_000, || {
        std::hint::black_box(dirty_once(&mut cache, &mut scene));
    }) * 1_000.0;
    println!("after   1 dirty element: {dirty_us:.2} us/frame");

    if steady_ns > 0.0 {
        println!(
            "steady speedup vs full rebuild: {:.0}x",
            rebuild_ms * 1_000_000.0 / steady_ns
        );
    }
}

#[inline(never)]
fn steady_once(cache: &mut VertexCache, scene: &mut Scene) -> usize {
    let sync = cache.sync(scene);
    assert!(sync.geometry_unchanged);
    sync.regenerated
}

#[inline(never)]
fn dirty_once(cache: &mut VertexCache, scene: &mut Scene) -> usize {
    scene
        .get_mut(1)
        .unwrap()
        .as_element_mut()
        .props_mut()
        .transform
        .translate(Vec2::new(0.01, 0.0));
    scene.mark_dirty(1);
    let sync = cache.sync(scene);
    assert_eq!(sync.regenerated, 1);
    sync.writes.len()
}

fn scene_with_rects(count: u64) -> Scene {
    let mut scene = Scene::new();
    for id in 1..=count {
        let column = (id % 400) as f32;
        let row = (id / 400) as f32;
        scene.add_rect(Rectangle::new(
            id,
            Vec2::new(column * 24.0, row * 24.0),
            Vec2::new(16.0, 12.0),
        ));
    }
    scene
}

fn median_ms_per_call(calls: usize, mut sample: impl FnMut()) -> f64 {
    for _ in 0..8 {
        sample();
    }
    let mut times = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        for _ in 0..calls {
            sample();
        }
        times.push(started.elapsed().as_secs_f64() * 1000.0 / calls as f64);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times[SAMPLES / 2]
}
