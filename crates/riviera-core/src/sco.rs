// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Self-checkout pods (ARC-RETAIL-004, `INTERIORS_PLAN.md` §6.2): units in two rows facing each
//! other across a courtyard, an attendant station at the closed end and a shared queue at the
//! open end, and the check of every pod.
//!
//! A pod reaches from its entry (the courtyard's open end) toward its closed end. Each unit's front
//! (local −y) faces the courtyard and its length (local x) runs along the pod. The units are a
//! self-checkout bank of their own ([`crate::checkouts`]): they carry the lane tag, so
//! `checkout.check` counts them under Table 227.2, and the ADA's aisle zone is kept in front of
//! the accessible ones.
//!
//! With no fixed US rule for self-checkout, each unit is checked against an accessibility profile
//! built from EN 301 549 cl. 8.3 and the Access Board's 2022 ANPRM (research/15 C9): advisory
//! findings only. The courtyard, the attendant's sightlines, the queue and the lane light's
//! headroom follow NCR's layout guidance (C8): practice, so notes.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::store::Store;
use crate::txn::Transaction;

/// The tag on a pod's units and attendant station: `{pod, role, courtyard, queue}`.
pub const POD_KEY: &str = "rxd:sco_pod";
/// A unit's accessibility profile, as stated (`sco.set_profile`).
pub const PROFILE_KEY: &str = "rxd:sco_profile";
const IN: f64 = 0.0254;
const FT: f64 = 0.3048;
const DEFAULT_ITEM: &str = "Self-checkout, pedestal";
const STATION: &str = "Attendant station";

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PodParams {
    /// The courtyard's open end (its middle) …
    pub entry: [f64; 2],
    /// … and a point down the pod toward its closed end.
    pub toward: [f64; 2],
    pub level: ElementRef,
    /// The units (default 4: NCR's quad pod).
    pub units: Option<u32>,
    /// The catalogue unit (default the pedestal self-checkout; or the self-checkout lane).
    pub item: Option<String>,
    /// Between the facing rows' fronts (default 11 ft, the middle of NCR's 10–12 ft).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub courtyard: Option<f64>,
    /// Centre to centre along a row (default the unit's length and 1 ft).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub pitch: Option<f64>,
    /// An attendant station at the closed end, facing the entry (default true).
    pub attendant: Option<bool>,
    /// The shared queue's depth beyond the entry (default 3 m).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub queue: Option<f64>,
    /// The accessible units, by index (row on the left looking in first; default as many as
    /// Table 227.2 asks, spread).
    pub accessible: Option<Vec<u32>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Pod {
    pub pod: String,
    pub units: Vec<ElementId>,
    pub accessible: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attendant: Option<ElementId>,
    pub courtyard: f64,
    /// The shared queue's corners in plan.
    pub queue: Vec<[f64; 2]>,
}

fn rotation_facing(f: [f64; 2]) -> f64 {
    f[0].atan2(-f[1])
}

/// Places a self-checkout pod (`sco.place_pod`).
pub(crate) fn place_pod(txn: &mut Transaction<'_>, p: PodParams) -> Result<Pod> {
    let item = p.item.clone().unwrap_or_else(|| DEFAULT_ITEM.to_owned());
    let sizes = crate::interior_layouts::sizes();
    let size = sizes.get(&item).copied().ok_or_else(|| bad("sco.no_item", format!("no catalogue item {item:?}")))?;
    let mut names = vec![item.clone()];
    let attendant = p.attendant.unwrap_or(true);
    if attendant {
        names.push(STATION.into());
    }
    let types = crate::furnishings::load(txn, crate::furnishings::LoadParams { names })?;
    let ty = types[&item];
    let kind = txn.store().get(ty).and_then(|t| t.param("@ElementType")).and_then(Value::as_str).unwrap_or_default().to_owned();
    if kind != "SelfCheckout" {
        return Err(bad("sco.not_a_unit", format!("{item} is not a self-checkout")));
    }
    let n = p.units.unwrap_or(4);
    if n == 0 {
        return Err(bad("sco.no_units", "a pod needs a unit"));
    }
    let v = [p.toward[0] - p.entry[0], p.toward[1] - p.entry[1]];
    let l = v[0].hypot(v[1]);
    if l < 1e-6 {
        return Err(bad("sco.no_direction", "`toward` must differ from `entry`"));
    }
    let d = [v[0] / l, v[1] / l];
    // On the left looking in.
    let s = [-d[1], d[0]];
    let (len, width) = (size[0], size[1]);
    let court = p.courtyard.unwrap_or(11.0 * FT);
    if court <= 0.0 {
        return Err(bad("sco.courtyard", "the courtyard must be wider than nothing"));
    }
    let pitch = p.pitch.unwrap_or(len + FT);
    if pitch < len - 1e-6 {
        return Err(bad("sco.pitch", format!("the pitch is under a unit's {:.0} mm length", len * 1000.0)));
    }
    let required = crate::checkouts::table_227_2(n);
    let accessible: Vec<u32> = match &p.accessible {
        Some(list) => {
            if let Some(k) = list.iter().find(|k| **k >= n) {
                return Err(bad("sco.bad_unit", format!("there is no unit {k}: the pod has {n}")));
            }
            list.clone()
        }
        None => (0..required).map(|j| ((f64::from(j) + 0.5) * f64::from(n) / f64::from(required)).floor() as u32).collect(),
    };
    let pod = crate::id::GlobalId::new().as_str().to_owned();
    let q = p.queue.unwrap_or(3.0);
    let at = |a: f64, b: f64| [p.entry[0] + d[0] * a + s[0] * b, p.entry[1] + d[1] * a + s[1] * b];
    let queue = vec![at(0.0, -court / 2.0), at(0.0, court / 2.0), at(-q, court / 2.0), at(-q, -court / 2.0)];
    let tag = |role: &str| json!({ "pod": pod, "role": role, "courtyard": court, "queue": queue });
    let left = n.div_ceil(2);
    let mut units = Vec::new();
    for k in 0..n {
        let (row, side) = if k < left { (k, 1.0) } else { (k - left, -1.0) };
        let point = at(len / 2.0 + f64::from(row) * pitch, side * (court / 2.0 + width / 2.0));
        let front = [-s[0] * side, -s[1] * side];
        let id = crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(ty), point, level: p.level.clone(), offset: 0.0, rotation: rotation_facing(front), mirror: false, scale: 1.0 })?;
        let acc = accessible.contains(&k);
        let lane = json!({ "frontend": pod, "function": "self_checkout", "lane": k, "accessible": acc });
        let t = tag("unit");
        txn.modify(id, |e| {
            e.params.insert(crate::checkouts::LANE_KEY.into(), lane);
            e.params.insert(POD_KEY.into(), t);
            if !acc {
                e.params.insert(crate::rules::ACCESSIBLE_KEY.into(), Value::Bool(false));
            }
        })?;
        units.push(id);
    }
    let station = if attendant {
        let st = sizes[STATION];
        let rows = f64::from(left.saturating_sub(1)) * pitch + len;
        let point = at(rows + 0.6 + st[1] / 2.0, 0.0);
        let id = crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(types[STATION]), point, level: p.level.clone(), offset: 0.0, rotation: rotation_facing([-d[0], -d[1]]), mirror: false, scale: 1.0 })?;
        let t = tag("attendant");
        txn.modify(id, |e| {
            e.params.insert(POD_KEY.into(), t);
        })?;
        Some(id)
    } else {
        None
    };
    Ok(Pod { pod, units, accessible, attendant: station, courtyard: court, queue })
}

