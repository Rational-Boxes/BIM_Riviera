// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Moving in 3D (ARC-MOD-001): a vector's height raises walls, floors, roofs and stairs by their
//! offsets; a door keeps to its wall, taking only the part along it and the height.

use riviera_app::{App, AppConfig};
use riviera_core::{Caller, ElementId};
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn recipe(app: &App, id: &Value) -> Value {
    let s = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    s.get(ElementId(id.as_u64().unwrap())).unwrap().param("rxd:recipe").cloned().unwrap()
}

fn sq() -> Value {
    let p = [[0.0, 0.0], [6.0, 0.0], [6.0, 5.0], [0.0, 5.0]];
    json!([(0..4).map(|k| json!({ "line": { "start": p[k], "end": p[(k + 1) % 4] } })).collect::<Vec<_>>()])
}

#[test]
fn a_vertical_move_raises_walls_floors_roofs_and_stairs() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let l1 = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let l2 = ok(&mut app, "level.create", json!({ "elevation": 3.0, "name": "Level 2" }));
    // A wall up to level 2: its base and its top offset both rise; an unconnected one keeps its height.
    let wall = ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [0.0, 0.0], "end": [6.0, 0.0] } }, "base_level": l1, "top_constraint": { "level": l2 } }));
    let free = ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [0.0, 2.0], "end": [6.0, 2.0] } }, "base_level": l1, "top_constraint": { "unconnected": 2.5 } }));
    let floor = ok(&mut app, "floor.create", json!({ "loops": sq(), "level_id": l1 }));
    let roof = ok(&mut app, "roof.create_footprint", json!({ "curves": sq()[0], "level_id": l2, "slope": "30°" }));
    let stair = ok(&mut app, "stair.create", json!({ "base_level": l1, "railings": false, "runs": [{ "start": [10.0, 0.0], "end": [14.0, 0.0] }] }));
    ok(&mut app, "element.move", json!({ "element_ids": [wall, free, floor, roof, stair], "vector": [0.0, 0.0, 0.5] }));
    let (w, f) = (recipe(&app, &wall), recipe(&app, &free));
    assert!((w["base_offset"].as_f64().unwrap() - 0.5).abs() < 1e-9 && (w["top_offset"].as_f64().unwrap() - 0.5).abs() < 1e-9, "{w:#}");
    assert!((f["base_offset"].as_f64().unwrap() - 0.5).abs() < 1e-9 && f["top_offset"].as_f64().unwrap().abs() < 1e-9, "{f:#}");
    assert!((recipe(&app, &floor)["offset"].as_f64().unwrap() - 0.5).abs() < 1e-9);
    assert!((recipe(&app, &roof)["base_offset"].as_f64().unwrap() - 0.5).abs() < 1e-9);
    assert!((recipe(&app, &stair)["baseOffset"].as_f64().unwrap() - 0.5).abs() < 1e-9, "{:#}", recipe(&app, &stair));
    // One undo puts them all back.
    ok(&mut app, "edit.undo", json!({}));
    assert!(recipe(&app, &wall)["base_offset"].as_f64().unwrap().abs() < 1e-9);
}

#[test]
fn a_door_moved_off_its_wall_keeps_to_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let l1 = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let wall = ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [0.0, 0.0], "end": [10.0, 0.0] } }, "base_level": l1, "top_constraint": { "unconnected": 3.5 } }));
    let win = ok(&mut app, "window.place", json!({ "host_wall_id": wall, "point_along": 3.0 }));
    let before = recipe(&app, &win);
    // 1 m along, 2 m across (dropped: it stays in its wall) and 0.2 m up.
    ok(&mut app, "element.move", json!({ "element_ids": [win], "vector": [1.0, 2.0, 0.2] }));
    let after = recipe(&app, &win);
    assert!((after["along"].as_f64().unwrap() - before["along"].as_f64().unwrap() - 1.0).abs() < 1e-9, "{after:#}");
    assert!((after["sill"].as_f64().unwrap() - before["sill"].as_f64().unwrap() - 0.2).abs() < 1e-9);
    assert_eq!(after["host"], before["host"]);
}
