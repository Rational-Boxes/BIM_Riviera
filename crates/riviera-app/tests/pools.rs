// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Swimming pools (ARC-POOL-001): the pool element from a template or an outline, its depth
//! profile and read-outs, the shell and water it generates, the floor it cuts, and its IFC.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn square(a: [f64; 2], b: [f64; 2]) -> Value {
    let p = [[a[0], a[1]], [b[0], a[1]], [b[0], b[1]], [a[0], b[1]]];
    Value::Array((0..4).map(|k| json!({ "line": { "start": p[k], "end": p[(k + 1) % 4] } })).collect())
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn a_competition_pool_from_its_template() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // A deck slab round it, its top at the level.
    let floor = ok(&mut app, "floor.create", json!({ "loops": [square([-20.0, -12.0], [20.0, 12.0])], "level_id": level }));
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "competition_25m", "origin": [0.0, 0.0] } }));
    let i = ok(&mut app, "pool.info", json!({ "pool_id": pool, "points": [[0.0, 0.0]] }));
    // 25.03 × 15 m (six 2.5 m lanes), 2.0 m deep throughout.
    assert!(near(i["area"].as_f64().unwrap(), 25.03 * 15.0, 1e-6) && near(i["perimeter"].as_f64().unwrap(), 2.0 * (25.03 + 15.0), 1e-6), "{i:#}");
    assert!(near(i["volume"].as_f64().unwrap(), 25.03 * 15.0 * 2.0, 25.03 * 15.0 * 2.0 * 0.01), "{i:#}");
    assert_eq!((i["depths"][0].as_f64(), i["deepest"].as_f64(), i["segments"].as_array().unwrap().len()), (Some(2.0), Some(2.0), 0));
    assert!(i["shell_volume"].as_f64().unwrap() > 25.0 * 15.0 * 0.3, "{i:#}");
    // The deck has an opening round the pool, and nothing overlaps.
    let s = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    let openings: Vec<_> = s.iter().filter(|e| e.param(riviera_core::pools::CUT_KEY).is_some()).collect();
    assert_eq!(openings.len(), 1);
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    assert_eq!(ok(&mut app, "pool.list", json!({}))[0]["template"], "competition_25m");
    // Deleting the pool takes its opening with it.
    ok(&mut app, "element.delete", json!({ "element_ids": [pool] }));
    let s = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    assert!(!s.iter().any(|e| e.param(riviera_core::pools::CUT_KEY).is_some()));
    assert!(s.get(riviera_core::ElementId(floor.as_u64().unwrap())).is_some());
}

#[test]
fn a_public_pool_with_a_profile() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "floor.create", json!({ "loops": [square([-5.0, -5.0], [20.0, 12.0])], "level_id": level }));
    // A 15 × 8 m pool, its shallow end on x = 0: 1.0 m to 5 ft over 7 m, then to 2.0 m over 3 m.
    let outline = json!([[0.0, 8.0], [0.0, 0.0], [15.0, 0.0], [15.0, 8.0]]);
    let profile = json!([{ "at": 0.0, "depth": 1.0 }, { "at": 7.0, "depth": 1.524 }, { "at": 10.0, "depth": 2.0 }]);
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": outline, "profile": profile }));
    let i = ok(&mut app, "pool.info", json!({ "pool_id": pool, "points": [[3.5, 4.0], [12.0, 4.0]] }));
    let seg = i["segments"].as_array().unwrap();
    assert_eq!((seg[0]["ratio"].as_str(), seg[1]["ratio"].as_str()), (Some("1:13.4"), Some("1:6.3")), "{i:#}");
    assert!(near(i["depths"][0].as_f64().unwrap(), 1.262, 1e-3) && near(i["depths"][1].as_f64().unwrap(), 2.0, 1e-9), "{i:#}");
    // Volume: the trapezoids along the axis times the width.
    let v = 8.0 * (7.0 * (1.0 + 1.524) / 2.0 + 3.0 * (1.524 + 2.0) / 2.0 + 5.0 * 2.0);
    assert!(near(i["volume"].as_f64().unwrap(), v, 1e-6), "{} vs {v}", i["volume"]);
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    // A deeper deep end: the shell and water follow, in one undo step.
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "profile": [{ "at": 0.0, "depth": 1.0 }, { "at": 7.0, "depth": 1.524 }, { "at": 10.0, "depth": 3.0 }] }));
    let j = ok(&mut app, "pool.info", json!({ "pool_id": pool }));
    assert_eq!(j["deepest"].as_f64(), Some(3.0));
    assert!(j["volume"].as_f64() > i["volume"].as_f64());
    ok(&mut app, "edit.undo", json!({}));
    assert_eq!(ok(&mut app, "pool.info", json!({ "pool_id": pool }))["deepest"].as_f64(), Some(2.0));
    // A bad profile is refused.
    assert!(app.execute("pool.set", json!({ "pool_id": pool, "profile": [{ "at": 5.0, "depth": 1.0 }, { "at": 1.0, "depth": 2.0 }] }), &Caller::user()).is_err());
}

