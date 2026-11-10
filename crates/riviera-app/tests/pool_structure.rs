// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A pool's structural loads (ARC-POOL-010): the water's and shell's weight, the walls'
//! hydrostatic pressure, uplift on the empty shell, equipment point loads, and an elevated
//! pool's weight put on its slab's analytical panel.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn notes(s: &Value) -> Vec<String> {
    s["notes"].as_array().unwrap().iter().map(|n| n.as_str().unwrap().to_string()).collect()
}

#[test]
fn an_in_ground_pool_full_empty_and_floating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // 10 × 5 m, 1.5 m deep throughout, a 0.3 m concrete shell.
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 5.0], [0.0, 0.0], [10.0, 0.0], [10.0, 5.0]], "profile": [{ "at": 0.0, "depth": 1.5 }, { "at": 10.0, "depth": 1.5 }] }));
    let i = ok(&mut app, "pool.info", json!({ "pool_id": pool }));
    let s = ok(&mut app, "pool.structure", json!({}))[0].clone();
    assert!(near(f(&s["water_weight"]), 9.81 * f(&i["volume"]), 1e-6) && near(f(&i["volume"]), 75.0, 1e-6), "{s:#}");
    assert!(near(f(&s["shell_weight"]), 24.5 * f(&i["shell_volume"]), 1e-6), "{s:#}");
    assert!(near(f(&s["footprint"]), 10.6 * 5.6, 1e-6) && near(f(&s["floor_pressure"]), 9.81 * 1.5, 1e-9), "{s:#}");
    assert!(near(f(&s["wall"]["pressure"]), 9.81 * 1.5, 1e-9) && near(f(&s["wall"]["moment"]), 9.81 * 1.5f64.powi(3) / 6.0, 1e-9), "{s:#}");
    assert!(near(f(&s["full"]), (f(&s["water_weight"]) + f(&s["shell_weight"])) / f(&s["footprint"]), 1e-9), "{s:#}");
    assert!(s["elevated"] == false && s.get("uplift").is_none() && notes(&s).iter().any(|n| n.contains("No water table given")), "{s:#}");
    // The water table at the level: 1.95 m of head under the whole footprint, more than the
    // shell's weight carries.
    let s = ok(&mut app, "pool.structure", json!({ "pool_id": pool, "groundwater": 0.0 }))[0].clone();
    let u = &s["uplift"];
    assert!(near(f(&u["head"]), 1.95, 1e-6) && near(f(&u["force"]), 9.81 * 1.95 * 59.36, 0.01 * 9.81 * 1.95 * 59.36), "{s:#}");
    assert!(u["ok"] == false && near(f(&u["ratio"]), f(&s["shell_weight"]) / f(&u["force"]), 1e-9), "{s:#}");
    assert!(notes(&s).iter().any(|n| n.contains("relief valve")), "{s:#}");
    // 3 m down: no uplift.
    let s = ok(&mut app, "pool.structure", json!({ "pool_id": pool, "groundwater": -3.0 }))[0].clone();
    assert!(near(f(&s["uplift"]["force"]), 0.0, 1e-9) && s["uplift"]["ok"] == true, "{s:#}");
    // A lift on the deck: its advisory point load.
    let ty = ok(&mut app, "furnishing.load", json!({ "names": ["Pool lift, 300 lb"] }))["Pool lift, 300 lb"].clone();
    let lift = ok(&mut app, "mesh_object.place", json!({ "type_id": ty, "point": [5.0, 5.6], "level": level }));
    let s = ok(&mut app, "pool.structure", json!({ "pool_id": pool }))[0].clone();
    let p = &s["point_loads"];
    assert!(p.as_array().unwrap().len() == 1 && p[0]["element"] == lift && f(&p[0]["load"]) == 4.0, "{s:#}");
}

#[test]
fn a_rooftop_pool_on_its_slab() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let roof = ok(&mut app, "level.create", json!({ "elevation": 12.0, "name": "Roof" }));
    // 8 × 4 m, 1.2 m deep, a 0.25 m shell, tiled.
    let pool = ok(&mut app, "pool.create", json!({ "level": roof, "outline": [[1.0, 5.0], [1.0, 1.0], [9.0, 1.0], [9.0, 5.0]], "profile": [{ "at": 0.0, "depth": 1.2 }, { "at": 8.0, "depth": 1.2 }] }));
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "shell": { "kind": "cast_concrete", "thickness": 0.25, "tile": 0.012, "membrane": false } }));
    let s = ok(&mut app, "pool.structure", json!({}))[0].clone();
    assert!(s["elevated"] == true && near(f(&s["deflection_limit"]), 4.0 / 360.0 * 1000.0, 1e-9), "{s:#}");
    assert!(near(f(&s["peak"]), 9.81 * 1.2 + 24.5 * 0.25 + 1.25, 1e-9) && f(&s["finishes"]) > 0.0, "{s:#}");
    assert!(notes(&s).iter().any(|n| n.contains("geofoam")), "{s:#}");
    // The roof slab's panel, 10 × 6 m, and the two cases.
    let panel = ok(&mut app, "analytical_panel.create_by_boundary", json!({ "boundary": [[0.0, 0.0, 12.0], [10.0, 0.0, 12.0], [10.0, 6.0, 12.0], [0.0, 6.0, 12.0]], "thickness": 0.3 }));
    let dead = ok(&mut app, "load_case.create", json!({ "name": "D", "nature": "D" }));
    let water = ok(&mut app, "load_case.create", json!({ "name": "Water", "nature": "D" }));
    let a = ok(&mut app, "pool.apply_loads", json!({ "pool_id": pool, "panel_id": panel, "dead_case_id": dead, "water_case_id": water }));
    assert!(near(f(&a["panel_area"]), 60.0, 1e-6), "{a:#}");
    assert!(near(f(&a["water_pressure"]), f(&s["water_weight"]) * 1000.0 / 60.0, 1e-6), "{a:#}");
    assert!(near(f(&a["dead_pressure"]), (f(&s["shell_weight"]) + f(&s["finishes"])) * 1000.0 / 60.0, 1e-6), "{a:#}");
    // A panel much smaller than the pool is refused.
    let small = ok(&mut app, "analytical_panel.create_by_boundary", json!({ "boundary": [[0.0, 0.0, 12.0], [2.0, 0.0, 12.0], [2.0, 2.0, 12.0], [0.0, 2.0, 12.0]], "thickness": 0.3 }));
    let e = app.execute("pool.apply_loads", json!({ "pool_id": pool, "panel_id": small, "dead_case_id": dead, "water_case_id": water }), &Caller::user()).unwrap_err();
    assert!(format!("{e:?}").contains("pool.panel_small"), "{e:?}");
}
