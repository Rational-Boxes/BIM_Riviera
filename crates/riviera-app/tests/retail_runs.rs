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

#[test]
fn store_checks_count_occupants_and_check_storage() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // A 12 × 8 m sales floor and a 4 × 8 m stock room beside it (inside faces).
    let wall = |app: &mut App, a: [f64; 2], b: [f64; 2]| {
        ok(app, "wall.create", json!({ "curve": { "line": { "start": a, "end": b } }, "base_level": level, "top_constraint": { "unconnected": 6.0 } }));
    };
    wall(&mut app, [-0.1, -0.1], [16.3, -0.1]);
    wall(&mut app, [16.3, -0.1], [16.3, 8.1]);
    wall(&mut app, [16.3, 8.1], [-0.1, 8.1]);
    wall(&mut app, [-0.1, 8.1], [-0.1, -0.1]);
    wall(&mut app, [12.1, 0.0], [12.1, 8.0]);
    let sales = ok(&mut app, "room.place", json!({ "level_id": level, "point": [6.0, 4.0], "name": "Sales" }));
    let stock = ok(&mut app, "room.place", json!({ "level_id": level, "point": [14.0, 4.0], "name": "Stock" }));
    ok(&mut app, "retail.set_area", json!({ "room_ids": [sales], "area": "sales" }));
    ok(&mut app, "retail.set_area", json!({ "room_ids": [stock], "area": "stock" }));
    // 96 m² at 60 ft² (5.57 m²) a person is 18; 32 m² at 300 ft² is 2.
    let c = ok(&mut app, "retail.check", json!({}));
    let loads: Vec<(String, u64)> = c["areas"].as_array().unwrap().iter().map(|a| (a["area_use"].as_str().unwrap().to_owned(), a["occupants"].as_u64().unwrap())).collect();
    assert_eq!(loads, vec![("sales".into(), 18), ("stock".into(), 2)], "{c:#}");
    assert_eq!((c["occupants"].as_u64(), c["levels"].as_array().unwrap().len()), (Some(20), 1));
    assert_eq!(c["findings"], json!([]));
    // At 30 ft² a person (some jurisdictions at grade), 35 on the sales floor.
    assert_eq!(ok(&mut app, "retail.check", json!({ "sales_factor": 30.0 * 0.092_903_04 }))["areas"][0]["occupants"], 35);

    // Upright heads 3 m up over the sales floor; a 72 in island under one keeps its 18 in.
    let t = ok(&mut app, "mep_type.load_starters", json!({ "names": ["Upright Sprinkler"] }))["Upright Sprinkler"].clone();
    for x in [3.0, 9.0] {
        ok(&mut app, "sprinkler.place", json!({ "type_id": t, "point": [x, 4.0], "level": level }));
    }
    let island = ok(&mut app, "retail.run", json!({ "start": [1.5, 4.0], "end": [4.5, 4.0], "level": level, "kind": "island" }));
    assert_eq!(ok(&mut app, "retail.check", json!({}))["findings"], json!([]));
    // A 10 ft island under the other head is too close to it, and over 96 in it is anchored.
    let tall = ok(&mut app, "retail.run", json!({ "start": [7.5, 4.0], "end": [10.5, 4.0], "level": level, "kind": "island", "height": 120.0 * IN }));
    let f = ok(&mut app, "retail.check", json!({}));
    let kinds: Vec<(&str, &str)> = f["findings"].as_array().unwrap().iter().map(|f| (f["rule"].as_str().unwrap(), f["severity"].as_str().unwrap())).collect();
    assert!(kinds.contains(&("NFPA 13 clearance", "error")) && kinds.contains(&("Madix anchorage", "note")), "{f:#}");
    assert_eq!(kinds[0].1, "error", "errors first");
    let n_tall = tall["sections"].as_array().unwrap().len();
    assert_eq!(kinds.iter().filter(|k| k.0 == "NFPA 13 clearance").count(), n_tall, "{f:#}");
    // An 84 in wall run on 16 in decks is over Madix's 78 in unanchored height.
    let w = ok(&mut app, "retail.run", json!({ "start": [0.5, 7.0], "end": [0.5 + 48.0 * IN, 7.0], "level": level, "kind": "wall", "toward": [0.5, 0.0] }));
    let id = w["sections"][0]["element"].clone();
    let f = ok(&mut app, "retail.check", json!({}));
    let mine: Vec<&Value> = f["findings"].as_array().unwrap().iter().filter(|x| x["element"] == id).collect();
    assert!(mine.len() == 1 && mine[0]["rule"] == "Madix anchorage" && (mine[0]["limit"].as_f64().unwrap() / IN - 78.0).abs() < 1e-3, "{f:#}");
    // A 16 ft pallet rack on the sales floor, away from the heads: high-piled, and anchored.
    let rack = ok(&mut app, "furnishing.load", json!({ "names": ["Pallet rack bay 96 × 42 in"] }))["Pallet rack bay 96 × 42 in"].clone();
    let r = ok(&mut app, "mesh_object.place", json!({ "type_id": rack, "point": [6.0, 1.0], "level": level }));
    let f = ok(&mut app, "retail.check", json!({}));
    let mut mine: Vec<&str> = f["findings"].as_array().unwrap().iter().filter(|x| x["element"] == r).map(|x| x["rule"].as_str().unwrap()).collect();
    mine.sort();
    assert_eq!(mine, vec!["FEMA 460 anchorage", "IFC 3201 high-piled"], "{f:#}");
    // In the stock room the rack is not in a public area.
    ok(&mut app, "element.move", json!({ "element_ids": [r], "vector": [8.0, 0.0, 0.0] }));
    let f = ok(&mut app, "retail.check", json!({}));
    let mine: Vec<&str> = f["findings"].as_array().unwrap().iter().filter(|x| x["element"] == r).map(|x| x["rule"].as_str().unwrap()).collect();
    assert_eq!(mine, vec!["IFC 3201 high-piled"], "{f:#}");
    let _ = island;
}
