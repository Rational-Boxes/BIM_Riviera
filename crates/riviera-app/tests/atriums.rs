// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Atriums (ARC-GLZ-018): a void through a five-storey building's floors with guards on each
//! opened floor and a glazed roof, checked against IBC 404 and the NFPA 92B plume.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn store(app: &App) -> riviera_core::Store {
    app.document(app.active().unwrap()).unwrap().doc.store().clone()
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let p = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    json!([(0..4).map(|k| json!({ "line": { "start": p[k], "end": p[(k + 1) % 4] } })).collect::<Vec<_>>()])
}

/// Five levels 4 m apart, each with a 30 × 30 m floor.
fn building() -> (tempfile::TempDir, App, Vec<Value>) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let mut levels = vec![ok(&mut app, "level.list", json!({}))[0]["id"].clone()];
    for k in 1..5 {
        levels.push(ok(&mut app, "level.create", json!({ "elevation": 4.0 * k as f64, "name": format!("Level {}", k + 1) })));
    }
    for l in &levels {
        ok(&mut app, "floor.create", json!({ "loops": rect(0.0, 0.0, 30.0, 30.0), "level_id": l }));
    }
    (dir, app, levels)
}

#[test]
fn a_central_atrium_under_a_ridge_roof() {
    let (_dir, mut app, levels) = building();
    // A 10 × 12 m void from level 1 to the roof on level 5, its ridge roof 4.5 m up.
    let a = ok(&mut app, "atrium.create", json!({ "outline": [[10.0, 9.0], [20.0, 9.0], [20.0, 21.0], [10.0, 21.0]], "base_level": levels[0], "top_level": levels[4], "roof_offset": 4.5 }));
    assert_eq!(a["opened"], json!(["Level 2", "Level 3", "Level 4", "Level 5"]), "{a:#}");
    assert_eq!(a["guards"].as_array().unwrap().len(), 4);
    let st = store(&app);
    let roof = st.iter().find(|e| json!(e.id()) == a["roof"]).unwrap();
    assert!(riviera_core::roofs::glazing::of(&st, roof).is_some(), "the roof is glazed");
    let guard = st.iter().find(|e| json!(e.id()) == a["guards"][0]).unwrap();
    assert!(guard.class == "IfcRailing");
    // The check: five storeys, 20.5 m to the roof's plate.
    let r = ok(&mut app, "atrium.check", json!({}))[0].clone();
    assert_eq!(r["storeys"], 5);
    assert!((r["height"].as_f64().unwrap() - 20.5).abs() < 0.05, "{r:#}");
    assert!((r["area"].as_f64().unwrap() - 120.0).abs() < 1e-6);
    let rules: Vec<&str> = r["findings"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap()).collect();
    for want in ["IBC 404.5", "IBC 404.6", "IBC 404.3", "NFPA 92"] {
        assert!(rules.contains(&want), "{want}: {r:#}");
    }
    // The layer 6 ft over level 5 (17.83 m): 2.7 m deep, under 20 % of 20.5 m.
    let s = &r["smoke"];
    assert!((s["clear_height"].as_f64().unwrap() - 17.83).abs() < 0.01, "{s:#}");
    assert!(r["findings"].as_array().unwrap().iter().any(|f| f["rule"] == "NFPA 92B" && f["message"].as_str().unwrap().contains("under 20 %")), "{r:#}");
    // m = 0.071 · 3500^(1/3) · 17.83^(5/3) + 0.0018 · 3500.
    let m = 0.071 * 3500f64.powf(1.0 / 3.0) * 17.83f64.powf(5.0 / 3.0) + 0.0018 * 3500.0;
    assert!((s["mass_flow"].as_f64().unwrap() - m).abs() < 0.5, "{s:#}");
    // A roof on level 5 itself holds no layer over level 5: an error.
    let (_d2, mut low, lv) = building();
    ok(&mut low, "atrium.create", json!({ "outline": [[10.0, 9.0], [20.0, 9.0], [20.0, 21.0], [10.0, 21.0]], "base_level": lv[0], "top_level": lv[4], "form": "flat" }));
    let r = ok(&mut low, "atrium.check", json!({}))[0].clone();
    assert!(r["findings"].as_array().unwrap().iter().any(|f| f["severity"] == "error" && f["rule"] == "IBC 909 / NFPA 92"), "{r:#}");
}

