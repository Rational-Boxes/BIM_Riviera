// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Checkout front ends (ARC-RETAIL-003): banks of checkstands at a pitch with their accessible
//! lanes spread by ADA Table 227.2 and their 36 in aisles kept clear (§904.3.1); the table checked
//! per function, each self-checkout bank its own; the small-store and alteration rules; the lane
//! estimate.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

const IN: f64 = 0.0254;

fn rules(found: &Value) -> Vec<String> {
    let mut v: Vec<String> = found.as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_owned()).collect();
    v.sort();
    v.dedup();
    v
}

#[test]
fn banks_of_checkouts_meet_table_227_2() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "clearance.set_rule_set", json!({ "standard": "ada_2010" }));

    // Nine carousel lanes (48 in wide, 84 in pitch) fit a 20 m front; three are accessible, the
    // middles of three equal shares.
    let bank = ok(&mut app, "frontend.layout", json!({ "start": [0.0, 0.0], "end": [20.0, 0.0], "level": level, "toward": [0.0, 10.0] }));
    let lanes = bank["lanes"].as_array().unwrap();
    assert_eq!(lanes.len(), 9, "{bank:#}");
    let acc: Vec<u64> = lanes.iter().filter(|l| l["accessible"] == true).map(|l| l["index"].as_u64().unwrap()).collect();
    assert_eq!((acc, bank["accessible_required"].as_u64()), (vec![1, 4, 7], Some(3)));
    assert!((lanes[0]["aisle"].as_f64().unwrap() / IN - 36.0).abs() < 1e-6 && lanes[8]["aisle"].is_null(), "{bank:#}");
    // The lanes reach from the front toward the floor, their exit ends on the line.
    let s = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    let it = riviera_core::rules::item(&s, s.get(riviera_core::ElementId(lanes[0]["element"].as_u64().unwrap())).unwrap()).unwrap();
    let exit = it.frame.apply([it.x[1], 0.0, 0.0]);
    assert!(exit[1].abs() < 1e-6 && it.frame.apply([it.x[0], 0.0, 0.0])[1] > 3.5, "{it:?}");
    assert_eq!(ok(&mut app, "rules.check", json!({})), json!([]));
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    let c = ok(&mut app, "checkout.check", json!({}));
    assert_eq!((c["functions"][0]["lanes"].as_u64(), c["functions"][0]["required"].as_u64(), c["findings"].as_array().unwrap().len()), (Some(9), Some(3), 0), "{c:#}");

    // A self-checkout bank is its own function: four lanes, one accessible.
    let sco = ok(&mut app, "frontend.layout", json!({ "start": [0.0, 10.0], "end": [9.0, 10.0], "level": level, "toward": [0.0, 20.0], "item": "Self-checkout lane", "count": 4, "pitch": 1.359 + 0.9144 }));
    assert_eq!((sco["function"].as_str(), sco["accessible_required"].as_u64()), (Some("self_checkout"), Some(1)));
    let c = ok(&mut app, "checkout.check", json!({}));
    assert_eq!(c["functions"].as_array().unwrap().len(), 2, "{c:#}");

    // Five express lanes with none accessible: two short; under 5,000 ft² of selling space one;
    // in an alteration one.
    ok(&mut app, "frontend.layout", json!({ "start": [30.0, 0.0], "end": [45.0, 0.0], "level": level, "toward": [30.0, 10.0], "count": 5, "function": "express", "accessible": [] }));
    let c = ok(&mut app, "checkout.check", json!({}));
    let f: Vec<(String, u64)> = c["findings"].as_array().unwrap().iter().map(|f| (f["function"].as_str().unwrap().to_owned(), f["short"].as_u64().unwrap())).collect();
    assert_eq!(f, vec![("express".to_owned(), 2)], "{c:#}");
    assert_eq!(ok(&mut app, "checkout.check", json!({ "selling_area": 400.0 }))["findings"][0]["short"], 1);
    assert_eq!(ok(&mut app, "checkout.check", json!({ "alteration": true }))["findings"][0]["short"], 1);

    // At 80 in the accessible lanes' aisles are 32 in: §904.3.1.
    let narrow = ok(&mut app, "frontend.layout", json!({ "start": [0.0, 30.0], "end": [12.0, 30.0], "level": level, "toward": [0.0, 40.0], "count": 4, "pitch": 80.0 * IN }));
    let ids: Vec<Value> = narrow["lanes"].as_array().unwrap().iter().map(|l| l["element"].clone()).collect();
    assert_eq!(rules(&ok(&mut app, "rules.check", json!({ "element_ids": ids }))), vec!["ADA 2010 904.3.1"]);

    // Tandem pairs back to back: the first of a pair has no aisle before it, the second an aisle
    // to the next pair of twice the pitch less two lanes.
    let pairs = ok(&mut app, "frontend.layout", json!({ "start": [0.0, 50.0], "end": [20.0, 50.0], "level": level, "toward": [0.0, 60.0], "count": 4, "handing": "paired" }));
    let a: Vec<Option<f64>> = pairs["lanes"].as_array().unwrap().iter().map(|l| l["aisle"].as_f64().map(|x| (x / IN * 100.0).round() / 100.0)).collect();
    let between = ((2.0 * 84.0 * IN - 2.0 * 48.0 * IN - 0.05) / IN * 100.0).round() / 100.0;
    assert_eq!(a, vec![None, Some(between), Some(between), None], "{pairs:#}");
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    let runs = ok(&mut app, "retail.runs", json!({}));
    assert_eq!(runs, json!([]));

    // The estimate: 300 customers an hour with 20 items at 900 items a lane-hour.
    let e = ok(&mut app, "checkout.lanes_estimate", json!({ "customers_per_hour": 300.0, "items_per_basket": 20.0 }));
    assert!(e["estimate"] == true && e["lanes"].as_u64().unwrap() >= 7 && e["wait"].as_f64().unwrap() <= 3.0, "{e:#}");
}
