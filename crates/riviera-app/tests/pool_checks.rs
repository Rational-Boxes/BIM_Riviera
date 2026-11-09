// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Swimming-pool checks (ARC-POOL-002): the pool packs chosen by the code and rule-set settings,
//! the floor's slopes, the deck, entries, accessible means of entry and main drains.

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

/// The findings' (pack, rule) pairs, with their messages for failures.
fn rules(c: &Value) -> Vec<(String, String)> {
    c["findings"].as_array().unwrap().iter().map(|f| (f["pack"].as_str().unwrap().to_owned(), f["rule"].as_str().unwrap().to_owned())).collect()
}

fn has(c: &Value, pack: &str, rule: &str) -> bool {
    rules(c).iter().any(|(p, r)| p == pack && r == rule)
}

/// A *Riviera Pools* item (or a bathroom component) placed, turned by `turn` radians.
fn place(app: &mut App, ty: &Value, level: &Value, at: [f64; 2], offset: f64, turn: f64) -> Value {
    let r = ok(app, "mesh_object.place", json!({ "type_id": ty, "point": at, "level": level, "offset": offset, "rotation": turn }));
    if r.is_array() { r[0].clone() } else { r }
}

#[test]
fn a_public_pool_is_checked() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    ok(&mut app, "clearance.set_rule_set", json!({ "standard": "ada_2010" }));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "floor.create", json!({ "loops": [square([-6.0, -6.0], [21.0, 14.0])], "level_id": level }));
    // 15 × 8 m: 1.0 m to 5 ft over 5 m (1:9.5), then to 2.0 m over 2 m.
    let profile = json!([{ "at": 0.0, "depth": 1.0 }, { "at": 5.0, "depth": 1.524 }, { "at": 7.0, "depth": 2.0 }]);
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 8.0], [0.0, 0.0], [15.0, 0.0], [15.0, 8.0]], "profile": profile }));
    // Under the IBC with the ADA rule set: the ISPSC, the VGB Act and the ADA's pool rules.
    let c = ok(&mut app, "pool.check", json!({}));
    assert_eq!(c["packs"], json!(["us_ispsc", "us_vgb", "ada_2010_pools"]), "{c:#}");
    assert!(has(&c, "us_ispsc", "ISPSC 2021 (public)"), "{c:#}");
    let slope = c["findings"].as_array().unwrap().iter().find(|f| f["message"].as_str().unwrap().contains("1:9.5")).expect("the shallow slope");
    assert_eq!((slope["severity"].as_str(), slope["confidence"].as_str()), (Some("error"), Some("secondary")));
    // No entries; one accessible means of entry needed (150 ft of wall, under 300); no drains yet.
    assert!(c["findings"].as_array().unwrap().iter().any(|f| f["rule"] == "ADA 2010 242.2" && f["message"].as_str().unwrap().contains("needs 1")), "{c:#}");
    assert!(c["findings"].as_array().unwrap().iter().any(|f| f["pack"] == "us_vgb" && f["severity"] == "note"), "{c:#}");
    let p = &c["pools"][0];
    assert_eq!((p["class"].as_str(), p["entries"].as_u64(), p["loads"][0]["bathers"].as_u64()), (Some("public"), Some(0), Some(25)), "{p:#}");
    // A gentler shallow slope, ladders at each end, a lift, and two drains too close together.
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "profile": [{ "at": 0.0, "depth": 1.0 }, { "at": 6.0, "depth": 1.524 }, { "at": 8.0, "depth": 2.0 }] }));
    let types = ok(&mut app, "furnishing.load", json!({ "names": ["Pool ladder, 3 steps", "Pool lift, 300 lb", "Main drain cover 300 × 300", "Main drain cover, unblockable 610 × 760"] }));
    let bench = ok(&mut app, "bath.load_components", json!({ "names": ["Steam room bench 600"] }))["Steam room bench 600"].clone();
    // The ladders' insertion points are the north wall's face; the lift on the south deck faces north.
    place(&mut app, &types["Pool ladder, 3 steps"], &level, [1.0, 8.0], 0.0, 0.0);
    place(&mut app, &types["Pool ladder, 3 steps"], &level, [14.0, 8.0], 0.0, 0.0);
    place(&mut app, &types["Pool lift, 300 lb"], &level, [3.0, -0.6], 0.0, std::f64::consts::PI);
    let d1 = place(&mut app, &types["Main drain cover 300 × 300"], &level, [11.0, 4.0], -2.15, 0.0);
    place(&mut app, &types["Main drain cover 300 × 300"], &level, [11.5, 4.0], -2.15, 0.0);
    let c = ok(&mut app, "pool.check", json!({}));
    let p = &c["pools"][0];
    assert_eq!((p["entries"].as_u64(), p["accessible_entries"].as_u64(), p["drains"].as_u64()), (Some(2), Some(1), Some(2)), "{c:#}");
    assert_eq!(rules(&c), vec![("us_vgb".to_owned(), "VGB 15 USC 8003(c); 16 CFR 1450".to_owned())], "{c:#}");
    assert!(c["findings"][0]["message"].as_str().unwrap().contains("0.50 m apart"), "{c:#}");
    // One unblockable drain instead passes; a bench on the deck blocks it.
    ok(&mut app, "element.delete", json!({ "element_ids": [d1] }));
    place(&mut app, &types["Main drain cover, unblockable 610 × 760"], &level, [11.0, 2.0], -2.15, 0.0);
    assert_eq!(rules(&ok(&mut app, "pool.check", json!({}))), Vec::<(String, String)>::new());
    let bench = place(&mut app, &bench, &level, [7.0, -1.0], 0.0, 0.0);
    let c = ok(&mut app, "pool.check", json!({}));
    assert_eq!(c["findings"][0]["element"], bench, "{c:#}");
    assert!(c["findings"][0]["message"].as_str().unwrap().contains("6 ft"), "{c:#}");
    ok(&mut app, "element.delete", json!({ "element_ids": [bench] }));
    // The MAHC, chosen for the project: its 1:12 shallow slope and its 20 ft² a bather.
    ok(&mut app, "rules.set_packs", json!({ "domain": "pool", "packs": ["us_mahc_2024", "us_vgb", "ada_2010_pools"] }));
    let c = ok(&mut app, "pool.check", json!({ "pool_id": pool }));
    assert!(has(&c, "us_mahc_2024", "MAHC 2024 4.5.2.1"), "{c:#}");
    assert!(c["findings"].as_array().unwrap().iter().any(|f| f["message"].as_str().unwrap().contains("1:11.5")), "{c:#}");
    assert_eq!(c["pools"][0]["loads"][0]["bathers"].as_u64(), Some(64));
}

