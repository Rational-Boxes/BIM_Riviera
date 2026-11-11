// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! The barrier round a pool (ARC-POOL-012): enclosure found from railings and walls, heights,
//! gaps and openings, gates' swing, and doors in a house wall, against the ISPSC and the MAHC.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

/// The findings of rules whose id contains `what`: (severity, message).
fn findings(c: &Value, what: &str) -> Vec<(String, String)> {
    c["findings"].as_array().unwrap().iter().filter(|x| x["rule"].as_str().unwrap().contains(what)).map(|x| (x["severity"].as_str().unwrap().to_string(), x["message"].as_str().unwrap().to_string())).collect()
}

fn setup(class: &str) -> (tempfile::TempDir, App, Value, Value) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "class": class, "outline": [[0.0, 5.0], [0.0, 0.0], [10.0, 0.0], [10.0, 5.0]], "profile": [{ "at": 0.0, "depth": 1.0 }, { "at": 10.0, "depth": 1.4 }] }));
    (dir, app, level, pool)
}

#[test]
fn a_fence_and_its_gate() {
    let (_dir, mut app, level, pool) = setup("public");
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(findings(&c, "305.2").iter().any(|(s, m)| s == "error" && m.starts_with("nothing encloses the pool")), "{c:#}");
    // A 1.25 m fence 2 in off the ground, its balusters 95 mm apart, all round the deck.
    let fence = ok(&mut app, "railing_type.create", json!({ "name": "Pool fence", "height": 1.25, "bottom_rail_height": 0.05, "baluster": 0.025, "baluster_spacing": 0.12 }));
    let rail = ok(&mut app, "railing.create", json!({ "type_id": fence, "path": [[-2.0, -2.0], [12.0, -2.0], [12.0, 7.0], [-2.0, 7.0]], "closed": true, "base_level": level }));
    let b = ok(&mut app, "pool.barriers", json!({}))[0].clone();
    assert!(b["enclosed"] == true && (115.0..126.0).contains(&f(&b["area"])), "{b:#}");
    let r = &b["barriers"][0];
    assert!(r["element"] == rail && f(&r["height"]) == 1.25 && (f(&r["opening"]) - 0.095).abs() < 1e-9 && f(&r["rails_apart"]) > 1.143, "{b:#}");
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(findings(&c, "305").is_empty(), "{c:#}");
    // An opening in it: no longer enclosed, and where.
    ok(&mut app, "railing.add_opening", json!({ "railing_id": rail, "point": [5.0, -2.0], "width": 1.0 }));
    let b = ok(&mut app, "pool.barriers", json!({}))[0].clone();
    assert!(b["enclosed"] == false && (f(&b["openings"][0][0]) - 5.0).abs() < 0.01, "{b:#}");
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(findings(&c, "305.2").iter().any(|(_, m)| m.contains("open at (5.0, -2.0)")), "{c:#}");
    // A gate there instead, swinging in toward the pool, then out.
    let key = ok(&mut app, "railing.features", json!({ "railing_id": rail }))[0]["key"].clone();
    ok(&mut app, "railing.remove_feature", json!({ "railing_id": rail, "key": key }));
    let gate = ok(&mut app, "railing.add_gate", json!({ "railing_id": rail, "point": [5.0, -2.0] }));
    let b = ok(&mut app, "pool.barriers", json!({}))[0].clone();
    assert!(b["enclosed"] == true && b["gates"][0]["element"] == gate && b["gates"][0]["swings_out"] == false, "{b:#}");
    let c = ok(&mut app, "pool.check", json!({}));
    assert_eq!(c["findings"].as_array().unwrap().iter().filter(|x| x["rule"] == "ISPSC 305.3").map(|x| x["element"].clone()).collect::<Vec<_>>(), vec![gate.clone()], "{c:#}");
    let key = ok(&mut app, "railing.features", json!({ "railing_id": rail }))[0]["key"].clone();
    ok(&mut app, "railing.remove_feature", json!({ "railing_id": rail, "key": key }));
    let gate = ok(&mut app, "railing.add_gate", json!({ "railing_id": rail, "point": [5.0, -2.0], "swing_out": true }));
    // Its hardware (ARC-POOL-016): self-closing and self-latching, the latch 53 in up.
    ok(&mut app, "railing.gate_hardware", json!({ "gate_id": gate, "latch_height": 1.35 }));
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(findings(&c, "305").is_empty(), "{c:#}");
    // The MAHC asks for 6 ft.
    ok(&mut app, "rules.set_packs", json!({ "domain": "pool", "packs": ["us_mahc_2024"] }));
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(findings(&c, "4.8.6").iter().any(|(s, m)| s == "error" && m.contains("at least 72 in")), "{c:#}");
}

#[test]
fn a_low_guard_rail_and_a_house_wall() {
    let (_dir, mut app, level, _pool) = setup("residential");
    // The default guard rail (1.0 m, 80 mm under it, balusters 100 mm apart between rails 0.88 m
    // apart) on three sides; the house's wall, with a door, on the fourth.
    let rail = ok(&mut app, "railing.create", json!({ "path": [[-2.0, 7.0], [-2.0, -2.0], [12.0, -2.0], [12.0, 7.0]], "base_level": level }));
    let wall = ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [-2.5, 7.0], "end": [12.5, 7.0] } }, "base_level": level, "top_constraint": { "unconnected": 3.0 } }));
    let door = ok(&mut app, "door.place", json!({ "host_wall_id": wall, "point_along": 7.5 }));
    let b = ok(&mut app, "pool.barriers", json!({}))[0].clone();
    assert!(b["enclosed"] == true && b["barriers"].as_array().unwrap().len() == 2, "{b:#}");
    assert!(b["wall_openings"].as_array().unwrap().iter().any(|o| o["element"] == door && o["kind"] == "door"), "{b:#}");
    let c = ok(&mut app, "pool.check", json!({}));
    let found = findings(&c, "305.2");
    assert!(found.iter().any(|(s, m)| s == "error" && m.contains("at least 48 in")), "{c:#}");
    assert!(found.iter().any(|(s, m)| s == "error" && m.contains("at most 1.75 in where its rails are under 45 in apart")), "{c:#}");
    assert!(found.iter().any(|(s, m)| s == "note" && m.contains("UL 2017")), "{c:#}");
    assert!(c["findings"].as_array().unwrap().iter().filter(|x| x["rule"] == "ISPSC 305.2" && x["severity"] == "error").all(|x| x["element"] == rail), "{c:#}");
}
