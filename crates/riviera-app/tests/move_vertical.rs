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

/// The scene's box of an element (min, max).
fn bounds(app: &App, id: &Value) -> ([f64; 3], [f64; 3]) {
    let s = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    let e = s.get(ElementId(id.as_u64().unwrap())).unwrap();
    let g = riviera_core::mesh_objects::regenerate(&s, e).unwrap();
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for (_, m) in g.slot_meshes() {
        for p in &m.0 {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    (lo, hi)
}

#[test]
fn components_and_masses_tilt_about_x_and_y_walls_only_turn() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let l1 = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // A desk, 60 × 30 in, standing at (3, 5): tipped 90° about x through its insertion point it
    // stands on its side, as tall as it was deep.
    let t = ok(&mut app, "furnishing.load", json!({ "names": ["Desk 60 × 30 in"] }))["Desk 60 × 30 in"].clone();
    let desk = ok(&mut app, "mesh_object.place", json!({ "type_id": t, "point": [3.0, 5.0], "level": l1 }));
    let (lo, hi) = bounds(&app, &desk);
    let depth = hi[1] - lo[1];
    ok(&mut app, "element.rotate_3d", json!({ "element_ids": [desk], "center": [3.0, 5.0, 0.0], "axis": "x", "angle": "90°" }));
    let (lo2, hi2) = bounds(&app, &desk);
    assert!(((hi2[2] - lo2[2]) - depth).abs() < 1e-6, "now {} tall, was {depth} deep", hi2[2] - lo2[2]);
    assert!(recipe(&app, &desk)["tilt"].is_array());
    // About z it turns in plan; the tilt turns with it (still on its side).
    ok(&mut app, "element.rotate_3d", json!({ "element_ids": [desk], "center": [3.0, 5.0, 0.0], "axis": "z", "angle": "90°" }));
    let (lo3, hi3) = bounds(&app, &desk);
    assert!(((hi3[2] - lo3[2]) - depth).abs() < 1e-6);
    // Saved and reopened, it keeps its tilt.
    let path = dir.path().join("tilt.ifc");
    ok(&mut app, "file.save_as", json!({ "path": path }));
    let d = app.active().unwrap();
    ok(&mut app, "file.close", json!({ "document_id": d }));
    ok(&mut app, "file.open", json!({ "path": path }));
    let (lo4, hi4) = bounds(&app, &desk);
    assert!(((hi4[2] - lo4[2]) - depth).abs() < 1e-6 && (lo4[0] - lo3[0]).abs() < 1e-6);
    // A wall turns in plan but refuses a tilt.
    let wall = ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [0.0, 0.0], "end": [6.0, 0.0] } }, "base_level": l1, "top_constraint": { "unconnected": 3.0 } }));
    ok(&mut app, "element.rotate_3d", json!({ "element_ids": [wall], "center": [0.0, 0.0, 0.0], "axis": "z", "angle": "90°" }));
    let e = app.execute("element.rotate_3d", json!({ "element_ids": [wall], "center": [0.0, 0.0, 0.0], "axis": "y", "angle": "30°" }), &Caller::user()).unwrap_err();
    assert_eq!(e.code, "element.cannot_tilt");
    // A mass tips over about y: its 25 m tall block lies 25 m long in x.
    let m = ok(&mut app, "mass.create", json!({ "name": "Block", "forms": [{ "kind": "extrusion", "start": 0.0, "end": 25.0, "level": l1, "profile": [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]] }] }));
    ok(&mut app, "element.rotate_3d", json!({ "element_ids": [m], "center": [0.0, 0.0, 0.0], "axis": "y", "angle": "90°" }));
    let r = recipe(&app, &m);
    let n = &r["forms"][0]["plane"]["normal"];
    assert!((n[0].as_f64().unwrap().abs() - 1.0).abs() < 1e-9 && n[2].as_f64().unwrap().abs() < 1e-9, "{r:#}");
}
