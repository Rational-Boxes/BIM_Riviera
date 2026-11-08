// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Self-checkout pods (ARC-RETAIL-004): units in two facing rows round a courtyard with the
//! attendant at the closed end; the pod counted as its own bank under Table 227.2; the practice
//! notes (courtyard, sightlines, queue) and the advisory accessibility profile.

use riviera_app::{App, AppConfig};
use riviera_core::Caller;
use serde_json::{Value, json};

fn ok(app: &mut App, name: &str, args: Value) -> Value {
    app.execute(name, args, &Caller::user()).map(|o| o.result).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn rules(c: &Value) -> Vec<String> {
    let mut v: Vec<String> = c["findings"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_owned()).collect();
    v.sort();
    v.dedup();
    v
}

#[test]
fn a_pod_is_laid_out_and_checked() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(AppConfig { live: None, untitled_dir: dir.path().join("u") });
    ok(&mut app, "file.new", json!({}));
    let level = ok(&mut app, "level.list", json!({}))[0]["id"].clone();
    ok(&mut app, "clearance.set_rule_set", json!({ "standard": "ada_2010" }));

    // Six pedestals reaching north from (0, 0): two rows of three; two of the six accessible, one
    // in each row.
    let pod = ok(&mut app, "sco.place_pod", json!({ "entry": [0.0, 0.0], "toward": [0.0, 5.0], "level": level, "units": 6 }));
    assert_eq!((pod["units"].as_array().unwrap().len(), pod["accessible"].clone()), (6, json!([1, 4])), "{pod:#}");
    let s = app.document(app.active().unwrap()).unwrap().doc.store().clone();
    let item = |id: &Value| riviera_core::rules::item(&s, s.get(riviera_core::ElementId(id.as_u64().unwrap())).unwrap()).unwrap();
    // Row one on the west (left looking north) faces east, row two faces west, their fronts 11 ft
    // apart.
    let (a, b) = (item(&pod["units"][0]), item(&pod["units"][3]));
    let (fa, fb) = (a.frame.apply([0.0, a.front, 0.0]), b.frame.apply([0.0, b.front, 0.0]));
    assert!((fb[0] - fa[0] - 11.0 * 0.3048).abs() < 1e-6 && fa[0] < 0.0, "{fa:?} {fb:?}");
    // The attendant station is past the rows on the axis, facing south.
    let st = item(&pod["attendant"]);
    let sf = st.frame.apply([0.0, st.front, 0.0]);
    assert!(sf[0].abs() < 1e-6 && sf[1] > 2.0 && st.frame.apply([0.0, st.back, 0.0])[1] > sf[1], "{st:?}");
    assert_eq!(ok(&mut app, "rules.check", json!({})), json!([]));
    assert_eq!(ok(&mut app, "overlap.find", json!({}))["overlaps"], json!([]));
    let t = ok(&mut app, "checkout.check", json!({}));
    assert_eq!((t["functions"][0]["lanes"].as_u64(), t["functions"][0]["accessible"].as_u64(), t["findings"].clone()), (Some(6), Some(2), json!([])), "{t:#}");

    // With no profile stated, each accessible unit and the pod are advisory findings.
    let c = ok(&mut app, "sco.check", json!({}));
    assert_eq!((rules(&c), c["findings"].as_array().unwrap().len()), (vec!["Self-checkout profile".to_owned()], 3), "{c:#}");
    assert!(c["findings"].as_array().unwrap().iter().all(|f| f["severity"] == "advisory"));
    let acc = [pod["units"][1].clone(), pod["units"][4].clone()];
    ok(&mut app, "sco.set_profile", json!({ "element_ids": acc, "operable": [0.8, 1.15], "screen_height": 1.3, "speech": true, "tactile_keypad": true, "braille": true }));
    let c = ok(&mut app, "sco.check", json!({}));
    assert_eq!((c["findings"].clone(), c["pods"][0]["meeting_profile"].as_u64(), c["pods"][0]["attendants"].as_u64()), (json!([]), Some(2), Some(1)), "{c:#}");

    // A card slot at 1300 mm and no Braille: advisory shortfalls for that unit.
    ok(&mut app, "sco.set_profile", json!({ "element_ids": [pod["units"][1]], "operable": [0.8, 1.3], "speech": true, "tactile_keypad": true }));
    let c = ok(&mut app, "sco.check", json!({}));
    assert_eq!(rules(&c), vec!["Access Board ANPRM 2022".to_owned(), "EN 301 549 8.3".to_owned()], "{c:#}");
    ok(&mut app, "sco.set_profile", json!({ "element_ids": [pod["units"][1]], "operable": [0.8, 1.15], "speech": true, "tactile_keypad": true, "braille": true }));

    // An EAS pedestal 1.55 m tall in the courtyard, between the station and the first unit's
    // screen: a sightline note. In the queue it is a queue note.
    let eas = ok(&mut app, "furnishing.load", json!({ "names": ["EAS pedestal"] }))["EAS pedestal"].clone();
    let mid = [(sf[0] + fa[0]) / 2.0, (sf[1] + fa[1]) / 2.0];
    let blocker = ok(&mut app, "mesh_object.place", json!({ "type_id": eas, "point": mid, "level": level }));
    let c = ok(&mut app, "sco.check", json!({}));
    assert!(rules(&c).contains(&"Attendant sightline".to_owned()), "{c:#}");
    ok(&mut app, "element.delete", json!({ "element_ids": [blocker] }));
    ok(&mut app, "mesh_object.place", json!({ "type_id": eas, "point": [0.0, -1.5], "level": level }));
    let c = ok(&mut app, "sco.check", json!({}));
    assert_eq!(rules(&c), vec!["Shared queue".to_owned()], "{c:#}");

    // A pod 2.5 m across with no attendant: practice notes.
    ok(&mut app, "sco.place_pod", json!({ "entry": [10.0, 0.0], "toward": [10.0, 5.0], "level": level, "courtyard": 2.5, "attendant": false, "accessible": [] }));
    let c = ok(&mut app, "sco.check", json!({}));
    let r = rules(&c);
    assert!(r.contains(&"NCR courtyard".to_owned()) && r.contains(&"Attendant".to_owned()), "{c:#}");
    // And Table 227.2 finds the second pod's bank short.
    let t = ok(&mut app, "checkout.check", json!({}));
    assert_eq!(t["findings"].as_array().unwrap().len(), 1, "{t:#}");
}
