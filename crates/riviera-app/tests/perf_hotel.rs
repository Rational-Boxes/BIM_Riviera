// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A performance benchmark on the hotel sample (DM-PERF-001): reading it, building its scene
//! twice, and saving it, with the profiler's breakdown. Run it in release:
//! `cargo test --release -p riviera-app --test perf_hotel -- --ignored --nocapture`.

use riviera_app::{App, AppConfig};
use std::time::Instant;
#[test]
#[ignore = "a benchmark: run in release with --ignored --nocapture"]
fn timing() {
    let dir = tempfile::tempdir().unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-data/samples/hotel.ifc");
    let path = dir.path().join("hotel.ifc");
    std::fs::copy(&src, &path).unwrap();
    let t = Instant::now();
    let loaded = riviera_app::load(&path, &|s| println!("stage {:?} at {:?}", s as u8, t.elapsed())).unwrap();
    println!("load (worker): {:?}", t.elapsed());
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    let t = Instant::now();
    let id = app.open_loaded(loaded, false).unwrap();
    println!("open_loaded (UI): {:?}", t.elapsed());
    let t = Instant::now();
    let scene = app.render_scene(id).unwrap();
    println!("render_scene (UI): {:?}, {} meshes", t.elapsed(), scene.meshes.len());
    riviera_core::profiler::start(50.0);
    let t = Instant::now();
    let scene = app.render_scene(id).unwrap();
    let r = riviera_core::profiler::report(Some(30), riviera_core::profiler::Sort::SelfTime);
    for row in &r.rows { println!("ROW {:<40} calls {:>7} total {:>9.1} self {:>9.1}", row.name, row.calls, row.total_ms, row.self_ms); }
    for p in r.paths.iter().take(25) { println!("PATH {:?}", p); }
    println!("render_scene again (UI): {:?}, {} meshes", t.elapsed(), scene.meshes.len());
    riviera_core::profiler::start(50.0);
    let t = Instant::now();
    app.execute("file.save_as", serde_json::json!({ "path": dir.path().join("again.ifc") }), &riviera_core::Caller::user()).unwrap();
    println!("save_as: {:?}", t.elapsed());
    let r = riviera_core::profiler::report(Some(15), riviera_core::profiler::Sort::SelfTime);
    for row in &r.rows { println!("SROW {:<40} calls {:>7} total {:>9.1} self {:>9.1}", row.name, row.calls, row.total_ms, row.self_ms); }
}
