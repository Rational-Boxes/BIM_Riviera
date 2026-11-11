// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Handrails, recessed steps and beach entries (ARC-POOL-013): the rails on stairs and sloped
//! entries, treads cut into the wall with their grab rails, a zero-depth entry, the checks that
//! measure them, and their IFC.

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

/// The error findings, "rule: message", leaving out the barrier and markings rules.
fn errors(app: &mut App) -> Vec<String> {
    let c = ok(app, "pool.check", json!({}));
    c["findings"].as_array().unwrap().iter().filter(|f| f["severity"] == "error").map(|f| format!("{}: {}", f["rule"].as_str().unwrap(), f["message"].as_str().unwrap())).collect()
}

/// A public pool, 15 × 8 m, 1.0 m deep at its west wall (edge 1) to 2.0 m at its east wall
/// (edge 3), marked and fenced, under the MAHC, the ISPSC and the ADA.
fn setup() -> (tempfile::TempDir, App, Value) {
    setup_with(json!([{ "at": 0.0, "depth": 1.0 }, { "at": 7.0, "depth": 1.524 }, { "at": 9.0, "depth": 2.0 }]), true)
}

fn setup_with(profile: Value, ada: bool) -> (tempfile::TempDir, App, Value) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let mut packs = vec!["us_mahc_2024", "us_ispsc", "us_vgb"];
    if ada {
        ok(&mut app, "clearance.set_rule_set", json!({ "standard": "ada_2010" }));
        packs.push("ada_2010_pools");
    }
    ok(&mut app, "rules.set_packs", json!({ "domain": "pool", "packs": packs }));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    let p = [[-6.0, -6.0], [21.0, -6.0], [21.0, 14.0], [-6.0, 14.0]];
    let loops = json!([(0..4).map(|k| json!({ "line": { "start": p[k], "end": p[(k + 1) % 4] } })).collect::<Vec<_>>()]);
    ok(&mut app, "floor.create", json!({ "loops": loops, "level_id": level }));
    let pool = ok(&mut app, "pool.create", json!({ "level": level, "outline": [[0.0, 8.0], [0.0, 0.0], [15.0, 0.0], [15.0, 8.0]], "profile": profile }));
    ok(&mut app, "pool.depth_markings", json!({ "pool_id": pool }));
    let fence = ok(&mut app, "railing_type.create", json!({ "name": "Enclosure", "height": 1.85, "bottom_rail_height": 0.05, "baluster": 0.025, "baluster_spacing": 0.12 }));
    let rail = ok(&mut app, "railing.create", json!({ "type_id": fence, "path": [[-5.8, -5.8], [20.8, -5.8], [20.8, 13.8], [-5.8, 13.8]], "closed": true, "base_level": level }));
    ok(&mut app, "railing.add_gate", json!({ "railing_id": rail, "point": [5.0, -5.8], "swing_out": true }));
    (dir, app, pool)
}

