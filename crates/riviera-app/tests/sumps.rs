// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Basement sumps and groundwater drainage (MEP-PIPE-011): the perimeter drain, the pit sized for
//! the pumps' starts, duplex pumps and the rising main.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let p = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    json!([(0..4).map(|k| json!({ "line": { "start": p[k], "end": p[(k + 1) % 4] } })).collect::<Vec<_>>()])
}

/// A 30 × 20 m basement 4 m down: its slab to the walls' inner faces, walls on strip footings
/// 0.3 m below it, and the ground floor's slab over it.
fn basement() -> (tempfile::TempDir, App, Value) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let l1 = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let b1 = ok(&mut app, "level.create", json!({ "elevation": -4.0, "name": "Basement" }));
    ok(&mut app, "floor.create", json!({ "loops": rect(0.1, 0.1, 29.9, 19.9), "level_id": b1 }));
    ok(&mut app, "floor.create", json!({ "loops": rect(0.0, 0.0, 30.0, 20.0), "level_id": l1 }));
    let p = [[0.0, 0.0], [30.0, 0.0], [30.0, 20.0], [0.0, 20.0]];
    let walls: Vec<Value> = (0..4).map(|k| ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": p[k], "end": p[(k + 1) % 4] } }, "base_level": b1, "base_offset": -0.3, "top_constraint": { "unconnected": 4.0 } }))).collect();
    ok(&mut app, "wall_foundation.create", json!({ "wall_ids": walls }));
    (dir, app, b1)
}

#[test]
fn a_sump_for_a_basement() {
    let (dir, mut app, b1) = basement();
    let s = ok(&mut app, "sump.create", json!({ "level": b1, "point": [5.0, 5.0] }));
    // The drain 0.6 m outside the floor's edge: 31 × 21 m, 104 m; 1.04 L/s at 0.01 L/s a metre.
    let f = |k: &str| s[k].as_f64().unwrap_or_else(|| panic!("{k}: {s:#}"));
    assert!((f("drain_length") - 104.0).abs() < 1e-6 && (f("inflow") - 1.04).abs() < 1e-9 && (f("design_flow") - 1.56).abs() < 1e-9, "{s:#}");
    // The pit: Q·T/4 at ten starts an hour, at least 1 m square; 1.2 + 3.0 + 3 m of head.
    assert!((f("working_volume") - 0.00156 * 360.0 / 4.0).abs() < 1e-9 && (f("pit_size") - 1.0).abs() < 1e-9 && (f("head") - 7.2).abs() < 1e-9, "{s:#}");
    assert!((f("motor") - 0.37).abs() < 1e-9 && s["pumps"].as_array().unwrap().len() == 2, "{s:#}");
    assert!(s["notes"].as_array().unwrap().len() == 2);
    // The drain's ring, its branch into the pit and the rising main are storm drainage pipes.
    assert!(s["pipes"].as_array().unwrap().len() >= 8, "{s:#}");
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    // A larger inflow: bigger pit and motor.
    let (_d2, mut big, b) = basement();
    let s2 = ok(&mut big, "sump.create", json!({ "level": b, "point": [5.0, 5.0], "inflow": 8.0 }));
    assert!(s2["pit_size"].as_f64().unwrap() > 1.0 && s2["motor"].as_f64().unwrap() > 0.37, "{s2:#}");
    // Outside the floor: refused.
    assert!(app.execute("sump.create", json!({ "level": b1, "point": [50.0, 5.0] }), &Caller::user()).is_err());
    // Saved: the pit is a SUMP chamber, the pumps submersible.
    let model = dir.path().join("sump.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let rows = riviera_ifc::IfcModel::open(&model).unwrap().step_rows().unwrap();
    assert!(rows.iter().any(|r| r.class == "IFCDISTRIBUTIONCHAMBERELEMENTTYPE" && r.attributes.contains(".SUMP.")));
    assert!(rows.iter().any(|r| r.class == "IFCPUMPTYPE" && r.attributes.contains(".SUBMERSIBLEPUMP.")));
}
