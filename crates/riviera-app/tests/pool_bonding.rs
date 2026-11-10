// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A pool's electrical zones and bonding (ARC-POOL-010): NEC Article 680's distances and
//! heights, its bonding grid, and IEC 60364-7-702's zones 0, 1 and 2.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// The findings on `element`: (rule, severity).
fn on(b: &Value, element: &Value) -> Vec<(String, String)> {
    b["findings"].as_array().unwrap().iter().filter(|x| &x["element"] == element).map(|x| (x["rule"].as_str().unwrap().to_string(), x["severity"].as_str().unwrap().to_string())).collect()
}

fn has(b: &Value, element: &Value, rule: &str, severity: &str) -> bool {
    on(b, element).iter().any(|(r, s)| r == rule && s == severity)
}

#[test]
fn devices_round_an_outdoor_pool() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // 10 × 5 m, its water 150 mm below the deck at the level.
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 5.0], [0.0, 0.0], [10.0, 0.0], [10.0, 5.0]], "profile": [{ "at": 0.0, "depth": 1.2 }, { "at": 10.0, "depth": 2.0 }] }));
    let t = ok(&mut app, "mep_type.load_starters", json!({ "names": ["Receptacle", "Light Switch", "Ceiling Luminaire"] }));
    let device = |app: &mut App, name: &str, p: [f64; 2], offset: f64| ok(app, "device.place", json!({ "type_id": t[name], "point": p, "level": level, "offset": offset, "work_plane": true }));
    let close = device(&mut app, "Receptacle", [5.0, 6.0], 0.3);
    let gfci = device(&mut app, "Receptacle", [5.0, 8.0], 0.3);
    let switch = device(&mut app, "Light Switch", [11.0, 2.5], 1.1);
    let lamp = ok(&mut app, "lighting_fixture.place", json!({ "type_id": t["Ceiling Luminaire"], "point": [5.0, 2.5], "level": level, "offset": 3.0, "work_plane": true }));
    // Two underwater lights in the east wall: the library's (its lens 473 mm down) and one
    // set 200 mm higher.
    let uw = ok(&mut app, "furnishing.load", json!({ "names": ["Underwater light, large niche"] }))["Underwater light, large niche"].clone();
    let deep = ok(&mut app, "mesh_object.place", json!({ "type_id": uw, "point": [10.0, 1.5], "level": level, "rotation": -std::f64::consts::FRAC_PI_2 }));
    let high = ok(&mut app, "mesh_object.place", json!({ "type_id": uw, "point": [10.0, 3.5], "level": level, "rotation": -std::f64::consts::FRAC_PI_2, "offset": 0.2 }));

    let b = ok(&mut app, "pool.bonding", json!({}))[0].clone();
    assert_eq!((b["basis"].as_str(), &b["indoor"]), (Some("nec"), &json!(false)), "{b:#}");
    assert!(has(&b, &close, "receptacle_distance", "error"), "{b:#}");
    assert!(has(&b, &gfci, "receptacle_gfci", "note") && !has(&b, &gfci, "receptacle_distance", "error"), "{b:#}");
    assert!(has(&b, &switch, "switch_distance", "error"), "{b:#}");
    // 3.15 m over the water outdoors: under 12 ft.
    assert!(has(&b, &lamp, "luminaire_height", "error"), "{b:#}");
    assert!(on(&b, &deep).is_empty() && has(&b, &high, "underwater_lens_depth", "error"), "{b:#}");
    let d = b["devices"].as_array().unwrap().iter().find(|d| d["element"] == close).unwrap();
    assert!(near(f(&d["distance"]), 1.0, 0.03) && d["kind"] == "receptacle", "{d:#}");
    // The bonding grid: the perimeter surface 3 ft wide, four ties, the lights to bond.
    let g = &b["grid"];
    let w = 3.0 * 0.3048;
    assert!(near(f(&g["perimeter_surface"]), (10.0 + 2.0 * w) * (5.0 + 2.0 * w) - 50.0, 1e-6), "{g:#}");
    assert!(g["ties"].as_array().unwrap().len() == 4 && near(f(&g["ties"][1][0]), 2.5, 1e-9) && near(f(&g["ties"][1][1]), 0.0, 1e-9), "{g:#}");
    let bonded: Vec<&Value> = b["bond"].as_array().unwrap().iter().map(|x| &x["element"]).collect();
    assert!(bonded.contains(&&deep) && bonded.contains(&&high), "{b:#}");

    // Under the UK code setting, IEC 60364-7-702's zones.
    ok(&mut app, "code.set", json!({ "code": "uk_adk", "occupancy": "other" }));
    let b = ok(&mut app, "pool.bonding", json!({ "pool_id": pool }))[0].clone();
    assert_eq!(b["basis"].as_str(), Some("iec"));
    let zone = |e: &Value| b["devices"].as_array().unwrap().iter().find(|d| &d["element"] == e).map(|d| d["zone"].clone());
    assert_eq!((zone(&close), zone(&gfci), zone(&deep), zone(&lamp)), (Some(json!(1)), Some(json!(2)), Some(json!(0)), Some(Value::Null)), "{b:#}");
    assert!(has(&b, &close, "zone_switches", "error") && has(&b, &switch, "zone_switches", "error"), "{b:#}");
    assert!(has(&b, &gfci, "zone2_protection", "note") && has(&b, &deep, "zone0_ip", "note"), "{b:#}");
    assert!(on(&b, &lamp).is_empty() && b.get("grid").is_none(), "{b:#}");
    assert_eq!(b["zones"].as_array().unwrap().iter().map(|z| f(&z["distance"])).collect::<Vec<_>>(), vec![2.0, 3.5]);
}
