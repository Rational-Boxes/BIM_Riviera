// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Modular units (ARC-GRP-004): a hotel room made a unit type and arrayed down both sides of a
//! corridor on two floors, merged with the building's walls and floors, connected to the
//! corridor's supply-air main, and edited once for every copy.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn line(a: [f64; 2], b: [f64; 2]) -> Value {
    json!({ "line": { "start": a, "end": b } })
}

fn wall(app: &mut App, level: &Value, a: [f64; 2], b: [f64; 2]) -> Value {
    ok(app, "wall.create", json!({ "curve": line(a, b), "base_level": level, "top_constraint": { "unconnected": 3.5 } }))
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let p = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    json!([(0..4).map(|k| line(p[k], p[(k + 1) % 4])).collect::<Vec<_>>()])
}

/// Two guest floors (3.5 m and 7.0 m) of a building 30 m long: a 2 m corridor along y = 9,
/// rooms 6 m deep each side to façades at y = 16 and y = 2, a core from x = 13 to 17, and a
/// supply-air main along the corridor at each floor; and the room unit drawn off to the side.
fn setup() -> (tempfile::TempDir, App, Vec<Value>, Vec<Value>) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let levels: Vec<Value> = [3.5, 7.0].iter().enumerate().map(|(k, z)| ok(&mut app, "level.create", json!({ "elevation": z, "name": format!("Level {}", k + 2) }))).collect();
    let t = ok(&mut app, "mep_type.load_starters", json!({ "names": ["Supply Diffuser 600"] }));
    let mut mains = Vec::new();
    for l in &levels {
        ok(&mut app, "floor.create", json!({ "loops": rect(0.0, 2.0, 30.0, 16.0), "level_id": l }));
        for (a, b) in [([0.0, 16.0], [30.0, 16.0]), ([0.0, 2.0], [30.0, 2.0]), ([0.0, 10.0], [30.0, 10.0]), ([0.0, 8.0], [30.0, 8.0])] {
            wall(&mut app, l, a, b);
        }
        let d = ok(&mut app, "duct.create", json!({ "points": [[0.5, 9.0, 3.1], [29.5, 9.0, 3.1]], "level": l, "shape": "round", "diameter": 0.4, "system_type_id": "supply_air" }));
        mains.push(d["segments"][0].clone());
    }
    // The room, on level 2 at x = 100.
    let members = draw_room(&mut app, &levels[0], 100.0, &t["Supply Diffuser 600"]);
    (dir, app, levels, [members, mains].concat())
}

/// A room on level 2 from `x0`: its corridor wall with the entrance door, its façade wall with a
/// window, its two side walls, the room, a bed, a ceiling diffuser and a balcony.
fn draw_room(app: &mut App, l2: &Value, x0: f64, diffuser_type: &Value) -> Vec<Value> {
    let corridor = wall(app, l2, [x0, 10.0], [x0 + 4.0, 10.0]);
    let facade = wall(app, l2, [x0, 16.0], [x0 + 4.0, 16.0]);
    let left = wall(app, l2, [x0, 10.0], [x0, 16.0]);
    let right = wall(app, l2, [x0 + 4.0, 10.0], [x0 + 4.0, 16.0]);
    ok(app, "door.place", json!({ "host_wall_id": corridor, "point_along": 1.0 }));
    ok(app, "window.place", json!({ "host_wall_id": facade, "point_along": 2.0 }));
    let room = ok(app, "room.place", json!({ "level_id": l2, "point": [x0 + 2.0, 13.0] }));
    let diffuser = ok(app, "air_terminal.place", json!({ "type_id": diffuser_type, "point": [x0 + 2.5, 12.0], "level": l2, "offset": 2.5, "flow": 0.05 }))["id"].clone();
    let balcony = ok(app, "floor.create", json!({ "loops": rect(x0 + 0.5, 16.1, x0 + 3.5, 17.6), "level_id": l2 }));
    let bed = ok(app, "furnishing.load", json!({ "names": ["Bed, king 76 × 80 in"] }))["Bed, king 76 × 80 in"].clone();
    let bed = ok(app, "mesh_object.place", json!({ "type_id": bed, "point": [x0 + 2.0, 14.0], "level": l2 }));
    vec![corridor, facade, left, right, room, diffuser, balcony, bed]
}

