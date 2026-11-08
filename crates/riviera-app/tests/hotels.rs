// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Hotel guest rooms (ARC-INT-002): the hotel templates furnish a room and mark it as a guest
//! room; the room matrix counts the guest rooms by floor and class and checks them against ADA
//! Tables 224.2 and 224.4, §224.5, and Approved Document M's one in twenty.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

/// Four 200 mm walls round a `w` × `d` inside from (x, 0); returns the south wall.
fn room(app: &mut App, level: &Value, x: f64, w: f64, d: f64) -> u64 {
    let t = 0.1;
    let mut walls = Vec::new();
    for (a, b) in [([x - t, -t], [x + w + t, -t]), ([x + w + t, -t], [x + w + t, d + t]), ([x + w + t, d + t], [x - t, d + t]), ([x - t, d + t], [x - t, -t])] {
        walls.push(ok(app, "wall.create", json!({ "curve": { "line": { "start": a, "end": b } }, "base_level": level, "top_constraint": { "unconnected": 2.7 } })).as_u64().unwrap());
    }
    walls[0]
}

fn setup() -> (tempfile::TempDir, App, Value) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "clearance.set_rule_set", json!({ "standard": "ada_2010" }));
    ok(&mut app, "rules.set_packs", json!({ "domain": "interiors", "packs": ["ada_2010_interiors", "practice_interiors"] }));
    (dir, app, level)
}