/// Which side of the unit a customer reaches from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Approach {
    #[default]
    Forward,
    Side,
}

/// A unit's accessibility features, as its maker states them.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// The lowest and highest operable part above the floor: touch targets, PIN pad, card slot,
    /// coin and note slots, receipt (metres).
    pub operable: Option<[f64; 2]>,
    #[serde(default)]
    pub approach: Approach,
    /// How far the reach passes over the unit (the bagging deck or shelf) to the highest part.
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub obstruction_depth: Option<f64>,
    /// The screen's centre above the floor.
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub screen_height: Option<f64>,
    /// Speech output through a standard headphone jack.
    pub speech: Option<bool>,
    /// A tactile keypad (12 keys, a raised "5").
    pub tactile_keypad: Option<bool>,
    /// Braille instructions to start speech.
    pub braille: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetProfileParams {
    pub element_ids: Vec<ElementRef>,
    #[serde(flatten)]
    pub profile: Profile,
}

/// States the accessibility features of self-checkout units (`sco.set_profile`).
pub(crate) fn set_profile(txn: &mut Transaction<'_>, p: SetProfileParams) -> Result<()> {
    let v = serde_json::to_value(&p.profile).map_err(|e| bad("sco.bad_profile", e.to_string()))?;
    for r in &p.element_ids {
        let id = txn.store().resolve(r).ok_or_else(|| bad("element.not_found", "no such element"))?;
        if txn.store().get(id).and_then(|e| e.param(crate::checkouts::LANE_KEY)).and_then(|t| t.get("function")).and_then(Value::as_str) != Some("self_checkout") {
            return Err(bad("sco.not_a_unit", format!("{} is not a self-checkout unit", id.0)));
        }
        let v = v.clone();
        txn.modify(id, |e| {
            e.params.insert(PROFILE_KEY.into(), v);
        })?;
    }
    Ok(())
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckParams {
    /// The height the attendant's sightlines are checked at (default 1.4 m: between a standing
    /// attendant's eye and the screens).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub sight_height: Option<f64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ScoFinding {
    pub rule: String,
    pub source: String,
    /// `note` (practice) or `advisory` (the accessibility profile).
    pub severity: String,
    pub pod: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<ElementId>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PodSummary {
    pub pod: String,
    pub units: u32,
    pub attendants: u32,
    /// Units whose stated profile meets every item.
    pub meeting_profile: u32,
    pub courtyard: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ScoCheck {
    pub pods: Vec<PodSummary>,
    pub findings: Vec<ScoFinding>,
}

/// The plan segments where every body (but floors and zones) crosses the plane at `h`.
pub(crate) fn slice(store: &Store, h: f64) -> Vec<(ElementId, [[f64; 2]; 2])> {
    let mut out = Vec::new();
    for b in crate::bodies::all(store) {
        if crate::rules::may_share(store, b.element) {
            continue;
        }
        for part in &b.parts {
            let (pos, idx) = &part.mesh;
            for t in idx.as_chunks::<3>().0 {
                let v = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                let mut pts = Vec::new();
                for (a, c) in [(0, 1), (1, 2), (2, 0)] {
                    let (za, zc) = (v[a][2] - h, v[c][2] - h);
                    if (za < 0.0) != (zc < 0.0) {
                        let f = za / (za - zc);
                        pts.push([v[a][0] + (v[c][0] - v[a][0]) * f, v[a][1] + (v[c][1] - v[a][1]) * f]);
                    }
                }
                if pts.len() == 2 {
                    out.push((b.element, [pts[0], pts[1]]));
                }
            }
        }
    }
    out
}

fn cross(o: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn segments_cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

fn inside(p: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let mut c = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) {
            c = !c;
        }
    }
    c
}

/// The first element whose slice enters the outline.
pub(crate) fn blocking(cuts: &[(ElementId, [[f64; 2]; 2])], poly: &[[f64; 2]]) -> Option<ElementId> {
    cuts.iter()
        .find(|(_, [a, b])| inside(*a, poly) || inside(*b, poly) || (0..poly.len()).any(|i| segments_cross(*a, *b, poly[i], poly[(i + 1) % poly.len()])))
        .map(|c| c.0)
}

pub(crate) fn name(store: &Store, id: ElementId) -> String {
    store.get(id).map(|e| e.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| e.class.clone())).unwrap_or_default()
}

/// The profile's shortfalls against EN 301 549 cl. 8.3 and the ANPRM; `None` when unstated.
fn shortfalls(p: &Profile) -> Option<Vec<(&'static str, &'static str, String)>> {
    let [lo, hi] = p.operable?;
    let mut out = Vec::new();
    const EN: &str = "EN 301 549 8.3";
    if lo < 0.38 - 1e-9 {
        out.push((EN, "S1427", format!("the lowest operable part is at {:.0} mm, under 380 mm", lo * 1000.0)));
    }
    let ob = p.obstruction_depth.unwrap_or(0.0);
    let limit = match p.approach {
        Approach::Forward if ob > 0.635 => None,
        Approach::Forward if ob > 0.51 => Some(1.12),
        Approach::Side if ob > 0.61 => None,
        Approach::Side if ob > 0.255 => Some(1.17),
        _ => Some(1.22),
    };
    match limit {
        None => out.push((EN, "S1427", format!("a reach over {:.0} mm is too deep to be within reach", ob * 1000.0))),
        Some(l) if hi > l + 1e-9 => out.push((EN, "S1427", format!("the highest operable part is at {:.0} mm, over {:.0} mm", hi * 1000.0, l * 1000.0))),
        Some(_) => {}
    }
    for (have, what) in [(p.speech, "speech output through a headphone jack"), (p.tactile_keypad, "a tactile keypad"), (p.braille, "Braille instructions to start speech")] {
        if have != Some(true) {
            out.push(("Access Board ANPRM 2022", "S1424", format!("{what} is not stated")));
        }
    }
    Some(out)
}

/// Checks the self-checkout pods (`sco.check`).
pub(crate) fn check(store: &Store, p: Option<CheckParams>) -> Result<ScoCheck> {
    let h = p.unwrap_or_default().sight_height.unwrap_or(1.4);
    // pod → (units, stations, courtyard, queue).
    type Group = (Vec<ElementId>, Vec<ElementId>, f64, Vec<[f64; 2]>);
    let mut pods: BTreeMap<String, Group> = BTreeMap::new();
    for e in store.iter() {
        let Some(t) = e.param(POD_KEY) else { continue };
        let Some(pod) = t.get("pod").and_then(Value::as_str) else { continue };
        let g = pods.entry(pod.to_owned()).or_insert_with(|| {
            let q = t.get("queue").and_then(|q| serde_json::from_value(q.clone()).ok()).unwrap_or_default();
            (Vec::new(), Vec::new(), t.get("courtyard").and_then(Value::as_f64).unwrap_or(0.0), q)
        });
        if t.get("role").and_then(Value::as_str) == Some("attendant") { g.1.push(e.id()) } else { g.0.push(e.id()) }
    }
    let mut summaries = Vec::new();
    let mut findings = Vec::new();
    if pods.is_empty() {
        return Ok(ScoCheck { pods: summaries, findings });
    }
    let bodies = crate::bodies::all(store);
    let sight_cuts = slice(store, h);
    let low_cuts = slice(store, 0.3);
    for (pod, (units, stations, court, queue)) in pods {
        let short = pod[..pod.len().min(8)].to_owned();
        let mut note = |rule: &str, src: &str, severity: &str, element: Option<ElementId>, message: String| {
            findings.push(ScoFinding { rule: rule.into(), source: src.into(), severity: severity.into(), pod: short.clone(), element, message });
        };
        const NCR: &str = "S1408";
        if !(10.0 * FT - 1e-6..=12.0 * FT + 1e-6).contains(&court) {
            note("NCR courtyard", NCR, "note", None, format!("the courtyard is {court:.2} m across; NCR leaves 10–12 ft (3.0–3.7 m)"));
        }
        if stations.is_empty() {
            note("Attendant", NCR, "note", None, "no attendant station: put one in clear view of every unit".into());
        } else if units.len() > 8 * stations.len() {
            note("Attendant ratio", "S1458", "note", None, format!("{} units for {} attendant station(s): one attendant usually serves 4–8", units.len(), stations.len()));
        }
        let items: Vec<crate::rules::Item> = units.iter().filter_map(|u| store.get(*u).and_then(|e| crate::rules::item(store, e))).collect();
        // Sightlines from just in front of each station to each unit's screen (its front face).
        for st in stations.iter().filter_map(|s| store.get(*s).and_then(|e| crate::rules::item(store, e))) {
            let eye = st.frame.apply([0.0, st.front - 0.3, 0.0]);
            for it in &items {
                let screen = it.frame.apply([(it.x[0] + it.x[1]) / 2.0, it.front - 0.01, 0.0]);
                let (a, b) = ([eye[0], eye[1]], [screen[0], screen[1]]);
                if let Some((by, _)) = sight_cuts.iter().find(|(id, [c, d])| *id != it.element && *id != st.element && segments_cross(a, b, *c, *d)) {
                    note("Attendant sightline", NCR, "note", Some(it.element), format!("{} stands between the attendant station and this unit's screen", name(store, *by)));
                }
            }
        }
        let mut own = units.clone();
        own.extend(&stations);
        let others: Vec<_> = low_cuts.iter().filter(|c| !own.contains(&c.0)).copied().collect();
        if queue.len() >= 3
            && let Some(by) = blocking(&others, &queue)
        {
            note("Shared queue", "research/15 C10", "note", None, format!("{} stands in the pod's shared queue", name(store, by)));
        }
        let mut meeting = 0;
        for it in &items {
            let e = store.get(it.element).expect("a unit");
            let plan = |q: &[[f64; 2]]| -> Vec<[f64; 2]> { q.iter().map(|q| it.frame.apply([q[0], q[1], 0.0])).map(|w| [w[0], w[1]]).collect() };
            // NCR: 96 in clear over a unit for its lane light.
            let foot = plan(&[[it.x[0], it.front], [it.x[1], it.front], [it.x[1], it.back], [it.x[0], it.back]]);
            let over = bodies.iter().filter(|b| !own.contains(&b.element) && !crate::rules::may_share(store, b.element)).find_map(|b| {
                let (lo, hi) = b.bounds()?;
                let hits = lo[2] > it.z[1] - 1e-6 && lo[2] < it.floor + 96.0 * IN - 1e-6 && foot.iter().any(|q| q[0] >= lo[0] && q[0] <= hi[0] && q[1] >= lo[1] && q[1] <= hi[1]);
                hits.then_some((b.element, lo[2] - it.floor))
            });
            if let Some((by, z)) = over {
                note("Lane light headroom", NCR, "note", Some(it.element), format!("{} is {z:.2} m up over this unit; NCR keeps 96 in (2.44 m) clear for the lane light", name(store, by)));
            }
            let profile: Option<Profile> = e.param(PROFILE_KEY).and_then(|v| serde_json::from_value(v.clone()).ok());
            let Some(mut fails) = profile.as_ref().and_then(shortfalls) else {
                if it.accessible {
                    note("Self-checkout profile", "S1427", "advisory", Some(it.element), "an accessible unit with no stated profile: state its operable heights and features (sco.set_profile)".into());
                }
                continue;
            };
            // The clear floor space, 760 × 1220 mm in front of the unit.
            let cx = (it.x[0] + it.x[1]) / 2.0;
            let space = plan(&[[cx - 0.38, it.front - 0.005], [cx + 0.38, it.front - 0.005], [cx + 0.38, it.front - 1.225], [cx - 0.38, it.front - 1.225]]);
            if let Some(by) = blocking(&others, &space) {
                fails.push(("EN 301 549 8.3", "S1427", format!("{} stands in the 760 × 1220 mm clear floor space", name(store, by))));
            }
            if let Some(sh) = profile.as_ref().and_then(|p| p.screen_height)
                && !(1.2 - 1e-9..=1.4 + 1e-9).contains(&sh)
            {
                note("EN 301 549 8.3 recommended", "S1427", "advisory", Some(it.element), format!("the screen's centre is at {:.0} mm; 1200–1400 mm is recommended", sh * 1000.0));
            }
            if fails.is_empty() {
                meeting += 1;
            } else if it.accessible {
                for (rule, src, msg) in fails {
                    note(rule, src, "advisory", Some(it.element), msg);
                }
            }
        }
        if meeting == 0 && !items.is_empty() {
            note("Self-checkout profile", "S1424", "advisory", None, "no unit meets the accessibility profile: make at least one in the pod meet it (preferably all)".into());
        }
        summaries.push(PodSummary { pod: short, units: units.len() as u32, attendants: stations.len() as u32, meeting_profile: meeting, courtyard: court });
    }
    Ok(ScoCheck { pods: summaries, findings })
}