#[test]
fn handrails_on_stairs_and_a_sloped_entry() {
    let (_dir, mut app, pool) = setup();
    // Stairs at the shallow end get a handrail at each side, 36 in above the nosings.
    let s = ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "stairs", "edge": 1, "at": 6.0 }));
    assert!(s["handrails"] == 2 && near(f(&s["handrail_height"]), 0.9144, 1e-9) && near(f(&s["handrail_gap"]), 1.5 - 0.06 - 0.048, 1e-9), "{s:#}");
    // The sloped entry's two, 35 in apart clear: inside the ADA's 33–38 in.
    let r = ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "sloped_entry", "edge": 1, "at": 2.0 }));
    assert!(r["handrails"] == 2 && near(f(&r["handrail_gap"]), 0.892, 1e-9), "{r:#}");
    // Recessed steps at the deep end make the second entry.
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "recessed_steps", "edge": 3, "at": 4.0 }));
    assert_eq!(errors(&mut app), Vec::<String>::new());
    let g = ok(&mut app, "pool.check", json!({}));
    assert_eq!(g["pools"][0]["entries"].as_u64(), Some(3), "{g:#}");
    // Without handrails, with one on stairs over 5 ft wide, too low, and the sloped entry's too
    // far apart.
    let entries = |stairs: Value, sloped: Value| {
        let mut s = json!({ "kind": "stairs", "edge": 1, "at": 6.0 });
        let mut r = json!({ "kind": "sloped_entry", "edge": 1, "at": 2.0 });
        s.as_object_mut().unwrap().extend(stairs.as_object().unwrap().clone());
        r.as_object_mut().unwrap().extend(sloped.as_object().unwrap().clone());
        json!([s, r, { "kind": "recessed_steps", "edge": 3, "at": 4.0 }])
    };
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "entries": entries(json!({ "handrails": 0 }), json!({ "handrails": 1 })) }));
    let e = errors(&mut app);
    assert!(e.contains(&"MAHC 2024 4.5.5: stairs 1 (edge 1) has 0 handrails; at least 1".to_owned()), "{e:#?}");
    assert!(e.contains(&"ADA 2010 1009.3.3: sloped entry 2 (edge 1) has 1 handrails; at least 2".to_owned()), "{e:#?}");
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "entries": entries(json!({ "width": 2.0, "at": 5.0, "handrails": 1, "handrail_height": 0.8 }), json!({ "handrail_gap": 1.0 })) }));
    let e = errors(&mut app);
    assert!(e.iter().any(|m| m.starts_with("MAHC 2024 4.5.5: stairs 1 (edge 1) is 6′ 7″ wide and has handrails on one side")), "{e:#?}");
    assert!(e.contains(&"MAHC 2024 Table 4.5.5.7: stairs 1 (edge 1): its handrails are 31.5 in above the nosings; 34–38 in".to_owned()), "{e:#?}");
    assert!(e.iter().any(|m| m.starts_with("ISPSC 2021 323.2 (public): stairs 1 (edge 1): its handrails are 31.5 in")), "{e:#?}");
    assert!(e.iter().any(|m| m.starts_with("ADA 2010 1009.3.3: sloped entry 2 (edge 1): its handrails are 39.4 in apart, clear; 33–38 in")), "{e:#?}");
    // Stairs 6 m wide: two handrails 5.9 m apart are too far; four are not.
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "entries": [{ "kind": "stairs", "edge": 1, "at": 5.0, "width": 6.0 }, { "kind": "recessed_steps", "edge": 3, "at": 4.0 }, { "kind": "sloped_entry", "edge": 1, "at": 1.0 }] }));
    let e = errors(&mut app);
    assert!(e.iter().any(|m| m.starts_with("MAHC 2024 4.5.5: stairs 1 (edge 1): its handrails are 19′")), "{e:#?}");
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "entries": [{ "kind": "stairs", "edge": 1, "at": 5.0, "width": 6.0, "handrails": 4 }, { "kind": "recessed_steps", "edge": 3, "at": 4.0 }, { "kind": "sloped_entry", "edge": 1, "at": 1.0 }] }));
    assert_eq!(errors(&mut app), Vec::<String>::new());
}

#[test]
fn recessed_steps_in_the_deep_wall() {
    let (_dir, mut app, pool) = setup();
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "stairs", "edge": 1, "at": 6.0 }));
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "sloped_entry", "edge": 1, "at": 2.0 }));
    let before = f(&ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"]);
    // In the east wall, 2.0 m deep: treads 10 in apart from 10 in below the deck down to 1.0 m
    // under the water (0.15 m below the deck): four of them.
    let r = ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "recessed_steps", "point": [15.0, 4.0] }));
    assert!(r["edge"] == 3 && r["risers"] == 4 && near(f(&r["riser"]), 0.254, 1e-9) && near(f(&r["recess"]), 0.14, 1e-9) && near(f(&r["width"]), 0.457, 1e-9), "{r:#}");
    assert!(r["handrails"] == 2 && near(f(&r["handrail_gap"]), 0.533, 1e-9) && near(f(&r["handrail_height"]), 0.85, 1e-9), "{r:#}");
    // The water fills the treads' openings: the top one's 0.104 m under the water, the rest 0.14 m.
    let after = f(&ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"]);
    assert!(near(after - before, 0.457 * 0.14 * (0.104 + 3.0 * 0.14), 1e-6), "{}", after - before);
    assert_eq!(errors(&mut app), Vec::<String>::new());
    // The top tread too low, too shallow, rails too far apart, too close together for the ISPSC.
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "entries": [{ "kind": "stairs", "edge": 1, "at": 6.0 }, { "kind": "sloped_entry", "edge": 1, "at": 2.0 }, { "kind": "recessed_steps", "edge": 3, "at": 4.0, "top": 0.4, "depth": 0.1, "riser": 0.165, "handrail_gap": 0.7 }] }));
    let e = errors(&mut app);
    for want in [
        "MAHC 2024 Table 4.5.7.4: recessed steps 3 (edge 3): its top tread is 15.75 in below the deck; at most 12 in",
        "MAHC 2024 Table 4.5.7.4: recessed steps 3 (edge 3): its treads go 3.9 in into the wall; at least 5 in",
        "MAHC 2024 4.5.6: recessed steps 3 (edge 3): its grab rails are 27.6 in apart, clear; 18–24 in",
        "ISPSC 2021 322.4 (public): recessed steps 3 (edge 3): its treads are 6.5 in apart; 7–12 in",
    ] {
        assert!(e.iter().any(|m| m.starts_with(want)), "{want}\n{e:#?}");
    }
    assert!(!e.iter().any(|m| m.starts_with("MAHC 2024 Table 4.5.7.4: recessed steps 3 (edge 3): its treads are 6.5")), "{e:#?}");
    // Deeper than the shell allows is refused.
    assert!(app.execute("pool.add_entry", json!({ "pool_id": pool, "kind": "recessed_steps", "edge": 3, "at": 2.0, "depth": 0.28 }), &Caller::user()).is_err());
}