#[test]
fn every_fourth_room_accessible() {
    let (_dir, mut app, levels, all) = setup();
    let members = all[..all.len() - 2].to_vec();
    ok(&mut app, "unit.create_type", json!({ "element_ids": members, "name": "King room", "origin": [100.0, 10.0], "width": 4.0, "depth": 6.0 }));
    let t = ok(&mut app, "mep_type.load_starters", json!({ "names": ["Supply Diffuser 600"] }));
    let acc = draw_room(&mut app, &levels[0], 120.0, &t["Supply Diffuser 600"]);
    ok(&mut app, "unit.create_type", json!({ "element_ids": acc, "name": "Accessible king", "origin": [120.0, 10.0], "width": 4.0, "depth": 6.0 }));
    let rep = ok(&mut app, "unit.array", json!({ "unit_type": "King room", "path": [[0.0, 9.0], [30.0, 9.0]], "levels": levels, "sides": "both", "corridor": 2.0, "start": 1.0, "exclusions": [[13.0, 17.0]], "variants": [{ "unit_type": "Accessible king", "every": 4 }] }));
    // On each floor's twelve, the 1st, 5th and 9th.
    assert_eq!(rep["per_type"], json!({ "King room": 18, "Accessible king": 6 }), "{rep:#}");
    let list = ok(&mut app, "unit.list", json!({}));
    let acc: Vec<&str> = list.as_array().unwrap().iter().filter(|c| c["unit_type"] == "Accessible king").map(|c| c["number"].as_str().unwrap()).collect();
    assert_eq!(acc, vec!["201", "205", "209", "301", "305", "309"]);
    let types = ok(&mut app, "unit.types", json!({}));
    assert!(types.as_array().unwrap().iter().any(|t| t["name"] == "Accessible king" && t["copies"] == 6), "{types:#}");
}

