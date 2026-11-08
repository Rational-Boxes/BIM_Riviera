// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Gondola runs (ARC-RETAIL-001): sections filled to the run with end caps on islands, and the
//! aisles between runs checked by the rule packs: ADA §403.5's 36 in route and 48 in turn, the IBC
//! Group M 44 in aisle, and the practice 6 ft cart aisle as a note.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

const IN: f64 = 0.0254;

fn rules(found: &Value) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = found.as_array().unwrap().iter().map(|f| (f["rule"].as_str().unwrap().to_owned(), f["severity"].as_str().unwrap().to_owned())).collect();
    v.sort();
    v.dedup();
    v
}

#[test]
fn gondola_runs_keep_their_aisles() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "clearance.set_rule_set", json!({ "standard": "ada_2010" }));
    // Under the IBC the mercantile pack is in use without being chosen.
    let packs = ok(&mut app, "rules.packs", json!({ "domain": "retail" }));
    let in_use: Vec<&str> = packs.as_array().unwrap().iter().filter(|p| p["in_use"] == true).map(|p| p["id"].as_str().unwrap()).collect();
    assert_eq!(in_use, vec!["us_ibc_mercantile"], "{packs:#}");

    // An island 229 in long: two 18.5 in end caps and four 48 in sections, 35¾ in deep.
    let len = 229.0 * IN;
    let a = ok(&mut app, "retail.run", json!({ "start": [0.0, 0.0], "end": [len, 0.0], "level": level, "kind": "island" }));
    let widths: Vec<f64> = a["sections"].as_array().unwrap().iter().filter(|s| s["end_cap"] == false).map(|s| s["width"].as_f64().unwrap() / IN).collect();
    assert_eq!(widths.len(), 4, "{a:#}");
    assert!(widths.iter().all(|w| (w - 48.0).abs() < 1e-6) && a["left_over"].as_f64().unwrap().abs() < 1e-6, "{a:#}");
    assert!((a["depth"].as_f64().unwrap() / IN - 35.75).abs() < 1e-6 && (a["overall"].as_f64().unwrap() - len).abs() < 1e-6);
    assert_eq!(a["sections"].as_array().unwrap().iter().filter(|s| s["end_cap"] == true).count(), 2);
    assert_eq!(ok(&mut app, "rules.check", json!({})), json!([]), "alone on the floor");

    // A second island 40 in from the first's faces: the ADA route is met, the 44 in Group M aisle
    // is not.
    let sep = 34.5 * IN + 40.0 * IN;
    let b = ok(&mut app, "retail.run", json!({ "start": [0.0, sep], "end": [len, sep], "level": level, "kind": "island" }));
    assert_eq!(rules(&ok(&mut app, "rules.check", json!({}))), vec![("IBC 1018.3".to_owned(), "error".to_owned())]);
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    // With the practice pack too, the 6 ft cart aisle is a note.
    ok(&mut app, "rules.set_packs", json!({ "domain": "retail", "packs": ["us_ibc_mercantile", "practice_retail"] }));
    assert_eq!(rules(&ok(&mut app, "rules.check", json!({}))), vec![("Cart aisle".to_owned(), "note".to_owned()), ("IBC 1018.3".to_owned(), "error".to_owned())]);
    ok(&mut app, "rules.set_packs", json!({ "domain": "retail", "packs": [] }));
    // At 30 in the ADA route fails too.
    let ids: Vec<Value> = b["sections"].as_array().unwrap().iter().map(|s| s["element"].clone()).collect();
    ok(&mut app, "element.move", json!({ "element_ids": ids, "vector": [0.0, -10.0 * IN, 0.0] }));
    assert_eq!(rules(&ok(&mut app, "rules.check", json!({}))), vec![("ADA 2010 403.5.1".to_owned(), "error".to_owned())]);
    ok(&mut app, "element.move", json!({ "element_ids": ids, "vector": [0.0, 10.0 * IN, 0.0] }));

    // A wall 40 in past the end caps (their fronts at x = 0) leaves no room to turn (48 in,
    // §403.5.2).
    let x = -40.0 * IN - 0.1;
    ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [x, -2.0], "end": [x, 4.0] } }, "base_level": level, "top_constraint": { "unconnected": 2.7 } }));
    assert_eq!(rules(&ok(&mut app, "rules.check", json!({}))), vec![("ADA 2010 403.5.2".to_owned(), "error".to_owned())]);

    // A wall run on a wall's face faces away from it: 84 in, WallGondola, one face.
    ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [0.0, 6.1], "end": [6.0, 6.1] } }, "base_level": level, "top_constraint": { "unconnected": 2.7 } }));
    let w = ok(&mut app, "retail.run", json!({ "start": [0.5, 6.0], "end": [0.5 + 144.0 * IN, 6.0], "level": level, "kind": "wall" }));
    assert_eq!(w["sections"].as_array().unwrap().len(), 3, "{w:#}");
    let s = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    let it = riviera_core::rules::item(&s, s.get(riviera_core::ElementId(w["sections"][0]["element"].as_u64().unwrap())).unwrap()).unwrap();
    assert!((it.z[1] / IN - 84.0).abs() < 1e-6, "{it:?}");
    // Its back on the wall face, its front 18.5 in toward the room.
    assert!((it.frame.apply([0.0, it.back, 0.0])[1] - 6.0).abs() < 1e-6 && (it.frame.apply([0.0, it.front, 0.0])[1] - (6.0 - 18.5 * IN)).abs() < 1e-6, "{it:?}");
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    let runs = ok(&mut app, "retail.runs", json!({}));
    assert_eq!(runs.as_array().unwrap().iter().map(|r| r["fixtures"].as_array().unwrap().len()).collect::<Vec<_>>(), vec![6, 6, 3]);

    let e = app.execute("retail.run", json!({ "start": [0.0, 9.0], "end": [3.0, 9.0], "level": level, "kind": "island", "section": 40.0 * IN }), &Caller::user()).unwrap_err();
    assert_eq!(e.code, "retail.bad_section");
}