#[test]
fn a_beach_entry() {
    // 0.9 m deep to 13 m out, then down to 1.05 m at the east wall; no ADA pack.
    let (_dir, mut app, pool) = setup_with(json!([{ "at": 0.0, "depth": 0.9 }, { "at": 13.0, "depth": 0.9 }, { "at": 15.0, "depth": 1.05 }]), false);
    let before = f(&ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"]);
    // Along the whole west wall at 1:12: the water's edge 1.8 m out (the deck is 0.15 m above
    // the water), the floor met 12.6 m out.
    let b = ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "beach", "edge": 1, "at": 1.0 }));
    assert!(near(f(&b["width"]), 8.0, 1e-9) && near(f(&b["at"][1]), 4.0, 1e-9) && near(f(&b["waterline"]), 1.8, 1e-9) && b["handrails"] == 0, "{b:#}");
    assert!(near(f(&b["run"]), 12.6, 0.011), "{b:#}");
    // It displaces the water over 1.8 m at full depth and a wedge over the next 10.8 m.
    let after = f(&ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"]);
    assert!(near(before - after, 8.0 * (1.8 * 0.9 + 0.5 * 10.8 * 0.9), 1e-5), "{}", before - after);
    // With recessed steps at the deep end it passes; at 1:9 it fails both packs.
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "recessed_steps", "edge": 3, "at": 4.0 }));
    let c = ok(&mut app, "pool.check", json!({}));
    assert_eq!(c["pools"][0]["entries"].as_u64(), Some(2), "{c:#}");
    assert_eq!(errors(&mut app), Vec::<String>::new());
    ok(&mut app, "pool.set", json!({ "pool_id": pool, "entries": [{ "kind": "beach", "edge": 1, "at": 4.0, "slope": 0.111111 }, { "kind": "recessed_steps", "edge": 3, "at": 4.0 }] }));
    let e = errors(&mut app);
    assert!(e.contains(&"MAHC 2024 4.5.9.2: beach entry 1 (edge 1) falls 1:9.0; at most 1:12.0".to_owned()), "{e:#?}");
    assert!(e.iter().any(|m| m.starts_with("ISPSC 2021 411.3 (public): beach entry 1 (edge 1) falls 1:9.0; at most 1:10.0")), "{e:#?}");
}

#[test]
fn handrails_in_ifc() {
    let (dir, mut app, pool) = setup();
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "stairs", "edge": 1, "at": 4.0 }));
    ok(&mut app, "pool.add_entry", json!({ "pool_id": pool, "kind": "recessed_steps", "edge": 3, "at": 4.0 }));
    let volume = f(&ok(&mut app, "pool.info", json!({ "pool_id": pool }))["volume"]);
    let model = dir.path().join("pool.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let rows = riviera_ifc::IfcModel::open(&model).unwrap().step_rows().unwrap();
    assert!(rows.iter().any(|r| r.class == "IFCRAILING" && r.attributes.contains(".HANDRAIL.")));
    let mut again = App::new(AppConfig { live: None, untitled_dir: dir.path().join("v") });
    ok(&mut again, "file.open", json!({ "path": model }));
    let id = ok(&mut again, "pool.list", json!({}))[0]["id"].clone();
    let i = ok(&mut again, "pool.info", json!({ "pool_id": id }));
    assert!(i["entries"][1]["kind"] == "recessed_steps" && near(f(&i["volume"]), volume, 1e-9), "{i:#}");
    let s = again.document(again.active().unwrap()).unwrap().doc.store().clone();
    // One railing: the enclosure; the handrails are the pool's parts.
    assert_eq!(s.iter().filter(|e| e.class == "IfcRailing").count(), 1);
}