#[test]
fn hotel_rooms_down_a_corridor() {
    let (_dir, mut app, levels, all) = setup();
    let (members, mains) = (all[..all.len() - 2].to_vec(), all[all.len() - 2..].to_vec());
    // The unit type: the bay from (100, 10) along +x, 4 m wide and 6 m deep.
    let ty = ok(&mut app, "unit.create_type", json!({ "element_ids": members, "name": "King room", "origin": [100.0, 10.0], "width": 4.0, "depth": 6.0 }));
    assert!(ty["members"]["IfcWall"] == 4 && ty["members"]["IfcDoor"] == 1 && ty["members"]["IfcWindow"] == 1, "{ty:#}");
    // The originals are gone into the type.
    assert_eq!(riviera_app_store(&app).iter().filter(|e| e.class == "IfcSpace").count(), 0);
    // Down both sides of the corridor on both floors from 1 m, skipping the core: bays at 1, 5,
    // 9, 17, 21 and 25 m, six a side.
    let rep = ok(&mut app, "unit.array", json!({ "unit_type": "King room", "path": [[0.0, 9.0], [30.0, 9.0]], "levels": levels, "sides": "both", "corridor": 2.0, "start": 1.0, "exclusions": [[13.0, 17.0]] }));
    assert_eq!(rep["copies"], 24, "{rep:#}");
    assert_eq!(rep["per_level"], json!({ "Level 2": 12, "Level 3": 12 }));
    assert_eq!(rep["unmerged"], json!([]), "{rep:#}");
    let list = ok(&mut app, "unit.list", json!({}));
    let numbers: Vec<&str> = list.as_array().unwrap().iter().map(|c| c["number"].as_str().unwrap()).collect();
    assert_eq!(numbers[..3], ["201", "202", "203"]);
    assert_eq!(numbers[12..15], ["301", "302", "303"]);
    assert!(list.as_array().unwrap().iter().filter(|c| c["mirrored"] == true).count() == 12);
    // Each copy's corridor and façade walls merged into the building's, and the side walls
    // between neighbours built once: per side and floor, 12 side walls less 4 shared.
    let walls = riviera_app_store(&app).iter().filter(|e| riviera_core::walls::wall_recipe(e).is_some()).count();
    assert_eq!(walls, 8 + 4 * 8, "the building's 8 walls and 8 unit walls per side per floor");
    // The doors and windows are in the building's walls: 24 of each.
    let st = riviera_app_store(&app);
    let hosts: Vec<String> = st.iter().filter(|e| e.class == "IfcDoor" || e.class == "IfcWindow").filter_map(|e| riviera_core::inserts::insert_recipe(e).map(|r| r.host)).collect();
    assert_eq!(hosts.len(), 48);
    let building: Vec<String> = st.iter().filter(|e| e.class == "IfcWall" && riviera_core::groups::instance_of(e).is_none()).map(|e| e.guid().to_string()).collect();
    assert!(hosts.iter().all(|h| building.contains(h)), "every door and window is in a building wall");
    // Rooms numbered by their copies; balconies joined to each floor.
    let rooms: Vec<String> = st.iter().filter(|e| riviera_core::rooms::room_recipe(e).is_some()).filter_map(|e| e.name.clone()).collect();
    assert!(rooms.len() == 24 && rooms.contains(&"201".to_owned()) && rooms.contains(&"312".to_owned()), "{rooms:?}");
    assert_eq!(st.iter().filter(|e| e.param("rxd:joined_to").is_some()).count(), 24);
    // Connected to the corridor mains: one branch a room, the flow the sum of twelve rooms'.
    let c = ok(&mut app, "unit.connect", json!({ "array_id": rep["array_id"], "spines": mains }));
    assert_eq!((c["branches"].as_u64(), c["unconnected"].as_array().unwrap().len()), (Some(24), 0), "{c:#}");
    for s in c["spines"].as_array().unwrap() {
        assert!(s["branches"] == 12 && (s["flow"].as_f64().unwrap() - 0.6).abs() < 1e-9, "{s:#}");
    }
    assert!(ok(&mut app, "unit.list", json!({})).as_array().unwrap().iter().all(|c| c["open_connectors"] == 0));
    let copy = list[0]["copy"].clone();
    // Editing one copy edits them all: its window moved 0.5 m in the group editor.
    ok(&mut app, "group.edit", json!({ "instance_id": copy }));
    let windows_before: Vec<f64> = alongs(&app, "IfcWindow");
    let w = riviera_app_store(&app).iter().find(|e| e.class == "IfcWindow" && riviera_core::groups::instance_of(e).is_some_and(|g| Some(g) == guid_of(&app, &copy))).map(|e| e.id()).unwrap();
    ok(&mut app, "element.move", json!({ "element_ids": [w], "vector": [0.5, 0.0, 0.0] }));
    ok(&mut app, "group.finish", json!({}));
    let windows_after = alongs(&app, "IfcWindow");
    assert_eq!(windows_after.len(), 24);
    assert_ne!(windows_before, windows_after);
    // Each copy's window is still its own, in the façade in front of it.
    let st = riviera_app_store(&app);
    let mut places: Vec<(String, i64)> = st.iter().filter(|e| e.class == "IfcWindow").filter_map(|e| riviera_core::inserts::insert_recipe(e).map(|r| (r.host, (r.along * 100.0).round() as i64))).collect();
    places.sort();
    places.dedup();
    assert_eq!(places.len(), 24, "{places:?}");
    assert!(places.iter().all(|(h, _)| building.contains(h)));
    assert_eq!(ok(&mut app, "unit.list", json!({})).as_array().unwrap().iter().filter(|c| !c["unmerged"].as_array().unwrap().is_empty()).count(), 0);
    // Fewer rooms: one level, three a side.
    let rep = ok(&mut app, "unit.set_array", json!({ "array_id": rep["array_id"], "levels": [levels[1]], "count": 3 }));
    assert_eq!(rep["copies"], 6, "{rep:#}");
}

fn riviera_app_store(app: &App) -> riviera_core::Store {
    app.document(app.active().unwrap()).unwrap().doc.store().clone()
}

fn guid_of(app: &App, id: &Value) -> Option<String> {
    let s = riviera_app_store(app);
    s.iter().find(|e| json!(e.id()) == *id).map(|e| e.guid().to_string())
}

fn alongs(app: &App, class: &str) -> Vec<f64> {
    let mut v: Vec<f64> = riviera_app_store(app).iter().filter(|e| e.class == class).filter_map(|e| riviera_core::inserts::insert_recipe(e).map(|r| (r.along * 1000.0).round() / 1000.0)).collect();
    v.sort_by(f64::total_cmp);
    v
}

