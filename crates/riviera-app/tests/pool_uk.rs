// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! The UK pool packs (ARC-POOL-007): Sport England's gradients, shallow depth, lanes, surround,
//! steps and ramps, and HSG179's bathing load, chosen in England; their findings are notes.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn square(a: [f64; 2], b: [f64; 2]) -> Value {
    let p = [[a[0], a[1]], [b[0], a[1]], [b[0], b[1]], [a[0], b[1]]];
    Value::Array((0..4).map(|k| json!({ "line": { "start": p[k], "end": p[(k + 1) % 4] } })).collect())
}

fn messages(c: &Value, pack: &str) -> Vec<String> {
    c["findings"].as_array().unwrap().iter().filter(|f| f["pack"] == pack).map(|f| {
        assert_eq!(f["severity"], "note", "guidance gives notes: {f}");
        f["message"].as_str().unwrap().to_owned()
    }).collect()
}

#[test]
fn a_community_pool_in_england() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    ok(&mut app, "code.set", json!({ "code": "uk_adk", "occupancy": "other" }));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // A 25 × 12.5 m community pool with a 2 m surround: Sport England's typical profile, 1.0 m,
    // 1:17 to 1.35 m, on to 1.8 m and 2.0 m.
    ok(&mut app, "floor.create", json!({ "loops": [square([-2.0, -2.0], [27.0, 14.5])], "level_id": level }));
    let profile = json!([{ "at": 0.0, "depth": 1.0 }, { "at": 6.0, "depth": 1.35 }, { "at": 15.0, "depth": 1.8 }, { "at": 25.0, "depth": 2.0 }]);
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 12.5], [0.0, 0.0], [25.0, 0.0], [25.0, 12.5]], "profile": profile }));
    ok(&mut app, "pool.depth_markings", json!({ "pool_id": pool }));
    let c = ok(&mut app, "pool.check", json!({}));
    assert_eq!(c["packs"], json!(["uk_sport_england", "uk_hsg179", "world_aquatics"]), "{c:#}");
    assert!(messages(&c, "uk_sport_england").is_empty() && messages(&c, "uk_hsg179").is_empty(), "{c:#}");
    // HSG179's starting point: 3 m² a bather.
    let load = c["pools"][0]["loads"].as_array().unwrap().iter().find(|l| l["pack"] == "uk_hsg179").cloned().unwrap();
    assert_eq!(load["bathers"].as_u64(), Some(104), "{load:#}");
    // Steeper between 0.9 and 1.35 m than 1 in 17; stairs with 0.25 m risers; a 1:12 ramp.
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "profile": [{ "at": 0.0, "depth": 0.9 }, { "at": 4.0, "depth": 1.35 }, { "at": 25.0, "depth": 2.0 }] }));
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "stairs", "edge": 1, "at": 3.0 }));
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "sloped_entry", "edge": 2, "at": 3.0, "reach": 0.4 }));
    let se = messages(&ok(&mut app, "pool.check", json!({})), "uk_sport_england").join("\n");
    assert!(se.contains("the floor from 0.00 m to 4.00 m falls 1:8.9 where the water is 0.80–1.35 m deep; at most 1:17.0"), "{se}");
    assert!(se.contains("its risers are") && se.contains("at most 5.5 in"), "{se}");
    assert!(se.contains("falls 1:12.0; at most 1:15.0"), "{se}");
    // Lanes of 1.9 m are under Sport England's 2.0 m.
    ok(&mut app, "pool.lanes", json!({ "pool_id": pool, "count": 6, "width": 1.9, "gap": 0.25 }));
    let se = messages(&ok(&mut app, "pool.check", json!({})), "uk_sport_england").join("\n");
    assert!(se.contains("lane 2 is 1.90 m wide; the lanes at least 2.00 m"), "{se}");
}