#[test]
fn wading_and_residential_pools_have_their_own_rules() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    ok(&mut app, "clearance.set_rule_set", json!({ "standard": "ada_2010" }));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "floor.create", json!({ "loops": [square([-10.0, -10.0], [30.0, 10.0])], "level_id": level }));
    // A wading pool: 0.45 m is under the ISPSC's 18 in; it needs a sloped entry under the ADA.
    let wading = ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "wading", "origin": [0.0, 0.0] } }));
    let c = ok(&mut app, "pool.check", json!({ "pool_id": wading }));
    assert_eq!(c["pools"][0]["class"], "wading");
    assert!(has(&c, "ada_2010_pools", "ADA 2010 242.3") && !rules(&c).iter().any(|(_, r)| r == "ISPSC 2021 (public)"), "{c:#}");
    ok(&mut app, "pool.set", json!({ "pool_id": wading, "profile": [{ "at": 0.0, "depth": 0.6 }] }));
    let c = ok(&mut app, "pool.check", json!({ "pool_id": wading }));
    assert!(c["findings"].as_array().unwrap().iter().any(|f| f["message"].as_str().unwrap().contains("at most 1′ 6″ for a wading pool")), "{c:#}");
    // A residential pool: a 1:6 shallow slope is over the ISPSC's 1:7; the public rules do not apply.
    let home = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[12.0, 3.0], [12.0, -1.0], [22.0, -1.0], [22.0, 3.0]], "class": "residential", "profile": [{ "at": 0.0, "depth": 1.0 }, { "at": 3.0, "depth": 1.5 }] }));
    let c = ok(&mut app, "pool.check", json!({ "pool_id": home }));
    assert_eq!(rules(&c), vec![("us_ispsc".to_owned(), "ISPSC 2021 807.1.1".to_owned())], "{c:#}");
}

#[test]
fn pool_equipment_keeps_its_deck_clear() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    ok(&mut app, "clearance.set_rule_set", json!({ "standard": "ada_2010" }));
    ok(&mut app, "rules.set_packs", json!({ "domain": "pool", "packs": ["us_mahc_2024", "us_vgb", "ada_2010_pools"] }));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "floor.create", json!({ "loops": [square([-6.0, -6.0], [21.0, 14.0])], "level_id": level }));
    let profile = json!([{ "at": 0.0, "depth": 1.0 }, { "at": 7.0, "depth": 1.524 }, { "at": 9.0, "depth": 2.0 }]);
    ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 8.0], [0.0, 0.0], [15.0, 0.0], [15.0, 8.0]], "profile": profile }));
    let types = ok(&mut app, "furnishing.load", json!({ "names": ["Pool lift, 300 lb", "Lifeguard chair 6 ft", "Starting platform 24 × 32 in"] }));
    // A lift on the south deck facing the water, a lifeguard chair on the north deck, and a block
    // at the deep end: on the 4 ft deck by design, so the deck stays clear.
    let lift = place(&mut app, &types["Pool lift, 300 lb"], &level, [3.0, -0.6], 0.0, std::f64::consts::PI);
    place(&mut app, &types["Lifeguard chair 6 ft"], &level, [7.0, 9.2], 0.0, 0.0);
    place(&mut app, &types["Starting platform 24 × 32 in"], &level, [15.5, 4.0], 0.0, std::f64::consts::FRAC_PI_2);
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(!c["findings"].as_array().unwrap().iter().any(|f| f["message"].as_str().unwrap().contains("stands in")), "{c:#}");
    assert_eq!(ok(&mut app, "rules.check", json!({})), json!([]), "the zones are clear");
    // A bench in the lift's clear deck: the ADA's 36 × 48 in, and the MAHC's 4 ft.
    let bench = ok(&mut app, "bath.load_components", json!({ "names": ["Steam room bench 600"] }))["Steam room bench 600"].clone();
    let b = place(&mut app, &bench, &level, [3.0, -1.6], 0.0, 0.0);
    let r = ok(&mut app, "rules.check", json!({}));
    let mut hits: Vec<(String, String)> = r.as_array().unwrap().iter().filter(|f| f["element"] == lift && f["other"] == b).map(|f| (f["pack"].as_str().unwrap().to_owned(), f["rule"].as_str().unwrap().to_owned())).collect();
    hits.sort();
    assert_eq!(hits, vec![("ada_2010_pools".to_owned(), "ADA 2010 1009.2.3".to_owned()), ("us_mahc_2024".to_owned(), "MAHC 2024 4.8.1.5 (ADA equipment)".to_owned())], "{r:#}");
}