#[test]
fn a_barrel_roof_reaching_over_a_roof_terrace() {
    let (_dir, mut app, levels) = building();
    // The void opens levels 2 and 3; the vault on level 3 runs on 8 m east over the terrace.
    let a = ok(&mut app, "atrium.create", json!({ "outline": [[10.0, 10.0], [18.0, 10.0], [18.0, 20.0], [10.0, 20.0]], "base_level": levels[0], "top_level": levels[2], "form": "barrel", "roof_outline": [[9.7, 9.7], [26.0, 9.7], [26.0, 20.3], [9.7, 20.3]], "roof_offset": 5.0 }));
    assert_eq!(a["opened"], json!(["Level 2", "Level 3"]));
    let r = ok(&mut app, "atrium.check", json!({ "atrium_id": a["atrium"] }))[0].clone();
    assert_eq!(r["storeys"], 3);
    assert!((r["roof_area"].as_f64().unwrap() - 16.3 * 10.6).abs() < 0.5, "{r:#}");
    // Three storeys: smoke control, but no separation finding.
    let rules: Vec<&str> = r["findings"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap()).collect();
    assert!(rules.contains(&"IBC 404.5") && !rules.contains(&"IBC 404.6"), "{r:#}");
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    // Saved and opened again, it is still an atrium.
    let model = _dir.path().join("atrium.ifc");
    ok(&mut app, "file.save_as", json!({ "path": model }));
    let mut again = App::new(AppConfig { live: None, untitled_dir: _dir.path().join("v") });
    ok(&mut again, "file.open", json!({ "path": model }));
    assert_eq!(ok(&mut again, "atrium.check", json!({})).as_array().unwrap().len(), 1);
}

#[test]
fn the_roofs_sides_closed_in_glass() {
    let (_dir, mut app, levels) = building();
    // The roof reaches 1 m past the void; four curtain walls 0.3 m inside its edges, attached to
    // the ridge roof so they rise to it.
    let a = ok(&mut app, "atrium.create", json!({ "outline": [[10.0, 9.0], [20.0, 9.0], [20.0, 21.0], [10.0, 21.0]], "base_level": levels[0], "top_level": levels[4], "roof_offset": 4.5, "overhang": 1.0, "walls": true }));
    let walls = a["walls"].as_array().unwrap();
    assert_eq!(walls.len(), 4, "{a:#}");
    let st = store(&app);
    for w in walls {
        let e = st.iter().find(|e| json!(e.id()) == *w).unwrap();
        assert_eq!(e.class, "IfcCurtainWall");
    }
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
}

#[test]
fn an_atrium_is_made_again_as_one() {
    let (_dir, mut app, levels) = building();
    let a = ok(&mut app, "atrium.create", json!({ "outline": [[10.0, 9.0], [20.0, 9.0], [20.0, 21.0], [10.0, 21.0]], "base_level": levels[0], "top_level": levels[4], "roof_offset": 4.5 }));
    // Picked by one of its guards: the void, four guards, the roof and the atrium space.
    let l = ok(&mut app, "layout.get", json!({ "element_id": a["guards"][0] }));
    assert_eq!(l["command"], "atrium.create");
    assert_eq!(l["elements"].as_array().unwrap().len(), 7, "{l:#}");
    // Opened to level 3 only, under a barrel: two guards, and the old parts gone.
    let r = ok(&mut app, "layout.redo", json!({ "element_id": a["roof"], "params": { "top_level": levels[2], "form": "barrel" } }));
    assert_eq!(r["removed"], 7);
    assert_eq!(r["result"]["opened"], json!(["Level 2", "Level 3"]));
    let st = store(&app);
    assert!(st.iter().all(|e| json!(e.id()) != a["roof"] && json!(e.id()) != a["atrium"]));
    assert_eq!(st.iter().filter(|e| e.class == "IfcRailing").count(), 2);
    // One guard type and one glazing type still: types are kept, not made twice.
    assert_eq!(st.iter().filter(|e| e.name.as_deref() == Some("Atrium glazing")).count(), 1);
    let r = ok(&mut app, "atrium.check", json!({}))[0].clone();
    assert_eq!(r["storeys"], 3, "{r:#}");
    ok(&mut app, "edit.undo", json!({}));
    assert_eq!(ok(&mut app, "atrium.check", json!({}))[0]["storeys"], 5);
}
