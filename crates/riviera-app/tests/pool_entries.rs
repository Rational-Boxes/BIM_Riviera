// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Pool entries (ARC-POOL-003): stairs, a sloped entry and a bench built into a pool, the water
//! they displace, the checks that count and measure them, and their IFC.

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

fn errors(c: &Value) -> Vec<String> {
    c["findings"].as_array().unwrap().iter().filter(|f| f["severity"] == "error").map(|f| format!("{}: {}", f["rule"].as_str().unwrap(), f["message"].as_str().unwrap())).collect()
}

#[test]
fn stairs_a_sloped_entry_and_a_bench() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    ok(&mut app, "clearance.set_rule_set", json!({ "standard": "ada_2010" }));
    ok(&mut app, "rules.set_packs", json!({ "domain": "pool", "packs": ["us_mahc_2024", "us_vgb", "ada_2010_pools"] }));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "floor.create", json!({ "loops": [square([-6.0, -6.0], [21.0, 14.0])], "level_id": level }));
    // 15 × 8 m, its shallow end the west wall (edge 1): 1.0 m to 5 ft over 7 m, to 2.0 m at 9 m.
    let profile = json!([{ "at": 0.0, "depth": 1.0 }, { "at": 7.0, "depth": 1.524 }, { "at": 9.0, "depth": 2.0 }]);
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 8.0], [0.0, 0.0], [15.0, 0.0], [15.0, 8.0]], "profile": profile }));
    let before = ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"].as_f64().unwrap();
    // Stairs by the west wall at y = 2: 1.15 m from deck to floor in five 0.23 m risers.
    let s = ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "stairs", "point": [0.1, 2.0] }));
    assert_eq!((s["index"].as_u64(), s["edge"].as_u64(), s["risers"].as_u64(), s["tread"].as_f64()), (Some(1), Some(1), Some(5), Some(0.305)), "{s:#}");
    assert!(near(s["riser"].as_f64().unwrap(), 0.23, 1e-9), "{s:#}");
    // The water they displace: four treads, each from the floor up to its top.
    let slope = 0.524 / 7.0;
    let displaced: f64 = (1..5).map(|k| {
        let x = (f64::from(k) - 0.5) * 0.305;
        (1.0 + slope * x - (0.23 * f64::from(k) - 0.15)) * 0.305 * 1.5
    }).sum();
    let after = ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"].as_f64().unwrap();
    assert!(near(before - after, displaced, 1e-6), "{} vs {displaced}", before - after);
    // A sloped entry from the west wall at y = 6, falling 1:12 to 27 in below the water.
    let r = ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "sloped_entry", "edge": 1, "at": 2.0 }));
    assert!(near(r["run"].as_f64().unwrap(), (0.15 + 0.6858) * 12.0, 1e-9) && near(r["reaches"].as_f64().unwrap(), 0.6858, 1e-9), "{r:#}");
    // A bench on the deep end's wall: the MAHC keeps benches to water 5 ft deep.
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "bench", "point": [14.9, 4.0] }));
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    let c = ok(&mut app, "pool.check", json!({}));
    let p = &c["pools"][0];
    // The stairs and the sloped entry are its two entries; the sloped entry its accessible one.
    assert_eq!((p["entries"].as_u64(), p["accessible_entries"].as_u64()), (Some(2), Some(1)), "{c:#}");
    assert_eq!(errors(&c), vec!["MAHC 2024 4.5.16: bench 3 (edge 3) stands in 6′ 7″ of water; at most 5 ft".to_owned()]);
    ok(&mut app, "pool.remove_entry", json!({ "pool_id": pool, "index": 3 }));
    assert_eq!(errors(&ok(&mut app, "pool.check", json!({}))), Vec::<String>::new());
    // Shallower treads fail the MAHC's 12 in; a steeper sloped entry fails the ADA's 1:12.
    let info = ok(&mut app, "pool.info", json!({ "pool_id": pool }));
    assert_eq!(info["entries"].as_array().unwrap().len(), 2);
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "entries": [
        { "kind": "stairs", "edge": 1, "at": 6.0, "tread": 0.25 },
        { "kind": "sloped_entry", "edge": 1, "at": 2.0, "slope": 0.1 },
    ] }));
    let e = errors(&ok(&mut app, "pool.check", json!({})));
    assert!(e.iter().any(|m| m.starts_with("MAHC 2024 Table 4.5.4.5: stairs 1 (edge 1): its treads are 9.8 in (250 mm); 12–18 in")), "{e:#?}");
    assert!(e.iter().any(|m| m.starts_with("ADA 2010 1009.3.1: sloped entry 2 (edge 1) falls 1:10.0")), "{e:#?}");
    // An entry off the pool's edges is refused.
    assert!(app.execute("pool.add_entry", json!({ "pool_id": pool, "kind": "bench", "edge": 5, "at": 1.0 }), &Caller::user()).is_err());
}

#[test]
fn entries_in_ifc() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "rectangle", "origin": [0.0, 0.0], "length": 12.0, "width": 6.0 } }));
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "stairs", "edge": 1, "at": 1.5 }));
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "bench", "edge": 2, "at": 3.0 }));
    let volume = ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"].as_f64().unwrap();
    let model = dir.path().join("pool.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let rows = riviera_ifc::IfcModel::open(&model).unwrap().step_rows().unwrap();
    assert!(rows.iter().any(|r| r.class == "IFCSTAIR" && r.attributes.contains(".STRAIGHT_RUN_STAIR.")));
    assert!(rows.iter().any(|r| r.class == "IFCBUILDINGELEMENTPROXY" && r.attributes.contains("'POOLBENCH'")));
    let mut again = App::new(AppConfig { live: None, untitled_dir: dir.path().join("v") });
    ok(&mut again, "file.open", json!({ "path": model }));
    let id = ok(&mut again, "pool.list", json!({}))[0]["id"].clone();
    let i = ok(&mut again, "pool.info", json!({ "pool_id": id }));
    assert_eq!(i["entries"].as_array().unwrap().len(), 2);
    assert!(near(i["volume"].as_f64().unwrap(), volume, 1e-9));
    let s = again.document(again.active().unwrap()).unwrap().doc.store().clone();
    assert!(!s.iter().any(|e| e.class == "IfcStair" || e.class == "IfcBuildingElementProxy"), "the entries are the pool's parts, not elements");
}
