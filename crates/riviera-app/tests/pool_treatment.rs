// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Pool water treatment (ARC-POOL-009): turnover, flow, filters, backwash, surge, skimmers,
//! pipes and drains on the MAHC's basis and on Sport England's and PWTAG's.

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

#[test]
fn a_public_pool_on_both_bases() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // 25 × 12.5 m, 1.0 m to 5 ft at 15 m and 2.0 m at the deep end; skimmers.
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 12.5], [0.0, 0.0], [25.0, 0.0], [25.0, 12.5]] }));
    let v = f(&ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"]);
    // The MAHC: 6 h over the pool and its 40.7 L/m² surge; 36.7 m/h filters.
    let t = ok(&mut app, "pool.treatment", json!({}))[0].clone();
    assert_eq!((t["basis"].as_str(), f(&t["turnover"])), (Some("us"), 6.0), "{t:#}");
    let surge = 0.0407 * 312.5;
    let flow = (v + surge) / 6.0;
    assert!(near(f(&t["surge_volume"]), surge, 1e-9) && near(f(&t["flow"]), flow, 1e-9), "{t:#}");
    assert!(near(f(&t["filter_area"]), flow / 36.7, 1e-9) && near(f(&t["filter_area_each"]), flow / 36.7 / 2.0, 1e-9), "{t:#}");
    // Seven skimmers (one per 46 m²); 12.5 m is wider than a skimmer pool should be.
    assert_eq!(t["skimmers"].as_u64(), Some(7));
    assert!(t["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("30 ft (9.1 m) wide")), "{t:#}");
    // The suction pipe at 1.8 m/s: d = √(4Q/πv).
    assert!(near(f(&t["suction"]["diameter"]), (4.0 * flow / 3600.0 / (std::f64::consts::PI * 1.8)).sqrt() * 1000.0, 1e-6), "{t:#}");
    assert!(near(f(&t["plant_room"][0]), 0.15 * 312.5, 1e-9) && near(f(&t["plant_room"][1]), 0.30 * 312.5, 1e-9));
    // Three main drains: each carries half the flow.
    let ty = ok(&mut app, "furnishing.load", json!({ "names": ["Main drain cover 300 × 300"] }))["Main drain cover 300 × 300"].clone();
    for x in [20.0, 21.5, 23.0] {
        ok(&mut app, "mesh_object.place", json!({ "type_id": ty, "point": [x, 6.25], "level": level, "offset": -2.2 }));
    }
    let t = ok(&mut app, "pool.treatment", json!({ "pool_id": pool }))[0].clone();
    assert!(t["main_drains"] == 3 && near(f(&t["flow_per_drain"]), flow / 2.0, 1e-9), "{t:#}");
    // Sport England and PWTAG: 4 h for a swimming pool, 25 m/h filters, backwash 30 m/h for 7
    // minutes, the balance tank half of 104 bathers at 80 L plus a backwash.
    ok(&mut app, "code.set", json!({ "code": "uk_adk", "occupancy": "other" }));
    let t = ok(&mut app, "pool.treatment", json!({}))[0].clone();
    assert_eq!((t["basis"].as_str(), f(&t["turnover"])), (Some("uk"), 4.0), "{t:#}");
    let flow = v / 4.0;
    let each = flow / 25.0 / 2.0;
    let backwash = each * 30.0 * 7.0 / 60.0;
    assert!(near(f(&t["flow"]), flow, 1e-9) && near(f(&t["backwash_volume"]), backwash, 1e-9), "{t:#}");
    assert!(near(f(&t["surge_volume"]), 0.5 * 312.5 / 3.0 * 0.08 + backwash, 1e-9), "{t:#}");
    assert!(near(f(&t["bathers_per_hour"]), flow / 1.7, 1e-9), "{t:#}");
}

#[test]
fn wading_and_diving_pools_turn_over_at_their_own_rates() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let wading = ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "wading", "origin": [0.0, 0.0] } }));
    let t = ok(&mut app, "pool.treatment", json!({ "pool_id": wading }))[0].clone();
    assert_eq!(f(&t["turnover"]), 1.0);
    assert!(t["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("secondary disinfection")), "{t:#}");
    // A deck-level diving pool with a board: 8 h, and gutters at 125 % of the flow.
    let well = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[20.0, 10.0], [20.0, 0.0], [34.0, 0.0], [34.0, 10.0]], "profile": [{ "at": 0.0, "depth": 3.8 }], "edges": [{ "kind": "deck_level" }] }));
    let ty = ok(&mut app, "furnishing.load", json!({ "names": ["Diving board 1 m, on stand"] }))["Diving board 1 m, on stand"].clone();
    ok(&mut app, "mesh_object.place", json!({ "type_id": ty, "point": [20.0 + 1.9 - 2.4384, 5.0], "level": level, "rotation": std::f64::consts::FRAC_PI_2 }));
    let t = ok(&mut app, "pool.treatment", json!({ "pool_id": well }))[0].clone();
    assert!(t["kind"].as_str().unwrap().starts_with("diving pool") && f(&t["turnover"]) == 8.0, "{t:#}");
    assert!(t["skimmers"].is_null() && near(f(&t["gutter_capacity"]), 1.25 * f(&t["flow"]), 1e-9), "{t:#}");
    // A turnover longer than the table's is a note.
    let t = ok(&mut app, "pool.treatment", json!({ "pool_id": well, "turnover": 10.0 }))[0].clone();
    assert!(t["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().starts_with("A 10.0 h turnover is longer")), "{t:#}");
}
