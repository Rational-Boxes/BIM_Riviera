// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Gyms (ARC-GYM-001, `INTERIORS_PLAN.md` §7.2): wall mirrors, and the check of a gym's
//! equipment.
//!
//! The equipment is the *Riviera Gym Equipment* library's ([`crate::furnishings`]), each piece an
//! `IfcFurniture` whose ElementType names what it is. Its use zones (a treadmill's sides and rear,
//! a rower's use area, a dumbbell rack's live depth) are the `gym_equipment` pack's, checked by
//! `rules.check`. `gym.check` adds what zones cannot say:
//! - ADA §236.1: one of each type of machine accessible, with a 30 × 48 in clear floor space
//!   beside it (§1004.1, §305.3);
//! - the ceiling over each piece, against its height and its user's rise;
//! - mirrors at treadmills (Sport England), and mirrors without safety glazing.
//!
//! A wall mirror is an `IfcCovering` USERDEFINED ("MIRROR") on its wall (the owner's answer to the
//! plan's Question 4).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::bath_components::Shape;
use crate::element::Element;
use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::mesh_objects::Category;
use crate::store::Store;
use crate::txn::Transaction;

/// The tag on a wall mirror: `{safety_glazing}`.
pub const MIRROR_KEY: &str = "rxd:gym_mirror";
const IN: f64 = 0.0254;
const GLASS: [f32; 4] = [0.78, 0.86, 0.9, 0.6];

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

/// The gym library's kinds, each with the clear height over it its use needs (metres; `None`:
/// its own height and 0.1 m) and whether it is exercise equipment under ADA §236.1 (storage is
/// not).
const KINDS: &[(&str, Option<f64>, bool)] = &[
    // A tall runner (1.93 m) on a 0.25 m deck with a bounce: 2.5 m (judgement, E-114).
    ("Treadmill", Some(2.5), true),
    ("Elliptical", Some(2.5), true),
    // 2.0 m machine; the user stands about 0.6 m up on its steps.
    ("StairClimber", Some(2.9), true),
    // A retailer's 2.5 m for riding a connected bike (S1370).
    ("ExerciseBike", Some(2.5), true),
    ("Rower", None, true),
    ("LegPress", None, true),
    ("CableCrossover", None, true),
    // Overhead presses and pull-ups: 2.7 m comfortable (S1388).
    ("PowerRack", Some(2.7), true),
    ("HalfRack", Some(2.7), true),
    ("SmithMachine", Some(2.7), true),
    ("WeightBench", Some(2.4), true),
    // Olympic lifting: about 3.7 m at least (S1388).
    ("LiftingPlatform", Some(3.7), true),
    ("DumbbellRack", None, false),
    ("PlateTree", None, false),
];

