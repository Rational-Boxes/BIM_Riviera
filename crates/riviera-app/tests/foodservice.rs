// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Commercial kitchens: equipment line-ups under their hoods (ARC-KIT-003) and the kitchen check
//! (ARC-KIT-004).

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn rules(c: &Value) -> Vec<String> {
    let mut v: Vec<String> = c["findings"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_owned()).collect();
    v.sort();
    v.dedup();
    v
}

const IN: f64 = 0.0254;

/// An element's plan bounds: (min, max).
fn bounds(app: &App, id: &Value) -> ([f64; 2], [f64; 2]) {
    let s = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    let it = riviera_core::rules::item(&s, s.get(riviera_core::ElementId(id.as_u64().unwrap())).unwrap()).unwrap();
    let q: Vec<[f64; 3]> = [[it.x[0], it.front], [it.x[1], it.front], [it.x[1], it.back], [it.x[0], it.back]].iter().map(|c| it.frame.apply([c[0], c[1], 0.0])).collect();
    let lo = [q.iter().map(|c| c[0]).fold(f64::MAX, f64::min), q.iter().map(|c| c[1]).fold(f64::MAX, f64::min)];
    let hi = [q.iter().map(|c| c[0]).fold(f64::MIN, f64::max), q.iter().map(|c| c[1]).fold(f64::MIN, f64::max)];
    (lo, hi)
}

#[test]
fn a_cook_line_gets_its_hood_baffle_and_checks() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // A kitchen 8 × 6 m; its north wall's inner face at y = 6 − half its width.
    let wall = ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [0.0, 6.0], "end": [8.0, 6.0] } }, "base_level": level, "top_constraint": { "unconnected": 3.5 } }));
    let s = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    let width = riviera_core::walls::regenerate(&s, s.get(riviera_core::ElementId(wall.as_u64().unwrap())).unwrap()).unwrap().width;
    let face = 6.0 - width / 2.0;

    // Too long for the line: nothing placed.
    let e = app.execute("foodservice.lineup", json!({ "start": [0.5, face], "end": [1.5, face], "level": level, "items": ["Range, 6-burner with oven 36 in", "Fryer, 40 lb gas"] }), &Caller::user()).unwrap_err();
    assert_eq!(e.code, "foodservice.too_long");

    // A cook line: a fryer beside a range (open flames) gets a baffle; the charbroiler makes it
    // heavy duty, so the Type I hood is rated at 400 cfm a foot.
    let line = ok(&mut app, "foodservice.lineup", json!({ "start": [0.5, face], "end": [7.5, face], "level": level, "items": ["Fryer, 40 lb gas", "Range, 6-burner with oven 36 in", "Charbroiler 36 in, on stand", "Convection oven, double deck"] }));
    assert_eq!(line["placed"].as_array().unwrap().len(), 4, "{line:#}");
    assert_eq!(line["baffles"].as_array().unwrap().len(), 1, "{line:#}");
    let hood = &line["hoods"][0];
    assert_eq!((hood["hood_type"].as_str(), hood["duty"].as_str()), (Some("I"), Some("heavy")));
    let used = line["used"].as_f64().unwrap();
    let w = (15.1 + 36.0 + 36.0 + 38.0) * IN + 0.006;
    assert!((used - w).abs() < 1e-6, "{used} vs {w}");
    assert!((hood["length"].as_f64().unwrap() - (w + 12.0 * IN)).abs() < 1e-6);
    assert!((hood["exhaust_cfm"].as_f64().unwrap() - (400.0 * (w + 12.0 * IN) / 0.3048).round()).abs() < 1e-6);
    assert_eq!(hood["makeup_cfm"], hood["exhaust_cfm"]);
    // The pieces stand back to the wall's face, in front of it (y below the face).
    for p in line["placed"].as_array().unwrap() {
        let (lo, hi) = bounds(&app, &p["element"]);
        assert!((hi[1] - face).abs() < 1e-3 && lo[1] < face - 0.3, "{p}: {lo:?} {hi:?}");
    }
    // Side by side, nothing overlapping: the wall, the pieces, the baffle and the hood.
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));

    // No hand sink, mop sink or extinguisher yet.
    let c = ok(&mut app, "foodservice.check", json!({}));
    assert_eq!(rules(&c), vec!["Class K extinguisher", "Hand sinks", "Mop sink"], "{c:#}");
    assert!(c["exhaust_cfm"].as_f64().unwrap() > 0.0);

    // The south wall: a hand sink, a mop sink and an extinguisher; then it passes.
    ok(&mut app, "foodservice.lineup", json!({ "start": [0.5, 0.0], "end": [4.0, 0.0], "level": level, "toward": [2.0, 3.0], "items": ["Hand sink, wall-hung", "Mop sink 24 × 24 in", "Class K extinguisher"], "gap": 0.3 }));
    let c = ok(&mut app, "foodservice.check", json!({}));
    assert_eq!(rules(&c), Vec::<String>::new(), "{c:#}");

    // A work table 0.5 m in front of the range: in its aisle.
    let t = ok(&mut app, "furnishing.load", json!({ "names": ["Work table 30 × 72 in"] }));
    let table = ok(&mut app, "mesh_object.place", json!({ "type_id": t["Work table 30 × 72 in"], "point": [1.5, face - 0.86 - 0.5 - 0.381], "level": level }));
    assert_eq!(rules(&ok(&mut app, "foodservice.check", json!({}))), vec!["Aisle"]);
    ok(&mut app, "element.delete", json!({ "element_ids": [table] }));

    // Without the baffle the fryer is too close to the range's flames.
    ok(&mut app, "element.delete", json!({ "element_ids": [line["baffles"][0].clone()] }));
    assert_eq!(rules(&ok(&mut app, "foodservice.check", json!({}))), vec!["Fryer separation"]);
}

#[test]
fn a_dish_machine_gets_a_type_ii_hood_and_wants_a_floor_sink() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let line = ok(&mut app, "foodservice.lineup", json!({ "start": [0.0, 0.0], "end": [6.0, 0.0], "level": level, "toward": [0.0, -1.0], "items": ["Dish table, soiled 60 in", "Dish machine, door type", "Dish table, clean 60 in", "Hand sink, wall-hung"], "gap": 0.0 }));
    assert_eq!(line["hoods"].as_array().unwrap().len(), 1);
    assert_eq!((line["hoods"][0]["hood_type"].as_str(), line["hoods"][0]["duty"].clone()), (Some("II"), Value::Null));
    let c = ok(&mut app, "foodservice.check", json!({}));
    assert_eq!(rules(&c), vec!["Floor sink", "Mop sink"], "{c:#}");
    // A floor sink in front of the machine.
    let machine = line["placed"][1]["element"].clone();
    let (lo, hi) = bounds(&app, &machine);
    let x = (lo[0] + hi[0]) / 2.0;
    let t = ok(&mut app, "furnishing.load", json!({ "names": ["Floor sink 12 × 12 in"] }));
    ok(&mut app, "mesh_object.place", json!({ "type_id": t["Floor sink 12 × 12 in"], "point": [x, -1.0], "level": level }));
    assert_eq!(rules(&ok(&mut app, "foodservice.check", json!({}))), vec!["Mop sink"]);
}
