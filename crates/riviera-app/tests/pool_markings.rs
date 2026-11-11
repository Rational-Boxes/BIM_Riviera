// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A pool's depth markings (ARC-POOL-011): where the markers go, their labels, NO DIVING, the
//! break-point band and float line, and the MAHC's, ISPSC's and HSG179's checks of them.

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

/// The findings whose rule mentions `what`.
fn findings(c: &Value, what: &str) -> Vec<String> {
    c["findings"].as_array().unwrap().iter().filter(|x| x["rule"].as_str().unwrap().contains(what)).map(|x| format!("{} {}", x["severity"].as_str().unwrap(), x["message"].as_str().unwrap())).collect()
}

#[test]
fn labels_round_to_three_inches_or_a_tenth_of_a_metre() {
    use riviera_core::pool_markings::{Units, label};
    assert_eq!(label(1.524, Units::Feet), "5 FT");
    assert_eq!(label(1.0229, Units::Feet), "3 FT 3 IN");
    assert_eq!(label(1.0229, Units::Both), "3 FT 3 IN / 1.0 M");
    assert_eq!(label(2.46, Units::Metres), "2.5 M");
}

#[test]
fn a_public_pool_marked_and_checked() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // 25 × 12.5 m: 1.0 m at the shallow end, 5 ft at 12 m, 2.5 m from 20 m.
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 12.5], [0.0, 0.0], [25.0, 0.0], [25.0, 12.5]],
        "profile": [{ "at": 0.0, "depth": 1.0 }, { "at": 8.0, "depth": 1.2 }, { "at": 12.0, "depth": 1.524 }, { "at": 20.0, "depth": 2.5 }, { "at": 25.0, "depth": 2.5 }] }));
    // Unmarked, the ISPSC (the default under the IBC) asks for markings and a break line.
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(findings(&c, "409").iter().any(|m| m.starts_with("error") && m.contains("no depth markings")), "{c:#}");
    assert!(findings(&c, "605").iter().any(|m| m.contains("break-point band")), "{c:#}");

    let m = ok(&mut app, "pool.depth_markings", json!({ "pool_id": pool }));
    let markers = m["markers"].as_array().unwrap();
    // Round the 75 m perimeter, never more than 25 ft apart.
    let along: Vec<f64> = markers.iter().map(|x| f(&x["along"])).collect();
    for k in 0..along.len() {
        let gap = if k + 1 < along.len() { along[k + 1] - along[k] } else { along[0] + 75.0 - along[k] };
        assert!(gap <= 7.62 + 1e-6 && gap >= 0.6 - 1e-6, "gap {gap} after {k}: {m:#}");
    }
    let reasons: Vec<&str> = markers.iter().map(|x| x["reason"].as_str().unwrap()).collect();
    for r in ["least", "greatest", "slope_change", "increment"] {
        assert!(reasons.contains(&r), "{r}: {reasons:?}");
    }
    // The 5 ft break (here a slope change too) is marked on both sides.
    for edge in [2, 4] {
        assert!(markers.iter().any(|x| x["edge"] == edge && near(f(&x["depth"]), 1.524, 0.01)), "edge {edge}: {m:#}");
    }
    // The shallow end reads its depth 3 ft out; NO DIVING exactly where it is 5 ft or less.
    let shallow = markers.iter().find(|x| x["edge"] == 1).unwrap();
    assert!(near(f(&shallow["depth"]), 1.0 + 0.2 * 0.9144 / 8.0, 1e-3) && shallow["label"] == "3 FT 3 IN", "{shallow:#}");
    for x in markers {
        assert_eq!(x["no_diving"].as_bool().unwrap(), f(&x["depth"]) <= 1.524 + 1e-6, "{x:#}");
        // Deck tiles within 18 in of the water's edge.
        let (p, q) = (&x["edge_point"], &x["at"]);
        assert!(near((f(&p[0]) - f(&q[0])).hypot(f(&p[1]) - f(&q[1])), 0.25, 1e-9));
    }
    // The band at the 5 ft break, 4 in wide on its shallow side; the float line 1 ft shallower.
    let l = &m["lines"];
    assert!(near(f(&l["band"][0]), 12.0, 1e-6) && near(f(&l["band"][1]), 12.0 - 0.1016, 1e-6) && near(f(&l["float_line"]), 12.0 - 0.1016 - 0.3048, 1e-6), "{l:#}");
    let i = ok(&mut app, "pool.info", json!({ "pool_id": pool }));
    assert_eq!(i["markers"].as_array().unwrap().len(), markers.len());
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(findings(&c, "409").is_empty() && findings(&c, "605").is_empty(), "{c:#}");
    // A ladder where the shallow end's marker stood: the marker steps aside along the wall.
    let ty = ok(&mut app, "furnishing.load", json!({ "names": ["Pool ladder, 3 steps"] }))["Pool ladder, 3 steps"].clone();
    let p = &shallow["edge_point"];
    ok(&mut app, "mesh_object.place", json!({ "type_id": ty, "point": [f(&p[0]), f(&p[1])], "level": level, "rotation": std::f64::consts::FRAC_PI_2 }));
    let i = ok(&mut app, "pool.info", json!({ "pool_id": pool }));
    let moved = i["markers"].as_array().unwrap().iter().find(|x| x["edge"] == 1).unwrap().clone();
    assert!((f(&moved["edge_point"][1]) - f(&p[1])).abs() > 0.3, "{moved:#}");
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    assert!(findings(&ok(&mut app, "pool.check", json!({})), "409").is_empty());

    // Steps of 1 m are too coarse for the ISPSC's 2 ft; in metres only, the MAHC objects.
    ok(&mut app, "pool.depth_markings", json!({ "pool_id": pool, "increment": 1.0 }));
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(findings(&c, "409").iter().any(|m| m.contains("every 1.00 m")), "{c:#}");
    ok(&mut app, "rules.set_packs", json!({ "domain": "pool", "packs": ["us_mahc_2024"] }));
    ok(&mut app, "pool.depth_markings", json!({ "pool_id": pool, "units": "metres", "increment": 0.3048, "no_diving": false }));
    let c = ok(&mut app, "pool.check", json!({}));
    let mahc = findings(&c, "4.5.19");
    assert!(mahc.iter().any(|m| m.contains("in metres")) && mahc.iter().any(|m| m.contains("no NO DIVING")), "{c:#}");
    // Spaced 40 ft apart, the gaps are too long.
    ok(&mut app, "pool.depth_markings", json!({ "pool_id": pool, "units": "both", "no_diving": true, "spacing": 12.2 }));
    let c = ok(&mut app, "pool.check", json!({}));
    let mahc = findings(&c, "4.5.19");
    assert!(!mahc.is_empty() && mahc.iter().all(|m| m.contains("apart round the perimeter")), "{c:#}");

    // Taken away, nothing is drawn or listed.
    ok(&mut app, "pool.depth_markings", json!({ "pool_id": pool, "remove": true }));
    let i = ok(&mut app, "pool.info", json!({ "pool_id": pool }));
    assert!(i.get("markers").is_none() && i.get("depth_lines").is_none(), "{i:#}");
}

#[test]
fn shallow_and_uk_pools() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    // A learner pool 0.9–1.2 m: no break line; under the UK code its markings are in metres.
    ok(&mut app, "code.set", json!({ "code": "uk_adk", "occupancy": "other" }));
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 7.0], [0.0, 0.0], [12.0, 0.0], [12.0, 7.0]], "profile": [{ "at": 0.0, "depth": 0.9 }, { "at": 12.0, "depth": 1.2 }] }));
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(findings(&c, "depth signs").iter().any(|m| m.starts_with("note") && m.contains("no depth markings")), "{c:#}");
    let m = ok(&mut app, "pool.depth_markings", json!({ "pool_id": pool }));
    assert!(m.get("lines").is_none(), "{m:#}");
    assert!(m["markers"].as_array().unwrap().iter().all(|x| x["label"].as_str().unwrap().ends_with(" M") && x["no_diving"] == true), "{m:#}");
    let c = ok(&mut app, "pool.check", json!({}));
    assert!(findings(&c, "depth signs").is_empty(), "{c:#}");
}
