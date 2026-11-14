// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Type catalogues (DM-TYPE-002): every kind of element lists the types it could take, those in
//! the model and those it can load, and takes a loaded one.

use riviera_app::{App, AppConfig};
use riviera_core::{Caller, ElementId};
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn store(app: &App) -> riviera_core::Store {
    app.document(app.active().unwrap()).unwrap().doc.store().clone()
}

/// The element's type's name.
fn type_name(app: &App, id: &Value) -> String {
    let s = store(app);
    let e = s.get(ElementId(id.as_u64().unwrap())).unwrap();
    let t = e.param("@type").and_then(Value::as_str).and_then(riviera_core::GlobalId::parse).and_then(|g| s.find_by_guid(&g)).unwrap();
    s.get(t).unwrap().name.clone().unwrap()
}

/// Loads `name` from `id`'s catalogue and gives it to `id`.
fn swap(app: &mut App, id: &Value, name: &str) {
    let list = ok(app, "type.catalogue", json!({ "element_id": id }));
    assert!(list.as_array().unwrap().iter().any(|e| e["name"] == name), "{name} not in {list:#}");
    let t = ok(app, "type.load", json!({ "element_id": id, "name": name }));
    ok(app, "element.set_type", json!({ "element_ids": [id], "type_id": t }));
    assert_eq!(type_name(app, id), name);
    // Loading it again reuses it.
    assert_eq!(ok(app, "type.load", json!({ "element_id": id, "name": name })), t);
}

fn sq(x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let p = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    json!((0..4).map(|k| json!({ "line": { "start": p[k], "end": p[(k + 1) % 4] } })).collect::<Vec<_>>())
}

#[test]
fn every_kind_loads_and_takes_a_catalogue_type() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let wall = ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [0.0, 0.0], "end": [10.0, 0.0] } }, "base_level": level, "top_constraint": { "unconnected": 3.5 } }));
    let door = ok(&mut app, "door.place", json!({ "host_wall_id": wall, "point_along": 2.0 }));
    let window = ok(&mut app, "window.place", json!({ "host_wall_id": wall, "point_along": 6.0 }));

    // A door: the model's single default first, then the starters.
    let list = ok(&mut app, "type.catalogue", json!({ "element_id": door }));
    assert_eq!((list[0]["name"].as_str(), list[0]["source"].as_str()), (Some("Single Door 900 x 2100"), Some("In the model")), "{list:#}");
    assert!(list.as_array().unwrap().iter().all(|e| !e["name"].as_str().unwrap().starts_with("Window")));
    swap(&mut app, &door, "Double Door 1800 x 2100");
    swap(&mut app, &door, "Sliding Door 1800 x 2100, automatic");
    swap(&mut app, &window, "Window 1800 x 1200");
    swap(&mut app, &window, "Round Window 900");

    // A wall: a layered starter, its width the layers' total.
    swap(&mut app, &wall, "Exterior - Brick and Block Cavity 300 mm");
    let s = store(&app);
    let w = riviera_core::walls::regenerate(&s, s.get(ElementId(wall.as_u64().unwrap())).unwrap()).unwrap();
    assert!((w.width - 0.3).abs() < 1e-9, "{}", w.width);

    // Floors and ceilings keep to their own kind.
    let floor = ok(&mut app, "floor.create", json!({ "loops": [sq(0.0, 1.0, 10.0, 8.0)], "level_id": level }));
    let ceiling = ok(&mut app, "ceiling.create", json!({ "loops": [sq(0.0, 1.0, 10.0, 8.0)], "level_id": level, "offset": 2.7 }));
    let floors = ok(&mut app, "type.catalogue", json!({ "element_id": floor }));
    assert!(floors.as_array().unwrap().iter().all(|e| !e["name"].as_str().unwrap().contains("Ceiling")), "{floors:#}");
    swap(&mut app, &floor, "Floor - Tile on Screed and Concrete 210 mm");
    swap(&mut app, &ceiling, "Ceiling - Acoustic Tile 600 x 600, 20 mm");
    let ceilings = ok(&mut app, "type.catalogue", json!({ "element_id": ceiling }));
    assert!(ceilings.as_array().unwrap().iter().all(|e| !e["name"].as_str().unwrap().starts_with("Floor")), "{ceilings:#}");

    // A roof and a ramp.
    let roof = ok(&mut app, "roof.create_footprint", json!({ "curves": sq(0.0, 1.0, 10.0, 8.0), "level_id": level, "base_offset": 3.5, "slope": "30°" }));
    swap(&mut app, &roof, "Roof - Timber Rafters 250 mm");
    let ramp = ok(&mut app, "ramp.create", json!({ "path": [[0.0, 12.0], [12.0, 12.0]], "base_level": level, "top_constraint": { "unconnected": 0.5 } }));
    swap(&mut app, &ramp, "Ramp 1:20");

    // A component: the libraries' items of its category, nothing of another.
    let bed = ok(&mut app, "furnishing.load", json!({ "names": ["Bed, king 76 × 80 in"] }))["Bed, king 76 × 80 in"].clone();
    let bed = ok(&mut app, "mesh_object.place", json!({ "type_id": bed, "point": [3.0, 5.0], "level": level }));
    let list = ok(&mut app, "type.catalogue", json!({ "element_id": bed }));
    assert!(list.as_array().unwrap().iter().any(|e| e["name"] == "Desk 60 × 30 in"), "{list:#}");
    assert!(list.as_array().unwrap().iter().all(|e| e["name"] != "WC, close-coupled" && e["name"] != "Supply Diffuser 600"));
    swap(&mut app, &bed, "Desk 60 × 30 in");

    // A footing lists the model's footing types only; a name not there is refused.
    let footing = ok(&mut app, "footing.place", json!({ "point": [20.0, 20.0], "level": level }));
    let list = ok(&mut app, "type.catalogue", json!({ "element_id": footing }));
    assert_eq!(list.as_array().unwrap().len(), 1, "{list:#}");
    let e = app.execute("type.load", json!({ "element_id": footing, "name": "Window 600 x 600" }), &Caller::user()).unwrap_err();
    assert_eq!(e.code, "type.not_in_catalogue");
}