#[test]
fn hotel_templates_mark_their_guest_rooms() {
    let (_d, mut app, level) = setup();
    // A king room, 3.6 × 4.6 m: its seven pieces fit, the room is placed and marked, and the
    // standard room's bed has no ADA spaces.
    let south = room(&mut app, &level, 0.0, 3.6, 4.6);
    let king = ok(&mut app, "interior.layout", json!({ "template": "hotel_king", "point": [1.8, 2.3], "level": level, "wall_id": south }));
    assert_eq!(king["placed"].as_array().unwrap().len(), 7, "{king:#}");
    let king_room = king["guest_room"].clone();
    assert!(king_room.is_u64(), "{king:#}");
    let ada = |app: &mut App| ok(app, "clearance.list", json!({})).as_array().unwrap().iter().filter(|z| z["rule"].as_str().is_some_and(|r| r.starts_with("ADA 2010 806.2.3"))).count();
    assert_eq!(ada(&mut app), 0, "a standard room needs no bed spaces");

    // An accessible king, 3.8 × 5.0 m: its bed keeps both 30 × 48 in spaces.
    let acc_south = room(&mut app, &level, 5.0, 3.8, 5.0);
    let acc = ok(&mut app, "interior.layout", json!({ "template": "hotel_accessible_king", "point": [6.9, 2.5], "level": level, "wall_id": acc_south }));
    assert_eq!(acc["placed"].as_array().unwrap().len(), 7, "{acc:#}");
    assert_eq!(ada(&mut app), 2);
    // Two queens, 4.0 × 4.6 m.
    let south = room(&mut app, &level, 10.0, 4.0, 4.6);
    let dq = ok(&mut app, "interior.layout", json!({ "template": "hotel_double_queen", "point": [12.0, 2.3], "level": level, "wall_id": south }));
    assert_eq!(dq["placed"].as_array().unwrap().len(), 6, "{dq:#}");
    assert_eq!(ok(&mut app, "rules.check", json!({})), json!([]));
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));

    let rooms = ok(&mut app, "hotel.rooms", json!({}));
    let kinds: Vec<(&str, u64, &str)> = rooms.as_array().unwrap().iter().map(|r| (r["room_type"].as_str().unwrap(), r["beds"].as_u64().unwrap(), r["mobility"].as_str().unwrap())).collect();
    assert_eq!(kinds, vec![("king", 1, "none"), ("king", 1, "mobility"), ("double_queen", 2, "none")]);

    // Three rooms need one mobility room (Table 224.2) and two with communication features
    // (Table 224.4), and a mobility room with them (§224.5); the two should be in different
    // classes (a dispersion note).
    let m = ok(&mut app, "hotel.room_matrix", json!({}));
    assert_eq!((m["placed"].as_u64(), m["floors"][0]["rooms"].as_u64(), m["classes"].as_array().unwrap().len()), (Some(3), Some(3), 2), "{m:#}");
    let rules: Vec<&str> = m["findings"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap()).collect();
    assert_eq!(rules, vec!["ADA 2010 Table 224.4", "ADA 2010 224.5", "ADA 2010 224.5 dispersion"], "{m:#}");
    assert_eq!(m["findings"][0]["short"], 2);

    // The accessible room and one other with communication features: the accessible one does not
    // count toward the two (a tenth of one mobility room is none), so one is still short, and the
    // double-queen class still has none.
    ok(&mut app, "hotel.set_room", json!({ "room_ids": [acc["guest_room"], king_room], "communication": true }));
    let m = ok(&mut app, "hotel.room_matrix", json!({}));
    let rules: Vec<&str> = m["findings"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap()).collect();
    assert_eq!((m["ada"]["communication_counted"].as_u64(), rules, m["findings"][0]["short"].as_u64()), (Some(1), vec!["ADA 2010 Table 224.4", "ADA 2010 224.5 dispersion"], Some(1)), "{m:#}");
    ok(&mut app, "hotel.set_room", json!({ "room_ids": [dq["guest_room"]], "communication": true }));
    assert_eq!(ok(&mut app, "hotel.room_matrix", json!({}))["findings"], json!([]));

    // Counted as 60 rooms: four mobility rooms, one with a roll-in shower, seven with communication
    // features; the double-queen class has no mobility room (a note).
    let m = ok(&mut app, "hotel.room_matrix", json!({ "total": 60 }));
    let ada_counts = &m["ada"];
    assert_eq!((ada_counts["without_roll_in_required"].as_u64(), ada_counts["roll_in_required"].as_u64(), ada_counts["communication_required"].as_u64()), (Some(3), Some(1), Some(7)));
    let f: Vec<(&str, &str)> = m["findings"].as_array().unwrap().iter().map(|f| (f["rule"].as_str().unwrap(), f["severity"].as_str().unwrap())).collect();
    assert_eq!(f, vec![("ADA 2010 Table 224.2 roll-in", "error"), ("ADA 2010 Table 224.2", "error"), ("ADA 2010 Table 224.4", "error"), ("ADA 2010 224.5 dispersion", "note"), ("count basis", "note")], "{m:#}");
    // Under Approved Document M, one in twenty: three of 60, one of three.
    let uk = ok(&mut app, "hotel.room_matrix", json!({ "total": 60, "standards": ["uk_adm_m2"] }));
    assert_eq!((uk["uk"]["required"].as_u64(), uk["findings"][0]["short"].as_u64()), (Some(3), Some(2)), "{uk:#}");
    assert!(ok(&mut app, "hotel.room_matrix", json!({ "standards": ["uk_adm_m2"] }))["findings"].as_array().unwrap().is_empty());

    // A roll-in room laid out again with the accessible template stays roll-in.
    ok(&mut app, "hotel.set_room", json!({ "room_ids": [acc["guest_room"]], "mobility": "roll_in" }));
    let again = ok(&mut app, "interior.layout", json!({ "template": "hotel_accessible_king", "point": [6.9, 2.5], "level": level, "wall_id": acc_south, "replace": true }));
    assert_eq!(again["guest_room"], acc["guest_room"]);
    let rooms = ok(&mut app, "hotel.rooms", json!({}));
    assert_eq!((rooms.as_array().unwrap().len(), rooms[1]["mobility"].as_str(), rooms[1]["communication"].as_bool()), (3, Some("roll_in"), Some(true)));
    assert_eq!(ada(&mut app), 2, "replaced, not doubled");

    // Cleared, a room is no longer counted; a wall is not a room.
    ok(&mut app, "hotel.set_room", json!({ "room_ids": [dq["guest_room"]], "clear": true }));
    assert_eq!(ok(&mut app, "hotel.room_matrix", json!({}))["placed"], 2);
    let e = app.execute("hotel.set_room", json!({ "room_ids": [south] }), &Caller::user()).unwrap_err();
    assert_eq!(e.code, "hotel.not_a_room");
}
