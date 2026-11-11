// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Pool gates' hardware (ARC-POOL-016): self-closing, self-latching and lockable gates and their
//! latches' heights, against the ISPSC's public and residential rules and the MAHC.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

/// The gate findings: (severity, message).
fn gate(app: &mut App, rule: &str) -> Vec<(String, String)> {
    let c = ok(app, "pool.check", json!({}));
    c["findings"].as_array().unwrap().iter().filter(|x| x["rule"].as_str().unwrap().starts_with(rule)).map(|x| (x["severity"].as_str().unwrap().to_string(), x["message"].as_str().unwrap().to_string())).collect()
}

/// A pool of `class` fenced 1.25 m high (the MAHC's 6 ft for public ones) with an outward gate.
fn setup(class: &str) -> (tempfile::TempDir, App, Value) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "pool.create", json!({ "level": level, "class": class, "outline": [[0.0, 5.0], [0.0, 0.0], [10.0, 0.0], [10.0, 5.0]], "profile": [{ "at": 0.0, "depth": 1.0 }, { "at": 10.0, "depth": 1.4 }] }));
    let h = if class == "public" { 1.85 } else { 1.25 };
    let fence = ok(&mut app, "railing_type.create", json!({ "name": "Pool fence", "height": h, "bottom_rail_height": 0.05, "baluster": 0.025, "baluster_spacing": 0.12 }));
    let rail = ok(&mut app, "railing.create", json!({ "type_id": fence, "path": [[-2.0, -2.0], [12.0, -2.0], [12.0, 7.0], [-2.0, 7.0]], "closed": true, "base_level": level }));
    let g = ok(&mut app, "railing.add_gate", json!({ "railing_id": rail, "point": [5.0, -2.0], "swing_out": true }));
    (dir, app, g)
}

#[test]
fn a_public_pools_gate() {
    let (_dir, mut app, g) = setup("public");
    // Unknown hardware is a note.
    let f = gate(&mut app, "ISPSC 305.3.3");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(f[0].0 == "note" && f[0].1.contains("hardware is not set"), "{f:#?}");
    // The first hardware: self-closing, self-latching, lockable, a 54 in outside latch: inside
    // the ISPSC's public 52–54 in.
    let h = ok(&mut app, "railing.gate_hardware", json!({ "gate_id": g }));
    assert!(h["self_closing"] == true && h["latch"] == "outside" && (h["latch_height"].as_f64().unwrap() - 1.3716).abs() < 1e-9, "{h:#}");
    assert_eq!(gate(&mut app, "ISPSC 305.3.3"), Vec::<(String, String)>::new());
    // A 48 in latch, a gate that does not close itself, a key latch 50 in up.
    ok(&mut app, "railing.gate_hardware", json!({ "gate_id": g, "latch_height": 1.2192, "self_closing": false }));
    let f = gate(&mut app, "ISPSC 305.3.3");
    assert!(f.contains(&("error".into(), "the gate is not self-closing".into())) && f.contains(&("error".into(), "its latch is 48 in above the ground; 52–54 in".into())), "{f:#?}");
    ok(&mut app, "railing.gate_hardware", json!({ "gate_id": g, "latch": "key", "latch_height": 1.27, "self_closing": true }));
    assert_eq!(gate(&mut app, "ISPSC 305.3.3"), vec![("error".to_owned(), "its key latch is 50 in above the ground; 34–48 in".to_owned())]);
    // A pool-side latch is not for public pools; the MAHC wants 54 in at least.
    ok(&mut app, "railing.gate_hardware", json!({ "gate_id": g, "latch": "pool_side", "latch_height": 1.2 }));
    assert!(gate(&mut app, "ISPSC 305.3.3")[0].1.starts_with("its latch is released from the pool side only; us_ispsc wants it released outside, 52–54 in"));
    ok(&mut app, "rules.set_packs", json!({ "domain": "pool", "packs": ["us_mahc_2024"] }));
    ok(&mut app, "railing.gate_hardware", json!({ "gate_id": g, "latch": "outside", "latch_height": 1.3, "lockable": false }));
    let f = gate(&mut app, "MAHC 2024 4.8.6 (gate hardware)");
    assert!(f.contains(&("error".into(), "the gate is not lockable".into())) && f.contains(&("error".into(), "its latch is 51.2 in above the ground; at least 54 in".into())), "{f:#?}");
    // Taken away, it is unknown again.
    assert_eq!(ok(&mut app, "railing.gate_hardware", json!({ "gate_id": g, "remove": true })), Value::Null);
    assert_eq!(gate(&mut app, "MAHC 2024 4.8.6 (gate hardware)")[0].0, "note");
}

#[test]
fn a_backyard_gate_latched_on_the_pool_side() {
    let (_dir, mut app, g) = setup("residential");
    // Released from the pool side 0.95 m up on a 1.25 m gate: 11.8 in below its top, but the
    // balusters round it are 95 mm apart.
    ok(&mut app, "railing.gate_hardware", json!({ "gate_id": g, "latch": "pool_side", "latch_height": 0.95 }));
    assert_eq!(gate(&mut app, "ISPSC 305.3.3"), vec![("error".to_owned(), "nothing within 18 in of its pool-side latch may open more than ½ in (latch_mesh)".to_owned())]);
    ok(&mut app, "railing.gate_hardware", json!({ "gate_id": g, "latch_mesh": true }));
    assert_eq!(gate(&mut app, "ISPSC 305.3.3"), Vec::<(String, String)>::new());
    // At 1.2 m it is only 2 in below the top.
    ok(&mut app, "railing.gate_hardware", json!({ "gate_id": g, "latch_height": 1.2 }));
    assert_eq!(gate(&mut app, "ISPSC 305.3.3"), vec![("error".to_owned(), "its pool-side latch is 2 in below the gate's top; at least 3 in".to_owned())]);
    // Saved and opened again, the gate keeps its hardware.
    let model = _dir.path().join("gate.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let mut again = App::new(AppConfig { live: None, untitled_dir: _dir.path().join("v") });
    ok(&mut again, "file.open", json!({ "path": model }));
    let b = ok(&mut again, "pool.barriers", json!({}))[0].clone();
    let h = &b["gates"][0]["hardware"];
    assert!(h["latch"] == "pool_side" && h["latch_mesh"] == true && (h["latch_height"].as_f64().unwrap() - 1.2).abs() < 1e-9, "{b:#}");
    // An element that is not a gate is refused.
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    assert!(app.execute("railing.gate_hardware", json!({ "gate_id": level }), &Caller::user()).is_err());
}
