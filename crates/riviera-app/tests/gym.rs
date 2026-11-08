// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Gyms (ARC-GYM-001): the equipment's use zones from the `gym_equipment` pack, ADA §236.1 by
//! type, the ceiling over each piece, and mirrors.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn rules(found: &Value) -> Vec<String> {
    let mut v: Vec<String> = found.as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_owned()).collect();
    v.sort();
    v.dedup();
    v
}

/// A rectangle's four sides as line curves.
fn square(a: [f64; 2], b: [f64; 2]) -> Value {
    let p = [[a[0], a[1]], [b[0], a[1]], [b[0], b[1]], [a[0], b[1]]];
    Value::Array((0..4).map(|k| json!({ "line": { "start": p[k], "end": p[(k + 1) % 4] } })).collect())
}

#[test]
fn a_gym_is_checked() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "clearance.set_rule_set", json!({ "standard": "ada_2010" }));
    let t = ok(&mut app, "furnishing.load", json!({ "names": ["Treadmill, commercial", "Flat bench", "Power rack"] }));

    // Two treadmills 2 m apart, stepped on from the south: their zones clear.
    let a = ok(&mut app, "mesh_object.place", json!({ "type_id": t["Treadmill, commercial"], "point": [0.0, 0.0], "level": level }));
    let b = ok(&mut app, "mesh_object.place", json!({ "type_id": t["Treadmill, commercial"], "point": [2.0, 0.0], "level": level }));
    assert_eq!(ok(&mut app, "rules.check", json!({})), json!([]));
    let c = ok(&mut app, "gym.check", json!({}));
    assert_eq!((c["types"][0]["kind"].as_str(), c["types"][0]["with_space"].as_u64(), c["findings"].clone()), (Some("Treadmill"), Some(2), json!([])), "{c:#}");

    // A bench 1.5 m behind the first treadmill is in its rear zone; 0.3 m beside it, in its side
    // zone.
    let bench = ok(&mut app, "mesh_object.place", json!({ "type_id": t["Flat bench"], "point": [0.0, -2.3], "level": level }));
    assert_eq!(rules(&ok(&mut app, "rules.check", json!({}))), vec!["ASTM F2115 treadmill rear".to_owned()]);
    ok(&mut app, "element.delete", json!({ "element_ids": [bench] }));
    let bench = ok(&mut app, "mesh_object.place", json!({ "type_id": t["Flat bench"], "point": [-1.05, 0.0], "level": level }));
    assert_eq!(rules(&ok(&mut app, "rules.check", json!({}))), vec!["ASTM F2115 treadmill sides".to_owned()]);
    ok(&mut app, "element.delete", json!({ "element_ids": [bench] }));

    // Both treadmills marked not accessible: §236.1 is an error.
    let mills = [a, b];
    ok(&mut app, "rules.set_accessible", json!({ "element_ids": mills, "accessible": false }));
    let c = ok(&mut app, "gym.check", json!({}));
    assert_eq!((rules(&c["findings"]), c["findings"][0]["severity"].as_str()), (vec!["ADA 2010 236.1".to_owned()], Some("error")), "{c:#}");
    ok(&mut app, "rules.set_accessible", json!({ "element_ids": mills, "accessible": true }));

    // A power rack under a 2.4 m ceiling: the overhead press needs 2.7 m.
    let rack = ok(&mut app, "mesh_object.place", json!({ "type_id": t["Power rack"], "point": [6.0, 0.0], "level": level }));
    ok(&mut app, "ceiling.create", json!({ "loops": [square([4.0, -2.0], [8.0, 2.0])], "level_id": level, "offset": 2.4 }));
    let c = ok(&mut app, "gym.check", json!({}));
    assert_eq!((rules(&c["findings"]), c["findings"][0]["element"].clone()), (vec!["Ceiling height".to_owned()], rack), "{c:#}");

    // A mirror across the treadmills' rear, then one without safety glazing well away.
    let m = ok(&mut app, "gym.mirror", json!({ "start": [-1.0, -3.5], "end": [3.0, -3.5], "level": level, "toward": [0.0, 0.0] }));
    let c = ok(&mut app, "gym.check", json!({}));
    assert!(rules(&c["findings"]).contains(&"Sport England mirrors".to_owned()), "{c:#}");
    ok(&mut app, "element.delete", json!({ "element_ids": [m] }));
    ok(&mut app, "gym.mirror", json!({ "start": [10.0, -3.0], "end": [10.0, 3.0], "level": level, "toward": [6.0, 0.0], "safety_glazing": false }));
    let c = ok(&mut app, "gym.check", json!({}));
    assert_eq!(rules(&c["findings"]), vec!["Ceiling height".to_owned(), "Mirror safety glazing".to_owned()], "{c:#}");
    let s = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    assert!(s.iter().any(|e| e.class == "IfcCovering" && e.param(riviera_core::gym::MIRROR_KEY).is_some()));
}