#[test]
fn a_duct_takes_another_type_and_size() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let run = ok(&mut app, "duct.create", json!({ "points": [[0.0, 0.0, 2.7], [6.0, 0.0, 2.7], [6.0, 5.0, 2.7]], "level": level, "shape": "round", "diameter": 0.3, "system_type_id": "supply_air" }));
    let seg = run["segments"][0].clone();
    let spiral = ok(&mut app, "mep_segment_type.create", json!({ "domain": "duct", "name": "Spiral Duct", "shape": "round", "material": "Galvanised steel" }));
    ok(&mut app, "mep_segment_type.create", json!({ "domain": "duct", "name": "Rectangular Duct", "shape": "rectangular" }));
    // Round ducts list round duct types only.
    let list = ok(&mut app, "type.catalogue", json!({ "element_id": seg }));
    let names: Vec<&str> = list.as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"Spiral Duct") && !names.contains(&"Rectangular Duct"), "{names:?}");
    ok(&mut app, "element.set_type", json!({ "element_ids": [seg], "type_id": spiral }));
    assert_eq!(type_name(&app, &seg), "Spiral Duct");
    // A size, the elbow's leg following.
    ok(&mut app, "duct.set_size", json!({ "element_ids": [seg], "diameter": 0.4 }));
    let s = store(&app);
    let r = riviera_core::mep_runs::segment_recipe(s.get(ElementId(seg.as_u64().unwrap())).unwrap()).unwrap();
    assert!((r.size.diameter - 0.4).abs() < 1e-9);
    // A rectangular type is refused for a round duct.
    let rect = s.iter().find(|e| e.name.as_deref() == Some("Rectangular Duct")).unwrap().id();
    let e = app.execute("element.set_type", json!({ "element_ids": [seg], "type_id": rect }), &Caller::user()).unwrap_err();
    assert_eq!(e.code, "type.wrong_family");
}

#[test]
fn a_railing_takes_another_type_and_its_values_change() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let rail = ok(&mut app, "railing.create", json!({ "path": [[0.0, 0.0], [5.0, 0.0]], "base_level": level }));
    swap(&mut app, &rail, "Guard 1067 mm (42 in)");
    let s = store(&app);
    let t = s.iter().find(|e| e.name.as_deref() == Some("Guard 1067 mm (42 in)")).unwrap().id();
    ok(&mut app, "type.set_param", json!({ "type_id": t, "param": "height", "value": "1100 mm" }));
    let s = store(&app);
    let ty = riviera_core::railings::type_of(&s, s.get(ElementId(rail.as_u64().unwrap())).unwrap());
    assert!((ty.height - 1.1).abs() < 1e-9);
    // Balusters no wider apart than they are: refused.
    let e = app.execute("type.set_param", json!({ "type_id": t, "param": "baluster_spacing", "value": 0.01 }), &Caller::user()).unwrap_err();
    assert_eq!(e.code, "type.invalid_value");
}

#[test]
fn a_stair_takes_another_type_and_its_rules_change() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let l1 = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let l2 = ok(&mut app, "level.create", json!({ "elevation": 3.0, "name": "Level 2" }));
    let _ = l2;
    let stair = ok(&mut app, "stair.create", json!({ "base_level": l1, "railings": false, "runs": [{ "start": [0.0, 0.0], "end": [8.0, 0.0] }] }));
    swap(&mut app, &stair, "Stair - Commercial 178 / 279, 1118 wide");
    let s = store(&app);
    let g = riviera_core::stairs::regenerate(&s, s.get(ElementId(stair.as_u64().unwrap())).unwrap()).unwrap();
    assert!(g.riser_height <= 0.178 + 1e-9, "{}", g.riser_height);
    let t = s.iter().find(|e| e.name.as_deref() == Some("Stair - Commercial 178 / 279, 1118 wide")).unwrap().id();
    ok(&mut app, "type.set_param", json!({ "type_id": t, "param": "max_riser", "value": "150 mm" }));
    let s = store(&app);
    let g = riviera_core::stairs::regenerate(&s, s.get(ElementId(stair.as_u64().unwrap())).unwrap()).unwrap();
    assert!(g.riser_height <= 0.15 + 1e-9, "{}", g.riser_height);
}

