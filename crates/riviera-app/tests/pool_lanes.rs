// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Competition lanes (ARC-POOL-005): the competition templates' lanes and markings, `pool.lanes`,
//! and the World Aquatics pack's course length, lane widths and starting-platform depth.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn by_pack<'a>(c: &'a Value, pack: &str) -> Vec<&'a str> {
    // The lane fittings' notes (ARC-POOL-017) are pool_lane_fittings.rs's.
    c["findings"].as_array().unwrap().iter().filter(|f| f["pack"] == pack).map(|f| f["message"].as_str().unwrap()).filter(|m| !m.contains("(pool.lanes fittings)")).collect()
}

fn app() -> (tempfile::TempDir, App, Value) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    (dir, app, level)
}

#[test]
fn a_50_m_pool_has_its_lanes_and_markings() {
    let (dir, mut app, level) = app();
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "competition_50m", "origin": [0.0, 0.0] } }));
    let i = ok(&mut app, "pool.info", json!({ "pool_id": pool }));
    let l = &i["lanes"];
    // 50.03 m wall to wall, 50.01 m between the pads; eight lanes in 20 m, the outer two 2.4 m.
    assert!(near(l["length"].as_f64().unwrap(), 50.03, 1e-9) && near(l["course"].as_f64().unwrap(), 50.01, 1e-9), "{l:#}");
    let widths: Vec<f64> = l["widths"].as_array().unwrap().iter().map(|w| (w.as_f64().unwrap() * 1000.0).round() / 1000.0).collect();
    assert_eq!(widths, vec![2.4, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.4]);
    assert!(near(l["centres"][0].as_f64().unwrap(), 1.3, 1e-9), "{l:#}");
    let c = ok(&mut app, "pool.check", json!({}));
    assert_eq!(c["pools"][0]["class"], "competition");
    assert!(by_pack(&c, "world_aquatics").is_empty(), "{c:#}");
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    // Saved, the markings are a LANEMARKING covering in the pool's assembly.
    let model = dir.path().join("pool.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let rows = riviera_ifc::IfcModel::open(&model).unwrap().step_rows().unwrap();
    assert!(rows.iter().any(|r| r.class == "IFCCOVERING" && r.attributes.contains("'LANEMARKING'")));
    // Without markings, no LANEMARKING; the volume is the same either way.
    let v = ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"].as_f64().unwrap();
    ok(&mut app, "pool.lanes", json!({ "pool_id": pool, "count": 8, "markings": false }));
    assert!(near(ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"].as_f64().unwrap(), v, 1e-9));
    ok(&mut app, "file.save", json!({}));
    let rows = riviera_ifc::IfcModel::open(&model).unwrap().step_rows().unwrap();
    assert!(!rows.iter().any(|r| r.class == "IFCCOVERING" && r.attributes.contains("'LANEMARKING'")));
}

#[test]
fn world_aquatics_checks_lanes_and_starting_platforms() {
    let (_dir, mut app, level) = app();
    // A 25 m pool, 1.2 m deep at its west end falling to 2.0 m at its east end.
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "competition_25m", "origin": [0.0, 0.0] }, "profile": [{ "at": 0.0, "depth": 1.2 }, { "at": 25.03, "depth": 2.0 }] }));
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(by_pack(&c, "world_aquatics").is_empty(), "{c:#}");
    // A starting platform at the shallow west end, facing east: 1.23 m at 1.0 m out.
    let ty = ok(&mut app, "furnishing.load", json!({ "names": ["Starting platform 24 × 32 in"] }))["Starting platform 24 × 32 in"].clone();
    let block = ok(&mut app, "mesh_object.place", json!({ "type_id": ty, "point": [-12.515 - 0.406, 1.25], "level": level, "rotation": std::f64::consts::FRAC_PI_2 }));
    let c = ok(&mut app, "pool.check", json!({}));
    let wa = by_pack(&c, "world_aquatics");
    assert_eq!(wa.len(), 1, "{c:#}");
    assert!(wa[0].starts_with("the water is 1.23 m deep 1.0–6.0 m out from the wall at") && wa[0].ends_with("at least 1.35 m; verify against the current World Aquatics text"), "{}", wa[0]);
    let f = c["findings"].as_array().unwrap().iter().find(|f| f["pack"] == "world_aquatics").unwrap();
    assert_eq!(f["element"], block);
    assert!(by_pack(&c, "us_ispsc").iter().any(|m| m.contains("1.20 m deep 0.0–1.0 m out")), "{c:#}");
    // Eight 2.4 m lanes leave 0.2 m outer lanes; ten do not fit.
    ok(&mut app, "pool.lanes", json!({ "pool_id": pool, "count": 8, "width": 2.4 }));
    let wa = by_pack(&ok(&mut app, "pool.check", json!({})), "world_aquatics").join("\n");
    assert!(wa.contains("lane 1 is 0.20 m wide; the outer lanes at least 2.40 m") && wa.contains("lane 2 is 2.40 m wide; the lanes at least 2.50 m"), "{wa}");
    assert!(app.execute("pool.lanes", json!({ "pool_id": pool, "count": 10 }), &Caller::user()).is_err());
}

#[test]
fn course_lengths_are_measured_between_the_pads() {
    let (_dir, mut app, level) = app();
    // A rectangle exactly 25 m wall to wall: with pads at both ends, 24.98 m is short.
    let short = ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "rectangle", "origin": [0.0, 0.0], "length": 25.0, "width": 15.0 }, "class": "competition", "lanes": { "count": 6 } }));
    let wa = by_pack(&ok(&mut app, "pool.check", json!({ "pool_id": short })), "world_aquatics").join("\n");
    assert!(wa.contains("24.980 m between the touch pads (25.000 m wall to wall, pads at 2 ends); a 25 m course is 25.000–25.010 m"), "{wa}");
    // With pads at one end, 24.99 m: still short. Without lanes, the course is unknown.
    ok(&mut app, "pool.lanes", json!({ "pool_id": short, "count": 6, "pads": 0 }));
    assert!(by_pack(&ok(&mut app, "pool.check", json!({ "pool_id": short })), "world_aquatics").is_empty());
    ok(&mut app, "pool.lanes", json!({ "pool_id": short, "count": 0 }));
    let c = ok(&mut app, "pool.check", json!({ "pool_id": short }));
    assert!(by_pack(&c, "world_aquatics").iter().any(|m| m.starts_with("no lanes are set (pool.lanes)")), "{c:#}");
    // A 25 yd pool is not a World Aquatics course: a note.
    let yards = ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "competition_25yd", "origin": [0.0, 40.0] } }));
    let c = ok(&mut app, "pool.check", json!({ "pool_id": yards }));
    let f: Vec<&Value> = c["findings"].as_array().unwrap().iter().filter(|f| f["pack"] == "world_aquatics" && !f["message"].as_str().unwrap().contains("(pool.lanes fittings)")).collect();
    assert!(f.len() == 1 && f[0]["severity"] == "note" && f[0]["message"].as_str().unwrap().starts_with("22.870 m between the touch pads is not a course of 50 m or 25 m"), "{c:#}");
}
