// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Swimouts and nosing stripes (ARC-POOL-015): a seat in an alcove behind the pool's wall, the
//! contrasting stripes on treads and seats, the checks that measure them, and their IFC.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn errors(app: &mut App) -> Vec<String> {
    let c = ok(app, "pool.check", json!({}));
    c["findings"].as_array().unwrap().iter().filter(|f| f["severity"] == "error").map(|f| format!("{}: {}", f["rule"].as_str().unwrap(), f["message"].as_str().unwrap())).collect()
}

/// A public pool, 15 × 8 m, 1.0 m deep at its west wall to 2.0 m at its east wall, marked and
/// fenced, under the MAHC and the ISPSC, with stairs at the shallow end and recessed steps at
/// the deep end.
fn setup() -> (tempfile::TempDir, App, Value) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    ok(&mut app, "rules.set_packs", json!({ "domain": "pool", "packs": ["us_mahc_2024", "us_ispsc", "us_vgb"] }));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let p = [[-6.0, -6.0], [21.0, -6.0], [21.0, 14.0], [-6.0, 14.0]];
    let loops = json!([(0..4).map(|k| json!({ "line": { "start": p[k], "end": p[(k + 1) % 4] } })).collect::<Vec<_>>()]);
    ok(&mut app, "floor.create", json!({ "loops": loops, "level_id": level }));
    let profile = json!([{ "at": 0.0, "depth": 1.0 }, { "at": 7.0, "depth": 1.524 }, { "at": 9.0, "depth": 2.0 }]);
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 8.0], [0.0, 0.0], [15.0, 0.0], [15.0, 8.0]], "profile": profile }));
    ok(&mut app, "pool.depth_markings", json!({ "pool_id": pool }));
    let fence = ok(&mut app, "railing_type.create", json!({ "name": "Enclosure", "height": 1.85, "bottom_rail_height": 0.05, "baluster": 0.025, "baluster_spacing": 0.12 }));
    let rail = ok(&mut app, "railing.create", json!({ "type_id": fence, "path": [[-5.8, -5.8], [20.8, -5.8], [20.8, 13.8], [-5.8, 13.8]], "closed": true, "base_level": level }));
    ok(&mut app, "railing.add_gate", json!({ "railing_id": rail, "point": [5.0, -5.8], "swing_out": true }));
    (dir, app, pool)
}

#[test]
fn nosing_stripes_on_the_stairs() {
    let (_dir, mut app, pool) = setup();
    let s = ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "stairs", "edge": 1, "at": 4.0 }));
    assert!((f(&s["nosing"]) - 0.0381).abs() < 1e-9, "{s:#}");
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "recessed_steps", "edge": 3, "at": 4.0 }));
    assert_eq!(errors(&mut app), Vec::<String>::new());
    let set = |app: &mut App, nosing: f64| ok(app, "pool.set", json!({ "pool_id": pool, "entries": [{ "kind": "stairs", "edge": 1, "at": 4.0, "nosing": nosing }, { "kind": "recessed_steps", "edge": 3, "at": 4.0 }] }));
    set(&mut app, 0.0);
    let e = errors(&mut app);
    assert!(e.contains(&"MAHC 2024 4.5.4.2: stairs 1 (edge 1) has no contrasting nosing stripes; 1–2 in".to_owned()), "{e:#?}");
    assert!(e.iter().any(|m| m.starts_with("ISPSC 2021 411.2 (public): stairs 1 (edge 1) has no contrasting nosing stripes")), "{e:#?}");
    set(&mut app, 0.06);
    assert!(errors(&mut app).iter().any(|m| m.starts_with("MAHC 2024 4.5.4.2: stairs 1 (edge 1): its nosing stripes are 2.4 in wide; 1–2 in")));
}

#[test]
fn a_swimout_behind_the_wall() {
    let (dir, mut app, pool) = setup();
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "stairs", "edge": 1, "at": 4.0 }));
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "recessed_steps", "edge": 3, "at": 4.0 }));
    let before = f(&ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"]);
    // On the south wall (edge 2) at 4 m: 1.5 m wide, 0.6 m out behind the wall, the seat 0.45 m
    // under the water; the water fills the alcove over the seat.
    let s = ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "swimout", "point": [4.0, 0.0] }));
    assert!(s["edge"] == 2 && (f(&s["area"]) - 0.9).abs() < 1e-9 && (f(&s["seat"]) - 0.45).abs() < 1e-9 && (f(&s["recess"]) - 0.6).abs() < 1e-9 && s.get("handrails").is_none(), "{s:#}");
    let after = f(&ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"]);
    assert!((after - before - 1.5 * 0.6 * 0.45).abs() < 1e-6, "{}", after - before);
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    assert_eq!(errors(&mut app), Vec::<String>::new());
    // Too shallow out from the wall, too small and too deep a seat, and a thin stripe fail the
    // ISPSC.
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "entries": [
        { "kind": "stairs", "edge": 1, "at": 4.0 },
        { "kind": "recessed_steps", "edge": 3, "at": 4.0 },
        { "kind": "swimout", "edge": 2, "at": 4.0, "width": 0.5, "depth": 0.25, "seat": 0.6, "nosing": 0.015 },
    ] }));
    let e = errors(&mut app);
    for want in [
        "ISPSC 2021 411.5.1 (public): swimout 3 (edge 2) is 9.8 in deep; at least 11 in",
        "ISPSC 2021 411.5.1 (public): swimout 3 (edge 2)'s seat is 194 in²; at least 240 in²",
        "ISPSC 2021 411.5.1 (public): swimout 3 (edge 2): its seat is 23.6 in below the water; at most 20 in",
        "ISPSC 2021 411.5.1 (public): swimout 3 (edge 2): its nosing stripes are 0.6 in wide; 0.75–2 in",
    ] {
        assert!(e.iter().any(|m| m.starts_with(want)), "{want}\n{e:#?}");
    }
    // Saved, the stripes are coverings in the pool's assembly.
    let model = dir.path().join("pool.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let rows = riviera_ifc::IfcModel::open(&model).unwrap().step_rows().unwrap();
    assert!(rows.iter().any(|r| r.class == "IFCCOVERING" && r.attributes.contains("'NOSING'")));
}