#[test]
fn fixtures_on_a_shared_side_wall_stay_in_their_rooms() {
    let (_dir, mut app, levels, all) = setup();
    let mut members = all[..all.len() - 2].to_vec();
    // A wall-hung lavatory on the room's right-hand side wall (x = 104), 1.5 m from the corridor:
    // copies' side walls merge into their neighbours', so it must move onto the wall that stays.
    let lav = ok(&mut app, "bath.load_components", json!({ "names": ["Lavatory, wall-hung"] }))["Lavatory, wall-hung"].clone();
    let fixture = ok(&mut app, "plumbing_fixture.place", json!({ "type_id": lav, "point": [103.9, 11.5], "level": levels[0], "host_wall": members[3] }));
    members.push(fixture);
    ok(&mut app, "unit.create_type", json!({ "element_ids": members, "name": "King room", "origin": [100.0, 10.0], "width": 4.0, "depth": 6.0 }));
    let rep = ok(&mut app, "unit.array", json!({ "unit_type": "King room", "path": [[0.0, 9.0], [30.0, 9.0]], "levels": levels, "sides": "both", "corridor": 2.0, "start": 1.0, "exclusions": [[13.0, 17.0]] }));
    assert_eq!(rep["copies"], 24, "{rep:#}");
    // Every lavatory inside its own bay, 1.5 m from its corridor, on a wall that is there.
    let st = riviera_app_store(&app);
    let fixtures: Vec<riviera_core::mesh_objects::MeshObjectRecipe> = st.iter().filter(|e| e.class == "IfcSanitaryTerminal").filter_map(riviera_core::mesh_objects::mesh_object_recipe).collect();
    assert_eq!(fixtures.len(), 24);
    for r in &fixtures {
        let f = r.face.as_ref().expect("on a wall");
        assert!(st.find_by_guid(&riviera_core::GlobalId::parse(&f.wall).unwrap()).is_some(), "its wall is there: {r:?}");
        let [x, y] = r.point;
        let from_corridor = if y > 9.0 { y - 10.0 } else { 8.0 - y };
        assert!((from_corridor - 1.5).abs() < 0.01, "{r:?}");
        assert!(x > 0.9 && x < 29.1 && !(13.0..17.0).contains(&x), "{r:?}");
    }
    // Editing a copy undoes its merges and makes them again: the lavatories stay in place.
    let copy = ok(&mut app, "unit.list", json!({}))[1]["copy"].clone();
    ok(&mut app, "group.edit", json!({ "instance_id": copy }));
    ok(&mut app, "group.finish", json!({}));
    let st = riviera_app_store(&app);
    let after: Vec<[i64; 2]> = st.iter().filter(|e| e.class == "IfcSanitaryTerminal").filter_map(riviera_core::mesh_objects::mesh_object_recipe).map(|r| r.point.map(|v| (v * 100.0).round() as i64)).collect();
    let mut before: Vec<[i64; 2]> = fixtures.iter().map(|r| r.point.map(|v| (v * 100.0).round() as i64)).collect();
    let mut after = after;
    before.sort();
    after.sort();
    assert_eq!(before, after);
    // None in a wall or in another (a mirrored copy's on the wrong face would meet its neighbour's).
    let o = ok(&mut app, "overlap.find", json!({}));
    let fixtures: Vec<&Value> = o["overlaps"].as_array().unwrap().iter().filter(|o| format!("{}{}", o["a_label"], o["b_label"]).contains("IfcSanitaryTerminal")).collect();
    assert!(fixtures.is_empty(), "{fixtures:#?}");
    // Their clear spaces in their rooms, not through the walls they hang on.
    let c = ok(&mut app, "clearance.check", json!({}));
    let spaces: Vec<&Value> = c.as_array().unwrap().iter().filter(|i| i["kind"] == "fixture_space").collect();
    assert!(spaces.is_empty(), "{spaces:#?}");
}