/// An element's ElementType, its own or its type's.
fn kind_of<'a>(store: &'a Store, e: &'a Element) -> Option<&'a str> {
    let text = |el: &'a Element, k: &str| el.param(k).and_then(Value::as_str).filter(|v| !v.is_empty());
    text(e, "@ObjectType").or_else(|| {
        let ty = e.param("@type").and_then(Value::as_str).and_then(crate::id::GlobalId::parse).and_then(|g| store.find_by_guid(&g)).and_then(|t| store.get(t))?;
        text(ty, "@ElementType")
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MirrorParams {
    /// Along the wall's face: from here …
    pub start: [f64; 2],
    /// … to here.
    pub end: [f64; 2],
    pub level: ElementRef,
    /// A point in the room, on the side the mirror faces.
    pub toward: [f64; 2],
    /// Its bottom edge above the floor (default 0.4 m, the top of the makers' 12–16 in).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub bottom: Option<f64>,
    /// Its top edge above the floor (default 2.1 m).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub top: Option<f64>,
    /// Tempered safety glass or a safety-film backing (default true).
    pub safety_glazing: Option<bool>,
}

/// Places a wall mirror (`gym.mirror`).
pub(crate) fn mirror(txn: &mut Transaction<'_>, p: MirrorParams) -> Result<ElementId> {
    let v = [p.end[0] - p.start[0], p.end[1] - p.start[1]];
    let len = v[0].hypot(v[1]);
    if len < 0.1 {
        return Err(bad("gym.too_short", "a mirror needs to be longer than 0.1 m"));
    }
    let (bottom, top) = (p.bottom.unwrap_or(0.4), p.top.unwrap_or(2.1));
    if !(bottom >= 0.0 && top > bottom + 0.05) {
        return Err(bad("gym.bad_height", "the mirror's top must be above its bottom, its bottom at or above the floor"));
    }
    let u = [v[0] / len, v[1] / len];
    let mut front = [-u[1], u[0]];
    let mid = [(p.start[0] + p.end[0]) / 2.0, (p.start[1] + p.end[1]) / 2.0];
    if (p.toward[0] - mid[0]) * front[0] + (p.toward[1] - mid[1]) * front[1] < 0.0 {
        front = [-front[0], -front[1]];
    }
    let name = format!("Wall mirror {:.0} × {:.0} mm", len * 1000.0, (top - bottom) * 1000.0);
    let existing = txn.store().iter().find(|e| e.name.as_deref() == Some(name.as_str()) && crate::mesh_objects::mesh_object_type(e).is_some()).map(Element::id);
    let ty = match existing {
        Some(id) => id,
        None => {
            let sh = Shape::default().cuboid([-len / 2.0, len / 2.0], [-0.006, 0.0], [bottom, top]);
            let id = crate::mesh_objects::create_generated_type(txn, &name, Category::Covering, &sh.p, &sh.t, GLASS, Vec::new(), Some("USERDEFINED"))?;
            txn.modify(id, |e| {
                e.params.insert("@ElementType".into(), "MIRROR".into());
            })?;
            id
        }
    };
    let rotation = front[0].atan2(-front[1]);
    let id = crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(ty), point: mid, level: p.level, offset: 0.0, rotation, mirror: false, scale: 1.0 })?;
    let tag = json!({ "safety_glazing": p.safety_glazing.unwrap_or(true) });
    txn.modify(id, |e| {
        e.params.insert(MIRROR_KEY.into(), tag);
    })?;
    Ok(id)
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckParams {}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TypeCount {
    pub kind: String,
    pub units: u32,
    /// Units not marked not accessible.
    pub accessible: u32,
    /// Accessible units with a clear 30 × 48 in space beside them.
    pub with_space: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GymFinding {
    pub rule: String,
    pub source: String,
    /// `error` or `note`.
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<ElementId>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GymCheck {
    pub types: Vec<TypeCount>,
    pub findings: Vec<GymFinding>,
}

/// A plan point in an item's frame (x across, y from its front to its back).
fn local(it: &crate::rules::Item, q: [f64; 2]) -> [f64; 2] {
    let (dx, dy) = (q[0] - it.frame.origin[0], q[1] - it.frame.origin[1]);
    let (c, s) = (it.frame.rotation.cos(), it.frame.rotation.sin());
    let (lx, ly) = (dx * c + dy * s, -dx * s + dy * c);
    [lx, if it.frame.mirror { -ly } else { ly }]
}

fn plan(it: &crate::rules::Item, q: &[[f64; 2]]) -> Vec<[f64; 2]> {
    q.iter().map(|q| it.frame.apply([q[0], q[1], 0.0])).map(|w| [w[0], w[1]]).collect()
}

/// The 30 × 48 in clear floor spaces that could serve an item: along each side (parallel) and
/// at each end (forward), centred.
fn spaces(it: &crate::rules::Item) -> Vec<Vec<[f64; 2]>> {
    let (w, l) = (30.0 * IN, 48.0 * IN);
    let (cx, cy) = ((it.x[0] + it.x[1]) / 2.0, (it.front + it.back) / 2.0);
    let g = 0.005;
    let rect = |x0: f64, x1: f64, y0: f64, y1: f64| plan(it, &[[x0, y0], [x1, y0], [x1, y1], [x0, y1]]);
    vec![
        rect(it.x[0] - g - w, it.x[0] - g, cy - l / 2.0, cy + l / 2.0),
        rect(it.x[1] + g, it.x[1] + g + w, cy - l / 2.0, cy + l / 2.0),
        rect(cx - w / 2.0, cx + w / 2.0, it.front - g - l, it.front - g),
        rect(cx - w / 2.0, cx + w / 2.0, it.back + g, it.back + g + l),
    ]
}

/// Checks a gym's equipment (`gym.check`).
pub(crate) fn check(store: &Store, _: Option<CheckParams>) -> Result<GymCheck> {
    let ada = crate::clearances::rule_set(store) == crate::clearances::RuleSet::Ada2010;
    let mut pieces: Vec<(&'static str, Option<f64>, bool, crate::rules::Item)> = Vec::new();
    let mut mirrors = Vec::new();
    for e in store.iter() {
        if let Some(t) = e.param(MIRROR_KEY) {
            mirrors.push((e.id(), t.get("safety_glazing").and_then(Value::as_bool) != Some(false)));
            continue;
        }
        let Some(k) = kind_of(store, e) else { continue };
        let Some(&(kind, need, equipment)) = KINDS.iter().find(|(n, ..)| n.eq_ignore_ascii_case(k)) else { continue };
        if let Some(it) = crate::rules::item(store, e) {
            pieces.push((kind, need, equipment, it));
        }
    }
    let mut findings = Vec::new();
    let mut note = |rule: &str, source: &str, severity: &str, element: Option<ElementId>, message: String| {
        findings.push(GymFinding { rule: rule.into(), source: source.into(), severity: severity.into(), element, message });
    };
    if pieces.is_empty() && mirrors.is_empty() {
        return Ok(GymCheck { types: Vec::new(), findings: Vec::new() });
    }
    let bodies = crate::bodies::all(store);
    let cuts = crate::sco::slice(store, 0.3);
    let ids: Vec<ElementId> = pieces.iter().map(|p| p.3.element).collect();
    // ADA §236.1: by type, one accessible unit with its clear floor space.
    let mut types: BTreeMap<&str, TypeCount> = BTreeMap::new();
    for (kind, _, equipment, it) in &pieces {
        if !equipment {
            continue;
        }
        let t = types.entry(kind).or_insert_with(|| TypeCount { kind: (*kind).to_owned(), units: 0, accessible: 0, with_space: 0 });
        t.units += 1;
        if it.accessible {
            t.accessible += 1;
            let others: Vec<_> = cuts.iter().filter(|c| c.0 != it.element).copied().collect();
            if spaces(it).iter().any(|s| crate::sco::blocking(&others, s).is_none()) {
                t.with_space += 1;
            }
        }
    }
    if ada {
        for t in types.values().filter(|t| t.with_space == 0) {
            let why = if t.accessible == 0 { "none is accessible" } else { "none of the accessible ones has a clear 30 × 48 in space beside it" };
            note("ADA 2010 236.1", "S1354", "error", None, format!("{} {}: one of each type must be accessible (§1004.1), and {why}", t.units, t.kind));
        }
    }
    // The ceiling over each piece: the lowest thing above it.
    for (kind, need, _, it) in &pieces {
        let need = need.unwrap_or(0.0).max(it.z[1] - it.floor + 0.1);
        let foot = plan(it, &[[it.x[0], it.front], [it.x[1], it.front], [it.x[1], it.back], [it.x[0], it.back]]);
        let over = bodies
            .iter()
            .filter(|b| !ids.contains(&b.element))
            .filter_map(|b| {
                let (lo, hi) = b.bounds()?;
                let above = lo[2] > it.floor + 0.5 && lo[2] > it.z[1] - 1e-6;
                (above && foot.iter().any(|q| q[0] >= lo[0] && q[0] <= hi[0] && q[1] >= lo[1] && q[1] <= hi[1])).then_some((b.element, lo[2] - it.floor))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((by, h)) = over
            && h < need - 1e-6
        {
            note("Ceiling height", "S1388 S1370 S1375", "note", Some(it.element), format!("{} is {h:.2} m up over this {kind}; its use needs {need:.2} m", crate::sco::name(store, by)));
        }
    }
    // Mirrors: none at treadmills (in line with the belt, within 3 m of either end).
    for (m, safety) in &mirrors {
        let Some((lo, hi)) = bodies.iter().find(|b| b.element == *m).and_then(|b| b.bounds()) else { continue };
        if !safety {
            note("Mirror safety glazing", "S1389", "note", Some(*m), "the mirror is not safety glazing: use tempered glass or a safety-film backing (ANSI Z97.1, CPSC 16 CFR 1201)".into());
        }
        let corners = [[lo[0], lo[1]], [hi[0], lo[1]], [hi[0], hi[1]], [lo[0], hi[1]]];
        for (_, _, _, it) in pieces.iter().filter(|p| p.0 == "Treadmill") {
            // The mirror's extent in the treadmill's frame, against the strip along its belt.
            let q: Vec<[f64; 2]> = corners.iter().map(|c| local(it, *c)).collect();
            let (x0, x1) = q.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
            let (y0, y1) = q.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
            let near = x0 < it.x[1] && x1 > it.x[0] && y0 < it.back + 3.0 && y1 > it.front - 3.0;
            if near {
                note("Sport England mirrors", "S1376", "note", Some(*m), format!("a mirror in line with a treadmill ({}): Sport England keeps them to the free-weight area, as they confuse visually impaired users", crate::sco::name(store, it.element)));
            }
        }
    }
    findings.sort_by_key(|f| f.severity != "error");
    Ok(GymCheck { types: types.into_values().collect(), findings })
}
