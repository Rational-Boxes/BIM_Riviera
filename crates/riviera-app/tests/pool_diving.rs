// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Diving envelopes (ARC-POOL-006): a 1 m board's plummet checked against the MAHC's Table
//! 4.8.2.3 (distances, depths, clear overhead) and the pool's own entries standing in its
//! envelope.

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

/// The MAHC diving table's findings.
fn diving(app: &mut App) -> Vec<String> {
    let c = ok(app, "pool.check", json!({}));
    c["findings"].as_array().unwrap().iter().filter(|f| f["rule"] == "MAHC 2024 Table 4.8.2.3").map(|f| f["message"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn a_one_metre_board_is_checked_against_its_table() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    ok(&mut app, "rules.set_packs", json!({ "domain": "pool", "packs": ["us_mahc_2024"] }));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "floor.create", json!({ "loops": [square([-6.0, -4.0], [21.0, 12.0])], "level_id": level }));
    // A diving pool 15 × 8 m, 3.7 m deep for 9 m from its west wall, rising to 2.0 m.
    let profile = json!([{ "at": 0.0, "depth": 3.7 }, { "at": 9.0, "depth": 3.7 }, { "at": 15.0, "depth": 2.0 }]);
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 8.0], [0.0, 0.0], [15.0, 0.0], [15.0, 8.0]], "profile": profile }));
    // A 1 m board on the west deck pointing east, its tip 1.9 m out over the water (A: 6 ft).
    let ty = ok(&mut app, "furnishing.load", json!({ "names": ["Diving board 1 m, on stand"] }))["Diving board 1 m, on stand"].clone();
    let board = ok(&mut app, "mesh_object.place", json!({ "type_id": ty, "point": [1.9 - 2.4384, 4.0], "level": level, "rotation": std::f64::consts::FRAC_PI_2 }));
    assert_eq!(diving(&mut app), Vec::<String>::new());
    // 3.0 m deep: short of H (12 ft), K (11.28 ft at 16.5 ft ahead) and M (11.63 ft each side).
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "profile": [{ "at": 0.0, "depth": 3.0 }] }));
    let d = diving(&mut app);
    assert_eq!(d.len(), 3, "{d:#?}");
    assert!(d[0].ends_with("H (the depth at the plummet): 3.00 m (9′ 10″); at least 3.66 m (12 ft)"), "{d:#?}");
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "profile": profile }));
    // A bench 2 m deep along the north wall stands up into the envelope (2.51 m each side).
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "bench", "edge": 4, "at": 11.0, "depth": 2.0 }));
    let d = diving(&mut app);
    assert!(d.len() == 1 && d[0].starts_with("bench 1 (edge 4) stands up into"), "{d:#?}");
    ok(&mut app, "pool.remove_entry", json!({ "pool_id": pool, "index": 1 }));
    // Moved 2 m from the south wall: short of B (10 ft).
    ok(&mut app, "mesh_object.set", json!({ "element_ids": [board], "point": [1.9 - 2.4384, 2.0] }));
    let d = diving(&mut app);
    assert!(d.iter().any(|m| m.ends_with("B (to the side walls): 2.00 m (6′ 7″); at least 3.05 m (10 ft)")), "{d:#?}");
    ok(&mut app, "mesh_object.set", json!({ "element_ids": [board], "point": [1.9 - 2.4384, 4.0] }));
    // A floor 4 m up over the pool: under 16 ft (E) over the board.
    let l2 = ok(&mut app, "level.create", json!({ "elevation": 4.0 }));
    let slab = ok(&mut app, "floor.create", json!({ "loops": [square([-6.0, -4.0], [21.0, 12.0])], "level_id": l2 }));
    let c = ok(&mut app, "pool.check", json!({}));
    let e: Vec<&Value> = c["findings"].as_array().unwrap().iter().filter(|f| f["rule"] == "MAHC 2024 Table 4.8.2.3").collect();
    assert!(e.len() == 1 && e[0]["element"] == slab && e[0]["message"].as_str().unwrap().contains("E (clear over the board at the plummet)"), "{c:#}");
    ok(&mut app, "element.delete", json!({ "element_ids": [slab] }));
    // Marked for competition, World Aquatics' springboard column: 3.3 m at the plummet is short of
    // its 3.40 m (H), with its note.
    ok(&mut app, "rules.set_packs", json!({ "domain": "pool", "packs": ["world_aquatics"] }));
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "class": "competition", "profile": [{ "at": 0.0, "depth": 3.3 }] }));
    let c = ok(&mut app, "pool.check", json!({}));
    let wa: Vec<&str> = c["findings"].as_array().unwrap().iter().filter(|f| f["rule"] == "FR 2017–2021 Annex 1.2: springboards").map(|f| f["message"].as_str().unwrap()).collect();
    assert!(wa.len() == 1 && wa[0].contains("H (the depth at the plummet): 3.30 m") && wa[0].ends_with("at least 3.40 m (11′ 2″); verify against the current World Aquatics text"), "{wa:#?}");
}
