// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A pool hall's air and water heat (ARC-POOL-008): ASHRAE's evaporation, its latent load, the
//! water's losses, ASHRAE 62.1's outdoor air as the MAHC adopts it, and the pool's latent load in
//! the design loads of the space that holds it.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn near(a: &Value, b: f64, tol: f64) -> bool {
    a.as_f64().is_some_and(|a| (a - b).abs() <= tol)
}

#[test]
fn a_25_m_pool_hall() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // The hall: four walls round a 31 × 18.5 m floor, and an MEP space in it.
    let (x0, y0, x1, y1) = (-3.0, -3.0, 28.0, 15.5);
    for (a, b) in [([x0, y0], [x1, y0]), ([x1, y0], [x1, y1]), ([x1, y1], [x0, y1]), ([x0, y1], [x0, y0])] {
        ok(&mut app, "wall.create", json!({ "curve": { "line": { "start": a, "end": b } }, "base_level": level, "top_constraint": { "unconnected": 6.0 } }));
    }
    let space = ok(&mut app, "space.place", json!({ "level_id": level, "point": [12.0, 6.0] }));
    // 25 × 12.5 m: water 28 °C, air 29 °C at 55 % (research/23 C.1.1's worked check), a cover.
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 12.5], [0.0, 0.0], [25.0, 0.0], [25.0, 12.5]],
        "hall": { "water": 28.0, "air": 29.0, "rh": 0.55, "cover": 0.1, "spectators": 50 } }));
    let n = ok(&mut app, "pool.natatorium", json!({ "pool_id": pool, "outdoor_dew_point": 18.0 }))[0].clone();
    // About 71 kg/h in use (AF 1.0), half that unoccupied, a tenth of that covered.
    let m = 4.0e-5 * 312.5 * (3.773519 - 0.55 * 3.999235);
    assert!(near(&n["evaporation"], m * 3600.0, 0.05) && near(&n["evaporation_unoccupied"], m * 1800.0, 0.05) && near(&n["evaporation_covered"], m * 180.0, 0.01), "{n:#}");
    assert!(near(&n["evaporation"], 70.8, 0.1), "{n:#}");
    assert!(near(&n["latent_load"], m * 2.43e6, 50.0), "{n:#}");
    assert!(near(&n["dew_point"], 19.05, 0.05), "{n:#}");
    // 2.44 L/s a m² of water and 1.5 m of wet deck (121.5 m²), and 3.8 L/s for each of 50
    // spectators; a purge of twice that; 5 % more exhausted.
    assert!(near(&n["wet_deck_area"], 121.5, 1e-6), "{n:#}");
    let oa = 2.44 * 434.0 + 3.8 * 50.0;
    assert!(near(&n["outdoor_air"], oa, 1e-6) && near(&n["purge"], 2.0 * oa, 1e-6) && near(&n["exhaust"], 1.05 * oa, 1e-6), "{n:#}");
    // The air a degree warmer: the water gains by convection; its losses are evaporation and
    // heating the make-up water from 10 °C.
    assert!(n["water_heat"]["convection"].as_f64().unwrap() < 0.0, "{n:#}");
    assert!(near(&n["water_heat"]["make_up"], m * 4186.0 * 18.0, 10.0), "{n:#}");
    // At an 18 °C outdoor dew point, outdoor air alone would take far more than the minimum.
    assert!(n["outdoor_air_for_moisture"].as_f64().unwrap() > 10_000.0, "{n:#}");
    assert!(n["notes"].as_array().unwrap().iter().any(|t| t.as_str().unwrap().contains("dehumidify mechanically")), "{n:#}");
    // The hall is the space; its volume gives the air changes.
    assert_eq!(n["hall"], space, "{n:#}");
    let ach = n["air_changes"].as_f64().unwrap();
    assert!(near(&n["air_changes"], oa / 1000.0 * 3600.0 / n["hall_volume"].as_f64().unwrap(), 1e-9) && ach > 0.5, "{n:#}");
    // The design loads carry the pool's latent load into the space: in use and out of hours.
    let store = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    let (spaces, map) = riviera_core::hvac_loads::spaces(&store);
    let guid = store.get(riviera_core::ElementId(space.as_u64().unwrap())).unwrap().guid().to_string();
    let s = spaces.iter().find(|s| map.get(&s.key) == Some(&guid)).expect("the hall's space");
    assert!((s.pool_latent[0] - m * 2.43e6).abs() < 50.0 && (s.pool_latent[1] - m * 2.43e6 * 0.05).abs() < 10.0, "{:?}", s.pool_latent);
    // Defaults by class: a competition pool's water 27 °C, air 28 °C, activity 0.65.
    let comp = ok(&mut app, "pool.create", json!({ "level": level, "template": { "kind": "competition_25m", "origin": [12.5, 40.0] } }));
    let c = ok(&mut app, "pool.natatorium", json!({ "pool_id": comp }))[0]["conditions"].clone();
    assert_eq!((c["water"].as_f64(), c["air"].as_f64(), c["activity"].as_f64()), (Some(27.0), Some(28.0), Some(0.65)), "{c:#}");
}