#[test]
fn a_unit_edited_in_isolation_updates_every_copy() {
    let (_dir, mut app, levels, all) = setup();
    let members = all[..all.len() - 2].to_vec();
    ok(&mut app, "unit.create_type", json!({ "element_ids": members, "name": "King room", "origin": [100.0, 10.0], "width": 4.0, "depth": 6.0 }));
    let rep = ok(&mut app, "unit.array", json!({ "unit_type": "King room", "path": [[0.0, 9.0], [30.0, 9.0]], "levels": levels, "sides": "both", "corridor": 2.0, "start": 1.0, "exclusions": [[13.0, 17.0]] }));
    let walls = |app: &App| riviera_app_store(app).iter().filter(|e| riviera_core::walls::wall_recipe(e).is_some()).count();
    let count = |app: &App, class: &str| riviera_app_store(app).iter().filter(|e| e.class == class).count();
    assert_eq!(walls(&app), 40);

    // Nothing to edit: an element that is not a unit's.
    let e = app.execute("unit.edit", json!({ "element_id": all[all.len() - 1] }), &Caller::user()).unwrap_err();
    assert_eq!(e.code, "unit.no_copy");

    // From the type: its first copy not mirrored, with the walls it had merged made again (its
    // corridor and façade walls), its door and window back in them.
    let ed = ok(&mut app, "unit.edit", json!({ "unit_type": "King room" }));
    let copies = ok(&mut app, "unit.list", json!({}));
    let first = copies.as_array().unwrap().iter().find(|c| c["mirrored"] == false).unwrap();
    assert_eq!((&ed["copy"], &ed["number"], &ed["unit_type"], &ed["copies"]), (&first["copy"], &first["number"], &json!("King room"), &json!(24)), "{ed:#}");
    let st = riviera_app_store(&app);
    let mine: Vec<riviera_core::ElementId> = ed["members"].as_array().unwrap().iter().map(|v| riviera_core::ElementId(v.as_u64().unwrap())).collect();
    let classes = |c: &str| mine.iter().filter(|id| st.get(**id).is_some_and(|e| e.class == c)).count();
    assert_eq!((classes("IfcWall"), classes("IfcDoor"), classes("IfcWindow"), classes("IfcSpace")), (4, 1, 1, 1), "{ed:#}");
    let own: Vec<String> = mine.iter().filter_map(|id| st.get(*id)).filter(|e| e.class == "IfcWall").map(|e| e.guid().to_string()).collect();
    for id in &mine {
        if let Some(r) = st.get(*id).and_then(riviera_core::inserts::insert_recipe) {
            assert!(own.contains(&r.host), "its door and window are in its own walls while it is edited");
        }
    }
    // One editor at a time.
    assert_eq!(app.execute("unit.edit", json!({ "unit_type": "King room" }), &Caller::user()).unwrap_err().code, "group.editing");

    // A second bed added in the editor: every copy gets one, and merges again.
    let bed = ok(&mut app, "furnishing.load", json!({ "names": ["Bed, king 76 × 80 in"] }))["Bed, king 76 × 80 in"].clone();
    let beds = count(&app, "IfcFurniture");
    let added = ok(&mut app, "mesh_object.place", json!({ "type_id": bed, "point": [3.0, 12.0], "level": levels[0] }));
    ok(&mut app, "group.add_members", json!({ "element_ids": [added] }));
    ok(&mut app, "group.finish", json!({}));
    assert_eq!(count(&app, "IfcFurniture"), beds + 24);
    assert_eq!(walls(&app), 40);
    assert_eq!(ok(&mut app, "unit.list", json!({})).as_array().unwrap().iter().filter(|c| !c["unmerged"].as_array().unwrap().is_empty()).count(), 0);

    // From a member of a copy, and cancelled: everything as it was.
    let window = riviera_app_store(&app).iter().find(|e| e.class == "IfcWindow").map(|e| e.id()).unwrap();
    let before = riviera_app_store(&app).iter().count();
    ok(&mut app, "unit.edit", json!({ "element_id": window }));
    assert_eq!(walls(&app), 42);
    ok(&mut app, "group.cancel", json!({}));
    assert_eq!((walls(&app), riviera_app_store(&app).iter().count()), (40, before));

    // From the array.
    let ed = ok(&mut app, "unit.edit", json!({ "element_id": rep["array_id"] }));
    assert_eq!(ed["copies"], 24);
    ok(&mut app, "group.cancel", json!({}));
}
