// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Ramps (ARC-RAMP-001) and Measure (ARC-MOD-023).

use riviera_core::{Caller, CommandRegistry, Document, ElementId};
use riviera_ifc::{IfcModel, Schema};
use riviera_store::LocalProject;
use serde_json::{Value, json};

fn registry() -> CommandRegistry {
    let mut r = CommandRegistry::with_core();
    riviera_store::commands::register(&mut r);
    r
}

#[test]
fn a_ramp_with_landings_its_warning_ifc_and_measures() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("r.ifc");
    let (mut project, mut doc) = LocalProject::new(Schema::Ifc4, &path).unwrap();
    let r = registry();
    let exec =
        |doc: &mut Document, name: &str, args: Value| r.execute(doc, name, args, &Caller::user());
    let l1 = doc
        .store()
        .iter()
        .find(|e| e.class == "IfcBuildingStorey")
        .unwrap()
        .id();
    // 0.6 m at 1:12 needs 7.2 m: drawn so, with a 6 m maximum incline, it gets one landing.
    let out = exec(
        &mut doc,
        "ramp.create",
        json!({
            "path": [[0, 0], [7.2, 0]], "base_level": l1.0,
            "top_constraint": { "unconnected": 0.6 },
        }),
    )
    .unwrap();
    assert!(out.warnings.is_empty());
    let ramp = ElementId(out.result.as_u64().unwrap());
    let t = doc
        .store()
        .iter()
        .find(|e| riviera_core::ramps::ramp_type(e).is_some())
        .unwrap()
        .id();
    exec(
        &mut doc,
        "type.set_param",
        json!({ "type_id": t.0, "param": "max_incline_length", "value": "6 m" }),
    )
    .unwrap();
    let g = riviera_core::ramps::regenerate(doc.store(), doc.store().get(ramp).unwrap()).unwrap();
    assert_eq!(g.pieces.iter().filter(|p| p.landing).count(), 1);
    assert!(!g.too_steep());
    // Drawn too short, it is still made, with a warning.
    let out = exec(
        &mut doc,
        "ramp.set",
        json!({ "ramp_ids": [ramp.0], "path": [[0, 0], [5, 0]] }),
    )
    .unwrap();
    assert_eq!(out.warnings[0].id, "arc.ramp.too_steep");
    // Measured along its path, and between two points.
    let along = exec(&mut doc, "measure.along", json!({ "element_id": ramp.0 })).unwrap();
    assert_eq!(along.result, json!(5.0));
    let between = exec(
        &mut doc,
        "measure.between",
        json!({ "a": [0, 0], "b": [3, 4] }),
    )
    .unwrap();
    assert_eq!(between.result["distance"], json!(5.0));

    project.save(&doc).unwrap();
    let rows = IfcModel::open(&path).unwrap().step_rows().unwrap();
    assert_eq!(rows.iter().filter(|r| r.class == "IFCRAMP").count(), 1);
    assert_eq!(rows.iter().filter(|r| r.class == "IFCRAMPTYPE").count(), 1);
}

#[test]
fn a_ramp_gets_railings_that_follow_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("r.ifc");
    let (mut project, mut doc) = LocalProject::new(Schema::Ifc4, &path).unwrap();
    let r = registry();
    let exec = |doc: &mut Document, name: &str, args: Value| r.execute(doc, name, args, &Caller::user()).unwrap().result;
    let l1 = doc.store().iter().find(|e| e.class == "IfcBuildingStorey").unwrap().id();
    // An L-shaped ramp turning left, 0.6 m up: railings on both sides (it rises over 150 mm).
    let ramp = exec(&mut doc, "ramp.create", json!({ "path": [[0, 0], [6, 0], [6, 6]], "base_level": l1.0, "top_constraint": { "unconnected": 0.6 } }));
    let rails = |doc: &Document| -> Vec<(bool, Vec<[f64; 3]>)> {
        let s = doc.store();
        s.iter()
            .filter_map(|e| {
                let rr = riviera_core::railings::railing_recipe(e)?;
                let riviera_core::railings::RailPath::Stair { left, .. } = rr.path else { return None };
                let (pts, _, _) = riviera_core::railings::resolve_path(s, &rr).ok()?;
                let z = riviera_core::railings::path_heights(s, &rr).ok()?;
                Some((left, pts.iter().zip(z).map(|(p, z)| [p[0], p[1], z]).collect()))
            })
            .collect()
    };
    let rs = rails(&doc);
    assert_eq!(rs.len(), 2);
    let side = |rs: &[(bool, Vec<[f64; 3]>)], left: bool| rs.iter().find(|r| r.0 == left).unwrap().1.clone();
    let near = |p: [f64; 3], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-6;
    // The right side goes round the outside of the corner landing, the left across the inside;
    // both start at the bottom and end 0.6 m up, level over the landing.
    let right = side(&rs, false);
    assert!(right.iter().any(|p| near(*p, [6.55, -0.55])), "{right:?}");
    let left = side(&rs, true);
    assert!(left.iter().any(|p| near(*p, [5.45, 0.55])), "{left:?}");
    for s in [&right, &left] {
        assert!(s[0][2].abs() < 1e-9 && (s.last().unwrap()[2] - 0.6).abs() < 1e-9, "{s:?}");
        assert!(s.windows(2).all(|w| w[1][2] >= w[0][2] - 1e-9));
    }
    // A wider ramp: the railings move out with it.
    exec(&mut doc, "ramp.set", json!({ "ramp_ids": [ramp], "width": 1.5 }));
    assert!((side(&rails(&doc), false)[0][1] + 0.7).abs() < 1e-6);
    // Added again: both sides have one, so nothing is added.
    assert_eq!(exec(&mut doc, "ramp.add_railings", json!({ "ramp_ids": [ramp] })), json!([]));
    // A 100 mm ramp has none until asked.
    let low = exec(&mut doc, "ramp.create", json!({ "path": [[0, 10], [2, 10]], "base_level": l1.0, "top_constraint": { "unconnected": 0.1 } }));
    assert_eq!(rails(&doc).len(), 2);
    assert_eq!(exec(&mut doc, "ramp.add_railings", json!({ "ramp_ids": [low], "sides": ["left"] })).as_array().unwrap().len(), 1);
    // Saved: the railings sit on the ramp's storey.
    project.save(&doc).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.matches("IFCRAILING(").count(), 3);
    // Deleting the ramp deletes its railings.
    exec(&mut doc, "element.delete", json!({ "element_ids": [ramp] }));
    assert_eq!(rails(&doc).len(), 1);
}