fn room(app: &mut App, level: &Value, w: f64, d: f64) -> Value {
    let p = [[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]];
    for k in 0..4 {
        ok(app, "wall.create", json!({ "curve": { "line": { "start": p[k], "end": p[(k + 1) % 4] } }, "base_level": level, "top_constraint": { "unconnected": 3.5 } }));
    }
    ok(app, "room.place", json!({ "level_id": level, "point": [w / 2.0, d / 2.0] }))
}

#[test]
fn a_gym_is_laid_out_in_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "clearance.set_rule_set", json!({ "standard": "ada_2010" }));
    let r = room(&mut app, &level, 14.0, 10.0);
    // Four treadmills along the 14 m head wall, then bikes, a rower and strength in later rows.
    let pieces = json!([
        { "item": "Treadmill, commercial", "count": 4 },
        { "item": "Upright bike", "count": 2 },
        { "item": "Rower" },
        { "item": "Power rack" },
        { "item": "Leg press, selectorised" },
    ]);
    let l = ok(&mut app, "gym.layout", json!({ "room_id": r, "pieces": pieces }));
    assert_eq!((l["placed"].as_array().unwrap().len(), l["unplaced"].clone()), (9, json!({})), "{l:#}");
    assert!(l["rows"].as_u64() >= Some(2), "{l:#}");
    assert_eq!(ok(&mut app, "rules.check", json!({})), json!([]));
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    let c = ok(&mut app, "gym.check", json!({}));
    assert_eq!(c["findings"], json!([]), "{c:#}");
    // Its occupant load at 50 ft² gross: about 135 m² for 29 people.
    assert_eq!((c["rooms"][0]["pieces"].as_u64(), c["rooms"][0]["occupants"].as_u64().map(|n| (27..=30).contains(&n))), (Some(9), Some(true)), "{c:#}");
    // Twenty more treadmills do not all fit.
    let l = ok(&mut app, "gym.layout", json!({ "room_id": r, "pieces": [{ "item": "Treadmill, commercial", "count": 20 }] }));
    assert!(l["unplaced"]["Treadmill, commercial"].as_u64() > Some(0), "{l:#}");
}

#[test]
fn a_cycling_studio_is_sized_and_filled() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let r = room(&mut app, &level, 10.0, 8.0);
    let s = ok(&mut app, "gym.studio", json!({ "room_id": r, "kind": "cycle", "head": [5.0, 8.0] }));
    let placed = s["placed"].as_array().unwrap().len() as u64;
    assert!(placed >= 10 && placed <= s["capacity"].as_u64().unwrap(), "{s:#}");
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    // Each bike's console (its back) is toward the instructor's wall at y = 8.
    let st = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    let it = riviera_core::rules::item(&st, st.get(riviera_core::ElementId(s["placed"][0].as_u64().unwrap())).unwrap()).unwrap();
    assert!(it.frame.apply([0.0, it.back, 0.0])[1] > it.frame.apply([0.0, it.front, 0.0])[1], "{it:?}");
    // A mat studio of the same room: about 30 places at 25 ft², none placed.
    let m = ok(&mut app, "gym.studio", json!({ "room_id": r, "kind": "mat" }));
    assert_eq!((m["capacity"].as_u64().unwrap() / 10, m["placed"].clone()), (3, json!([])), "{m:#}");
}
