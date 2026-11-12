// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Rooftop PV arrays (MEP-ELEC-022): modules filling a roof round its equipment and glazing, rows
//! spaced clear of each other's winter shadow.

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

#[test]
fn modules_round_a_rooftop_unit_and_a_glazed_roof() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    ok(&mut app, "location.set", json!({ "lat": 25.8, "lon": -80.2, "time_zone": -5.0 }));
    let roof = ok(&mut app, "level.create", json!({ "elevation": 4.0, "name": "Roof" }));
    ok(&mut app, "floor.create", json!({ "loops": rect(0.0, 0.0, 30.0, 20.0), "level_id": roof }));
    // Bare: 4 ft from the edges, rows 1.70 + 0.30 / tan 40.8° apart.
    let bare = ok(&mut app, "pv.array", json!({ "level": roof }));
    let pitch = 1.722 * 10f64.to_radians().cos() + 1.722 * 10f64.to_radians().sin() / (90.0f64 - 25.8 - 23.44).to_radians().tan();
    assert!((bare["row_pitch"].as_f64().unwrap() - pitch).abs() < 1e-9, "{bare:#}");
    let n = bare["modules"].as_u64().unwrap();
    assert!(n > 150 && (bare["kwp"].as_f64().unwrap() - n as f64 * 0.4).abs() < 1e-9, "{bare:#}");
    assert!(bare["kept_clear_of"].as_array().unwrap().is_empty());
    ok(&mut app, "element.delete", json!({ "element_ids": [bare["array"]] }));
    // A rooftop unit and a glazed roof 3 m up over a third of the roof: fewer modules, clear of both.
    let t = ok(&mut app, "mep_type.load_starters", json!({ "names": ["Air Handling Unit"] }));
    ok(&mut app, "mech_equipment.place", json!({ "type_id": t["Air Handling Unit"], "point": [6.0, 10.0], "level": roof }));
    let glazing = ok(&mut app, "roof.create_glazing_type", json!({ "name": "Glazing" }));
    ok(&mut app, "roof.create_footprint", json!({ "type_id": glazing, "curves": rect(20.0, 0.0, 30.0, 20.0)[0], "level_id": roof, "base_offset": 3.0, "slope": "0°" }));
    let a = ok(&mut app, "pv.array", json!({ "level": roof }));
    let m = a["modules"].as_u64().unwrap();
    assert!(m > 40 && m < n * 2 / 3, "{a:#}");
    let kept: Vec<&str> = a["kept_clear_of"].as_array().unwrap().iter().map(|o| o["class"].as_str().unwrap()).collect();
    assert!(kept.contains(&"IfcRoof") && kept.len() == 2, "{kept:?}");
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    // Saved, the array is a solar device with its modules.
    let model = dir.path().join("pv.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let rows = riviera_ifc::IfcModel::open(&model).unwrap().step_rows().unwrap();
    assert!(rows.iter().any(|r| r.class == "IFCSOLARDEVICE" && r.attributes.contains(".SOLARPANEL.")));
}