#[test]
fn a_type_saved_to_the_users_catalogue_loads_in_another_project() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("shared/type-catalogue.json");
    let file = file.to_str().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let wall = ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [0.0, 0.0], "end": [10.0, 0.0] } }, "base_level": level, "top_constraint": { "unconnected": 3.5 } }));
    let door = ok(&mut app, "door.place", json!({ "host_wall_id": wall, "point_along": 2.0 }));
    // A door type of the office's own, saved from a door.
    let t = ok(&mut app, "type.load", json!({ "element_id": door, "name": "Double Door 1800 x 2100" }));
    let mine = ok(&mut app, "type.duplicate", json!({ "type_id": t, "new_name": "Office Double Door 1700" }));
    ok(&mut app, "type.set_param", json!({ "type_id": mine, "param": "width", "value": "1700 mm" }));
    ok(&mut app, "element.set_type", json!({ "element_ids": [door], "type_id": mine }));
    let s = ok(&mut app, "type.save_to_catalogue", json!({ "type_id": door, "catalogue": file }));
    assert_eq!((s["name"].as_str(), s["replaced"].as_bool()), (Some("Office Double Door 1700"), Some(false)));
    assert_eq!(ok(&mut app, "type.save_to_catalogue", json!({ "type_id": mine, "catalogue": file }))["replaced"], true);
    // A layered wall type too.
    let w = ok(&mut app, "type.load", json!({ "element_id": wall, "name": "Exterior - Brick and Block Cavity 300 mm" }));
    let w = ok(&mut app, "type.duplicate", json!({ "type_id": w, "new_name": "Office Cavity Wall" }));
    ok(&mut app, "type.save_to_catalogue", json!({ "type_id": w, "catalogue": file }));

    // Another project: listed as the user's, loaded with its width and layers.
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let wall = ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": [0.0, 0.0], "end": [10.0, 0.0] } }, "base_level": level, "top_constraint": { "unconnected": 3.5 } }));
    let door = ok(&mut app, "door.place", json!({ "host_wall_id": wall, "point_along": 2.0 }));
    let list = ok(&mut app, "type.catalogue", json!({ "element_id": door, "catalogue": file }));
    let e = list.as_array().unwrap().iter().find(|e| e["name"] == "Office Double Door 1700").unwrap_or_else(|| panic!("{list:#}"));
    assert_eq!(e["source"], "Your catalogue");
    // Walls' entries are not a door's.
    assert!(list.as_array().unwrap().iter().all(|e| e["name"] != "Office Cavity Wall"));
    let t = ok(&mut app, "type.load", json!({ "element_id": door, "name": "Office Double Door 1700", "catalogue": file }));
    ok(&mut app, "element.set_type", json!({ "element_ids": [door], "type_id": t }));
    let st = store(&app);
    let it = riviera_core::inserts::insert_type(st.get(ElementId(t.as_u64().unwrap())).unwrap()).unwrap();
    assert!((it.width - 1.7).abs() < 1e-9, "{}", it.width);
    let t = ok(&mut app, "type.load", json!({ "element_id": wall, "name": "Office Cavity Wall", "catalogue": file }));
    ok(&mut app, "element.set_type", json!({ "element_ids": [wall], "type_id": t }));
    let st = store(&app);
    let g = riviera_core::walls::regenerate(&st, st.get(ElementId(wall.as_u64().unwrap())).unwrap()).unwrap();
    assert!((g.width - 0.3).abs() < 1e-9, "{}", g.width);
    // Removed: no longer listed.
    assert_eq!(ok(&mut app, "type.remove_from_catalogue", json!({ "element_id": door, "name": "Office Double Door 1700", "catalogue": file })), true);
    let list = ok(&mut app, "type.catalogue", json!({ "element_id": door, "catalogue": file }));
    assert!(list.as_array().unwrap().iter().all(|e| e["source"] != "Your catalogue"), "{list:#}");
    // A component's type: kept in component libraries instead.
    let bed = ok(&mut app, "furnishing.load", json!({ "names": ["Bed, king 76 × 80 in"] }))["Bed, king 76 × 80 in"].clone();
    let e = app.execute("type.save_to_catalogue", json!({ "type_id": bed, "catalogue": file }), &Caller::user()).unwrap_err();
    assert_eq!(e.code, "type.not_savable");
}
