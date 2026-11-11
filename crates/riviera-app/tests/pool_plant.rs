// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A pool's treatment plant as equipment (ARC-POOL-014): the balance tank, pumps, filters and
//! chemical tanks sized from `pool.treatment`, set out in the plant room, and saved as IFC.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

/// A room `w` × `d` from (x, y), walled 0.2 m thick round it.
fn room(app: &mut App, level: &Value, x: f64, y: f64, w: f64, d: f64) -> Value {
    let t = 0.1;
    let c = [[x - t, y - t], [x + w + t, y - t], [x + w + t, y + d + t], [x - t, y + d + t]];
    for k in 0..4 {
        ok(app, "wall.create", json!({ "curve": { "line": { "start": c[k], "end": c[(k + 1) % 4] } }, "base_level": level, "top_constraint": { "unconnected": 3.0 } }));
    }
    ok(app, "room.place", json!({ "level_id": level, "point": [x + w / 2.0, y + d / 2.0], "name": "Plant" }))
}

fn classes(app: &App) -> [usize; 3] {
    let s = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    ["IfcTank", "IfcPump", "IfcFilter"].map(|c| s.iter().filter(|e| e.class == c).count())
}

#[test]
fn the_plant_in_its_room() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "competition_25m", "origin": [0.0, 0.0] } }));
    let t = ok(&mut app, "pool.treatment", json!({ "pool_id": pool }))[0].clone();
    // A 20 × 5 m plant room: 100 m², inside the 15–30 % of the water.
    let r = room(&mut app, &level, 0.0, -20.0, 20.0, 5.0);
    let p = ok(&mut app, "pool.plant", json!({ "room": r }));
    assert!((f(&p["flow"]) - f(&t["flow"])).abs() < 1e-9 && f(&p["head"]) == 15.0, "{p:#}");
    let power = 9.81 * f(&t["flow"]) / 3600.0 * 15.0 / 0.6;
    assert!((f(&p["pump_power"]) - power).abs() < 1e-9 && f(&p["motor"]) >= power, "{p:#}");
    let d = (4.0 * f(&t["filter_area_each"]) / std::f64::consts::PI).sqrt();
    assert!(f(&p["filter_diameter"]) >= d && f(&p["filter_diameter"]) < d + 0.05, "{p:#}");
    let roles: Vec<&str> = p["items"].as_array().unwrap().iter().map(|i| i["role"].as_str().unwrap()).collect();
    assert_eq!(roles, vec!["balance_tank", "pump", "pump", "filter", "filter", "hypochlorite", "acid"], "{p:#}");
    assert_eq!(classes(&app), [3, 2, 2]);
    // Along the room's south wall, 0.5 m off it; the chemicals 2 m apart.
    let items = p["items"].as_array().unwrap();
    assert!(items.iter().all(|i| f(&i["at"][1]) > -20.0 + 0.5 && f(&i["at"][1]) < -15.0 && f(&i["at"][0]) > 0.6), "{p:#}");
    let (h, a) = (&items[5], &items[6]);
    assert!(((f(&a["at"][0]) - f(&h["at"][0])) - (0.8 + 2.0)).abs() < 1e-9, "{p:#}");
    assert!((f(&p["room_area"]) - 100.0).abs() < 1e-6 && !p["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("plant room is") || n.as_str().unwrap().contains("needs")), "{p:#}");
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    // A cramped room: too small for the range, and the row does not fit. The plant is replaced.
    let small = room(&mut app, &level, 30.0, -20.0, 6.0, 3.0);
    let p = ok(&mut app, "pool.plant", json!({ "room": small }));
    let notes: Vec<&str> = p["notes"].as_array().unwrap().iter().map(|n| n.as_str().unwrap()).collect();
    assert!(notes.iter().any(|n| n.starts_with("the plant room is 18.0 m²")) && notes.iter().any(|n| n.contains("the room is 6.0 × 3.0 m")), "{notes:#?}");
    assert_eq!(classes(&app), [3, 2, 2]);
    // On a level from a point, turned to run north; the UK basis sizes it differently.
    let p = ok(&mut app, "pool.plant", json!({ "level": level, "point": [40.0, 0.0], "rotation": std::f64::consts::FRAC_PI_2, "basis": "uk" }));
    let items = p["items"].as_array().unwrap();
    assert!(items.iter().all(|i| f(&i["at"][0]) < 40.0 && f(&i["at"][1]) > 0.0) && p["basis"] == "uk", "{p:#}");
    assert_eq!(classes(&app), [3, 2, 2]);
    // Saved, they are tanks, pumps and water filters.
    let model = dir.path().join("plant.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let rows = riviera_ifc::IfcModel::open(&model).unwrap().step_rows().unwrap();
    assert_eq!(rows.iter().filter(|r| r.class == "IFCTANK").count(), 3);
    assert!(rows.iter().any(|r| r.class == "IFCFILTERTYPE" && r.attributes.contains(".WATERFILTER.")));
    assert!(rows.iter().any(|r| r.class == "IFCTANKTYPE" && r.attributes.contains(".STORAGE.")));
}
