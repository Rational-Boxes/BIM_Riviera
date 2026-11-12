// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Event and dining seating (ARC-INT-006): a ballroom set out for a banquet, a theatre, a
//! classroom and a reception, a boardroom and a restaurant, with the IBC's occupant loads, exits
//! and egress width.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

/// A room of walls from (x0, y0) to (x1, y1) with `doors` doors in its south wall; the room.
fn room(app: &mut App, level: &Value, x0: f64, y0: f64, x1: f64, y1: f64, doors: usize) -> Value {
    let p = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    let mut walls = Vec::new();
    for k in 0..4 {
        walls.push(ok(app, "wall.create", json!({ "curve": { "line": { "start": p[k], "end": p[(k + 1) % 4] } }, "base_level": level, "top_constraint": { "unconnected": 4.0 } })));
    }
    for d in 0..doors {
        ok(app, "door.place", json!({ "host_wall_id": walls[0], "point_along": (x1 - x0) * (d as f64 + 1.0) / (doors as f64 + 1.0) }));
    }
    ok(app, "room.place", json!({ "level_id": level, "point": [(x0 + x1) / 2.0, (y0 + y1) / 2.0] }))
}

fn count(v: &Value, k: &str) -> u64 {
    v[k].as_u64().unwrap_or_else(|| panic!("{k}: {v:#}"))
}

#[test]
fn a_ballroom_four_ways() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let ballroom = room(&mut app, &level, 0.0, 0.0, 30.0, 20.0, 3);
    // Banquet: 72 in rounds 11.7 ft apart, 44 in clear of the walls: 7 × 5 rounds of ten.
    let b = ok(&mut app, "seating.layout", json!({ "room_id": ballroom, "style": "banquet" }));
    assert_eq!((count(&b, "tables"), count(&b, "chairs")), (35, 350), "{b:#}");
    // The room's net area is inside its walls' faces; at 15 ft² it holds more than 350.
    let area_ft2 = b["area"].as_f64().unwrap() / (0.3048 * 0.3048);
    assert_eq!(count(&b, "code_load"), (area_ft2 / 15.0).floor() as u64);
    assert_eq!((count(&b, "exits_needed"), count(&b, "doors")), (2, 3), "{b:#}");
    assert!(b["findings"].as_array().unwrap().is_empty(), "{b:#}");
    // Theatre: rows of chairs in four blocks; at 7 ft² the code load is larger and needs 3 exits.
    let t = ok(&mut app, "seating.layout", json!({ "room_id": ballroom, "style": "theatre" }));
    assert!(count(&t, "seats") > 600 && count(&t, "tables") == 0, "{t:#}");
    assert_eq!(count(&t, "exits_needed"), 3);
    // The banquet's pieces are gone: only the theatre's chairs are marked with the room.
    let st = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    assert_eq!(st.iter().filter(|e| e.param("rxd:seating").is_some()).count() as u64, count(&t, "chairs"));
    // Classroom: seminar tables for three.
    let c = ok(&mut app, "seating.layout", json!({ "room_id": ballroom, "style": "classroom" }));
    assert!(count(&c, "tables") > 20 && count(&c, "chairs") == 3 * count(&c, "tables"), "{c:#}");
    // Reception: cocktail tables, standing at 7 ft².
    let r = ok(&mut app, "seating.layout", json!({ "room_id": ballroom, "style": "reception" }));
    assert!(count(&r, "seats") as f64 > area_ft2 / 7.0 - 1.0 && count(&r, "chairs") == 0, "{r:#}");
    // Banquet rounds 9 ft apart are denser than 15 ft² a seat: a note to take the
    // increased-load route.
    let dense = ok(&mut app, "seating.layout", json!({ "room_id": ballroom, "style": "banquet", "round": 60, "pitch": 2.75 }));
    assert!(count(&dense, "seats") > count(&dense, "code_load"), "{dense:#}");
    assert!(dense["findings"].as_array().unwrap().iter().any(|f| f["rule"] == "IBC 1004.5.1"), "{dense:#}");
    // Cleared.
    let gone = ok(&mut app, "seating.layout", json!({ "room_id": ballroom, "style": "banquet", "clear": true }));
    assert_eq!(count(&gone, "chairs"), 0);
}

#[test]
fn a_boardroom_and_a_restaurant() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let board = room(&mut app, &level, 0.0, 0.0, 7.0, 5.0, 1);
    let b = ok(&mut app, "seating.layout", json!({ "room_id": board, "style": "boardroom", "perimeter": 0.6 }));
    assert_eq!((count(&b, "tables"), count(&b, "chairs"), count(&b, "exits_needed")), (1, 8, 1), "{b:#}");
    // A 15 × 10 m restaurant with one door: four-tops 7 ft apart; more than 49 people need two
    // exits.
    let r = room(&mut app, &level, 20.0, 0.0, 35.0, 10.0, 1);
    let d = ok(&mut app, "seating.layout", json!({ "room_id": r, "style": "dining" }));
    assert!(count(&d, "tables") >= 12 && count(&d, "chairs") == 4 * count(&d, "tables"), "{d:#}");
    assert!(d["findings"].as_array().unwrap().iter().any(|f| f["rule"] == "IBC 1006.3.2" && f["severity"] == "error"), "{d:#}");
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
}
