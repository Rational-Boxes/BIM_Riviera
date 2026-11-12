// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Sun-control baffles (ARC-GLZ-019): blades over a roof pool and patio, their cut-off, and the
//! share of the direct sun they let onto the water.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

/// A 10 × 5 m pool on level 1, at 25.8° N (Miami).
fn setup() -> (tempfile::TempDir, App, Value, Value) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    ok(&mut app, "location.set", json!({ "lat": 25.8, "lon": -80.2, "time_zone": -5.0, "place": "Miami" }));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 5.0], [0.0, 0.0], [10.0, 0.0], [10.0, 5.0]], "profile": [{ "at": 0.0, "depth": 1.2 }] }));
    (dir, app, level, pool)
}

fn share(app: &mut App, pool: &Value) -> Value {
    ok(app, "shading.sun_check", json!({ "elements": [pool] }))["areas"][0].clone()
}

#[test]
fn baffles_over_a_pool() {
    let (dir, mut app, level, pool) = setup();
    // Open to the sky: all the direct sun, over the 25 % allowed.
    let open = share(&mut app, &pool);
    assert!((f(&open["share"]) - 1.0).abs() < 1e-9 && open["passes"] == false, "{open:#}");
    // Vertical east–west blades 0.6 m apart and 0.4 m deep, 6 m up over the pool and its deck:
    // the cut-off is atan(0.575 / 0.4) = 55.2° from vertical each side.
    let region = json!([[-3.0, -3.0], [13.0, -3.0], [13.0, 8.0], [-3.0, 8.0]]);
    let b = ok(&mut app, "shading.baffles", json!({ "outline": region, "level": level, "height": 6.0 }));
    assert_eq!(b["blades"], 19, "{b:#}");
    assert!((f(&b["cutoff"][0]) - (0.575f64 / 0.4).atan().to_degrees()).abs() < 1e-6 && (f(&b["length"]) - 19.0 * 16.0).abs() < 1e-6, "{b:#}");
    // At 25.8° N the sun is high: vertical blades along the sun's path let most of it through.
    let vertical = share(&mut app, &pool);
    assert!(f(&vertical["share"]) < 1.0 && f(&vertical["share"]) > 0.25, "{vertical:#}");
    // Blades 0.3 m apart, 0.45 m deep and tilted 40°, over the whole 40 × 35 m roof terrace so
    // the low sun cannot slip under its edges: the high sun is cut.
    let dev = b["device"].clone();
    ok(&mut app, "element.delete", json!({ "element_ids": [dev] }));
    let terrace = json!([[-15.0, -15.0], [25.0, -15.0], [25.0, 20.0], [-15.0, 20.0]]);
    ok(&mut app, "shading.baffles", json!({ "outline": terrace, "level": level, "height": 6.0, "spacing": 0.3, "depth": 0.45, "tilt": "40°", "direction": "90°" }));
    let tilted = share(&mut app, &pool);
    assert!(f(&tilted["share"]) < f(&vertical["share"]) && tilted["passes"] == true, "{tilted:#}\n{vertical:#}");
    // Four days, the hours with the sun up.
    assert_eq!(tilted["days"].as_array().unwrap().len(), 4);
    assert!(tilted["days"][1]["hours"].as_array().unwrap().len() >= 12);
    // An area by outline beyond the baffles gets all the sun.
    let c = ok(&mut app, "shading.sun_check", json!({ "areas": [{ "name": "Lawn", "outline": [[100.0, 0.0], [105.0, 0.0], [105.0, 5.0], [100.0, 5.0]], "z": 0.0 }] }));
    assert!((f(&c["areas"][0]["share"]) - 1.0).abs() < 1e-9, "{c:#}");
    // Bad blades are refused.
    assert!(app.execute("shading.baffles", json!({ "outline": region, "level": level, "height": 6.0, "spacing": 0.02, "thickness": 0.025 }), &Caller::user()).is_err());
    // Saved, the baffles are a shading device with a body; nothing overlaps.
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    let model = dir.path().join("baffles.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let rows = riviera_ifc::IfcModel::open(&model).unwrap().step_rows().unwrap();
    assert!(rows.iter().any(|r| r.class == "IFCSHADINGDEVICE" && r.attributes.contains("'Baffle'")));
    let mut again = App::new(AppConfig { live: None, untitled_dir: dir.path().join("v") });
    ok(&mut again, "file.open", json!({ "path": model }));
    let p = ok(&mut again, "pool.list", json!({}))[0]["id"].clone();
    assert!((f(&share(&mut again, &p)["share"]) - f(&tilted["share"])).abs() < 1e-9);
}