#[test]
fn a_pool_in_ifc() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "teaching", "origin": [0.0, 0.0] }, "name": "Teaching pool" }));
    let model = dir.path().join("pool.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let ifc = riviera_ifc::IfcModel::open(&model).unwrap();
    let rows = ifc.step_rows().unwrap();
    // An assembly of a shell wall, a base slab and coping; a WATER space with its volume.
    let assembly = rows.iter().find(|r| r.class == "IFCELEMENTASSEMBLY" && r.attributes.contains("'SWIMMINGPOOL'")).unwrap();
    let agg = rows.iter().find(|r| r.class == "IFCRELAGGREGATES" && ifc.attribute(r.id, "RelatingObject").unwrap() == riviera_ifc::AttrValue::Ref(assembly.id.0)).unwrap();
    let riviera_ifc::AttrValue::List(parts) = ifc.attribute(agg.id, "RelatedObjects").unwrap() else { panic!() };
    let mut classes: Vec<String> = parts.iter().map(|m| if let riviera_ifc::AttrValue::Ref(m) = m { ifc.class_of(riviera_ifc::StepId(*m)).unwrap().to_ascii_uppercase() } else { panic!() }).collect();
    classes.sort();
    assert_eq!(classes, vec!["IFCCOVERING", "IFCSLAB", "IFCWALL"]);
    assert!(rows.iter().any(|r| r.class == "IFCSLAB" && r.attributes.contains(".BASESLAB.")));
    let water = rows.iter().find(|r| r.class == "IFCSPACE" && r.attributes.contains("'WATER'")).unwrap();
    assert!(water.attributes.contains("'Teaching pool water'"), "{}", water.attributes);
    let vol = rows.iter().find(|r| r.class == "IFCQUANTITYVOLUME").unwrap();
    let riviera_ifc::AttrValue::Real(v) = ifc.attribute(vol.id, "VolumeValue").unwrap() else { panic!() };
    assert!(near(v, 12.0 * 7.0 * 0.9, 1e-6), "12 × 7 × 0.9 m: {v}");
    // It reads back as one pool, its parts not loaded as elements; saving again changes nothing.
    let mut again = App::new(AppConfig { live: None, untitled_dir: dir.path().join("v") });
    ok(&mut again, "file.open", json!({ "path": model }));
    let pools = ok(&mut again, "pool.list", json!({}));
    assert_eq!((pools.as_array().unwrap().len(), pools[0]["template"].as_str()), (1, Some("teaching")), "{pools:#}");
    let s = again.document(again.active().unwrap()).unwrap().doc.store().clone();
    assert!(!s.iter().any(|e| e.class == "IfcSpace" && e.name.as_deref() == Some("Teaching pool water")));
    assert!(!s.iter().any(|e| e.class == "IfcWall"));
    ok(&mut again, "file.save", json!({}));
    let ifc2 = riviera_ifc::IfcModel::open(&model).unwrap();
    assert_eq!(ifc2.step_rows().unwrap().iter().filter(|r| r.class == "IFCSPACE" && r.attributes.contains("'WATER'")).count(), 1);
}
