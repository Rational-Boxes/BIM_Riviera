// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A competition pool's lane fittings (ARC-POOL-017): lane ropes, backstroke flags and the
//! false-start rope, checked against World Aquatics, and their IFC.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

/// The World Aquatics findings on the fittings: (severity, message).
fn fittings(app: &mut App) -> Vec<(String, String)> {
    let c = ok(app, "pool.check", json!({}));
    c["findings"].as_array().unwrap().iter().filter(|x| ["FR 2.6", "FR 2.9", "FR 2.11"].iter().any(|r| x["rule"].as_str().unwrap().starts_with(r))).map(|x| (x["severity"].as_str().unwrap().to_string(), x["message"].as_str().unwrap().to_string())).collect()
}

#[test]
fn ropes_flags_and_the_false_start_rope() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // A 25 m competition pool: six lanes, 2.0 m deep, deeper toward its far end.
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "competition_25m", "origin": [12.5, 7.5] }, "profile": [{ "at": 0.0, "depth": 1.8 }, { "at": 25.03, "depth": 2.0 }] }));
    // Without fittings, three notes.
    let f = fittings(&mut app);
    assert!(f.len() == 3 && f.iter().all(|(s, _)| s == "note"), "{f:#?}");
    // With them: five ropes (the 0.1 m side gaps are too narrow for a 0.15 m float), flags 1.8 m up and 5 m
    // out, the false-start rope 15 m from the deep (far) end.
    let lay = ok(&mut app, "pool.lanes", json!({ "pool_id": pool, "count": 6, "fittings": true }));
    let fit = &lay["fittings"];
    assert_eq!(fit["ropes"].as_array().unwrap().len(), 5, "{lay:#}");
    assert!(fit["flag_height"] == 1.8 && fit["false_start_distance"] == 15.0 && fit["start"] == "deep", "{fit:#}");
    assert_eq!(fittings(&mut app), Vec::<(String, String)>::new());
    // Wrong: 200 mm floats, flags 2.2 m up and 4.5 m out, the false-start rope 1.0 m up 12 m out.
    ok(&mut app, "pool.lanes", json!({ "pool_id": pool, "count": 6, "float": 0.2, "flag_height": 2.2, "flag_distance": 4.5, "false_start_height": 1.0, "false_start_distance": 12.0 }));
    let f = fittings(&mut app);
    for want in [
        "the lane ropes' floats are 200 mm across; 100–150 mm",
        "the backstroke flags' rope is 2.20 m above the water; 1.80 m",
        "the backstroke flags are 4.50 m out from each end wall; 5.00 m",
        "the false-start rope is 1.00 m above the water; at least 1.20 m",
        "the false-start rope is 12.00 m out from the start end (the deep end); 15.00 m",
    ] {
        assert!(f.iter().any(|(s, m)| s == "error" && m.starts_with(want)), "{want}\n{f:#?}");
    }
    // Flags that do not fit are refused.
    assert!(app.execute("pool.lanes", json!({ "pool_id": pool, "count": 6, "flag_distance": 13.0 }), &Caller::user()).is_err());
    ok(&mut app, "pool.lanes", json!({ "pool_id": pool, "count": 6, "float": 0.15, "flag_height": 1.8, "flag_distance": 5.0, "false_start_height": 1.2, "false_start_distance": 15.0 }));
    // Nothing overlaps; saved, the fittings are discrete accessories in the pool's assembly.
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    let model = dir.path().join("pool.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let rows = riviera_ifc::IfcModel::open(&model).unwrap().step_rows().unwrap();
    for t in ["'LANEROPE'", "'BACKSTROKEFLAGS'", "'FALSESTARTROPE'"] {
        assert!(rows.iter().any(|r| r.class == "IFCDISCRETEACCESSORY" && r.attributes.contains(t)), "{t}");
    }
    // Reopened, the pool keeps them; taken away, they are notes again.
    let mut again = App::new(AppConfig { live: None, untitled_dir: dir.path().join("v") });
    ok(&mut again, "file.open", json!({ "path": model }));
    let p = ok(&mut again, "pool.list", json!({}))[0]["id"].clone();
    assert_eq!(ok(&mut again, "pool.info", json!({ "pool_id": p }))["lanes"]["fittings"]["ropes"].as_array().unwrap().len(), 5);
    ok(&mut app, "pool.lanes", json!({ "pool_id": pool, "count": 6, "fittings": false }));
    assert!(fittings(&mut app).iter().all(|(s, _)| s == "note"));
}
