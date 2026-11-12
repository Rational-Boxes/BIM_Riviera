// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! The discipline selector (DOC-GFX-021): a view shown from one discipline's point of view, the
//! others ghosted or hidden, as temporary visibility.

use riviera_app::{App, AppConfig};
use riviera_core::{Caller, ElementId};
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn id(v: &Value) -> ElementId {
    serde_json::from_value(v.clone()).unwrap()
}

#[test]
fn each_discipline_in_turn() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let l1 = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let wall = ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [0.0, 0.0], "end": [6.0, 0.0] } }, "base_level": l1, "top_constraint": { "unconnected": 3.0 } }));
    let column = ok(&mut app, "column.place", json!({ "point": [3.0, 3.0], "base_level": l1, "top_constraint": { "unconnected": 3.0 } }));
    let duct = ok(&mut app, "duct.create", json!({ "points": [[0.0, 2.0, 2.7], [6.0, 2.0, 2.7]], "level": l1, "shape": "round", "diameter": 0.3, "system_type_id": "supply_air" }))["segments"][0].clone();
    let pipe = ok(&mut app, "pipe.create", json!({ "points": [[0.0, 4.0, 2.5], [6.0, 4.0, 2.5]], "level": l1, "diameter": 0.05, "system_type_id": "domestic_cold_water" }))["segments"][0].clone();
    let ground = ok(&mut app, "toposolid.create", json!({ "level_id": l1, "boundary": [[-10.0, -10.0], [20.0, -10.0], [20.0, -2.0], [-10.0, -2.0]], "points": [[-10.0, -10.0, -0.5], [20.0, -2.0, -0.3]] }));
    let doc = app.active().unwrap();
    // Structure: the column shown, the wall, duct, pipe and ground ghosted.
    let r = ok(&mut app, "view.discipline", json!({ "disciplines": ["structure"] }));
    assert_eq!(r["others"], 4, "{r:#}");
    for k in ["Architecture", "Structure", "Mechanical", "Plumbing", "Site"] {
        assert_eq!(r["by_discipline"][k], 1, "{k}: {r:#}");
    }
    let t = app.temp_state(doc, None);
    let iso = t.isolated.as_ref().expect("isolated");
    assert!(iso.elements.contains(&id(&column)) && !iso.elements.contains(&id(&wall)) && iso.ghost_others.is_some());
    // Mechanical and plumbing together, the rest hidden.
    ok(&mut app, "view.discipline", json!({ "disciplines": ["mechanical", "plumbing"], "others": "hidden" }));
    let t = app.temp_state(doc, None);
    let iso = t.isolated.as_ref().unwrap();
    assert!(iso.elements.contains(&id(&duct)) && iso.elements.contains(&id(&pipe)) && !iso.elements.contains(&id(&ground)) && iso.ghost_others.is_none());
    // Site.
    ok(&mut app, "view.discipline", json!({ "disciplines": ["site"] }));
    assert!(app.temp_state(doc, None).isolated.unwrap().elements.contains(&id(&ground)));
    // None: everything again.
    let r = ok(&mut app, "view.discipline", json!({}));
    assert_eq!(r["others"], 0);
    assert!(app.temp_state(doc, None).is_empty());
}
