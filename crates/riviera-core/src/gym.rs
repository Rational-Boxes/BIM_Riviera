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

/// A room holding gym equipment.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GymRoom {
    pub room: ElementId,
    pub name: String,
    pub area: f64,
    pub pieces: u32,
    /// IBC Table 1004.5's exercise rooms: 50 ft² gross an occupant.
    pub occupants: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GymCheck {
    pub types: Vec<TypeCount>,
    pub rooms: Vec<GymRoom>,
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
        return Ok(GymCheck { types: Vec::new(), rooms: Vec::new(), findings: Vec::new() });
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
    // Each room's occupant load, and its equipment against Sport England's 5 m² a piece.
    let mut rooms = Vec::new();
    for r in crate::rooms::all(store).into_iter().filter(|r| !r.redundant) {
        let Some(region) = &r.geometry.region else { continue };
        let ring: Vec<[f64; 2]> = region.outer.iter().map(|e| e.line.start).collect();
        let n = pieces.iter().filter(|p| p.2 && crate::sco::inside([p.3.frame.origin[0], p.3.frame.origin[1]], &ring)).count() as u32;
        if n == 0 {
            continue;
        }
        let area = r.geometry.area();
        if area / f64::from(n) < 5.0 - 1e-9 {
            note("Sport England density", "S1377", "note", Some(r.id), format!("{n} pieces in {area:.0} m²: Sport England plans 5 m² a piece with its circulation"));
        }
        rooms.push(GymRoom { room: r.id, name: r.name.clone(), area, pieces: n, occupants: (area / (50.0 * 0.092_903_04) - 1e-9).ceil().max(0.0) as u32 });
    }
    findings.sort_by_key(|f| f.severity != "error");
    Ok(GymCheck { types: types.into_values().collect(), rooms, findings })
}

// ---- Layouts (`gym.layout`, `gym.studio`) ----

/// The tag on a layout's pieces: `{layout, row}`.
pub const LAYOUT_KEY: &str = "rxd:gym_layout";
/// An accessible route between rows (ADA §403.5.1).
const AISLE: f64 = 36.0 * IN;

/// The clear margins a piece keeps round its body when laid out: (sides, back, front), metres. A
/// treadmill's front is its step-on end; a strength station's 0.9 m is the ACSM's.
fn margins(kind: &str) -> (f64, f64, f64) {
    match kind {
        "Treadmill" => (0.5, 0.3, 2.0),
        "Rower" => (0.305, 0.15, 0.15),
        "DumbbellRack" => (0.3, 0.0, 0.91),
        "LegPress" | "CableCrossover" | "PowerRack" | "HalfRack" | "SmithMachine" => (0.9, 0.9, 0.9),
        // Circuit machines: 0.6 m (a consumer guide's minimum; 0.9 m preferred).
        _ => (0.6, 0.3, 0.6),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Piece {
    /// A catalogue item from the gym library.
    pub item: String,
    #[serde(default = "one")]
    pub count: u32,
}

fn one() -> u32 {
    1
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutParams {
    pub room_id: ElementRef,
    /// The pieces, in rows from the head wall in this order.
    pub pieces: Vec<Piece>,
    /// The head wall: the room's edge nearest this point (default its longest edge).
    pub head: Option<[f64; 2]>,
    /// Between rows, where the pieces' own zones leave less (default 36 in, an accessible route).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub aisle: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Placed {
    pub item: String,
    pub element: ElementId,
    pub row: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Layout {
    pub layout: String,
    pub placed: Vec<Placed>,
    /// Pieces that did not fit, by item.
    pub unplaced: BTreeMap<String, u32>,
    pub rows: u32,
}

/// A room's outline, level and floor height.
fn room_outline(store: &Store, r: &ElementRef) -> Result<(Vec<[f64; 2]>, String)> {
    let id = store.resolve(r).ok_or_else(|| bad("element.not_found", "no such room"))?;
    let info = crate::rooms::all(store).into_iter().find(|i| i.id == id).ok_or_else(|| bad("gym.not_a_room", "not a Riviera room"))?;
    let region = info.geometry.region.ok_or_else(|| bad("gym.not_enclosed", "the room is not enclosed"))?;
    let ring: Vec<[f64; 2]> = region.outer.iter().map(|e| e.line.start).collect();
    let level = store.get(id).and_then(crate::rooms::room_recipe).map(|r| r.level).unwrap_or_default();
    Ok((ring, level))
}

/// A frame on a room's head wall: its origin at the edge's start, `u` along it, `n` into the room.
fn head_frame(ring: &[[f64; 2]], head: Option<[f64; 2]>) -> ([f64; 2], [f64; 2], [f64; 2]) {
    let n = ring.len();
    let edge = |k: usize| (ring[k], ring[(k + 1) % n]);
    let dist = |p: [f64; 2], (a, b): ([f64; 2], [f64; 2])| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-12)).clamp(0.0, 1.0);
        (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dy * t)
    };
    let len = |(a, b): ([f64; 2], [f64; 2])| (b[0] - a[0]).hypot(b[1] - a[1]);
    let k = match head {
        Some(p) => (0..n).min_by(|i, j| dist(p, edge(*i)).total_cmp(&dist(p, edge(*j)))).unwrap_or(0),
        None => (0..n).max_by(|i, j| len(edge(*i)).total_cmp(&len(edge(*j)))).unwrap_or(0),
    };
    let (a, b) = edge(k);
    let l = len((a, b)).max(1e-12);
    let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
    let mut nn = [-u[1], u[0]];
    // Into the room: toward the ring's centroid.
    let c = ring.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0] / n as f64, s[1] + p[1] / n as f64]);
    if (c[0] - a[0]) * nn[0] + (c[1] - a[1]) * nn[1] < 0.0 {
        nn = [-nn[0], -nn[1]];
    }
    (a, u, nn)
}

/// Lays a gym out in rows from its head wall (`gym.layout`).
pub(crate) fn layout(txn: &mut Transaction<'_>, p: LayoutParams) -> Result<Layout> {
    let (ring, level) = room_outline(txn.store(), &p.room_id)?;
    let (o, u, n) = head_frame(&ring, p.head);
    let to_local = |q: [f64; 2]| [(q[0] - o[0]) * u[0] + (q[1] - o[1]) * u[1], (q[0] - o[0]) * n[0] + (q[1] - o[1]) * n[1]];
    let local_ring: Vec<[f64; 2]> = ring.iter().map(|q| to_local(*q)).collect();
    let (a0, a1) = local_ring.iter().fold((f64::MAX, f64::MIN), |(lo, hi), q| (lo.min(q[0]), hi.max(q[0])));
    let aisle = p.aisle.unwrap_or(AISLE);
    let names: Vec<String> = p.pieces.iter().map(|q| q.item.clone()).collect();
    let types = crate::furnishings::load(txn, crate::furnishings::LoadParams { names: names.clone() })?;
    let sizes = crate::interior_layouts::sizes();
    let store = txn.store().clone();
    let mut queue = Vec::new();
    for q in &p.pieces {
        let ty = types[&q.item];
        let kind = store.get(ty).and_then(|t| t.param("@ElementType")).and_then(Value::as_str).unwrap_or_default().to_owned();
        if !KINDS.iter().any(|k| k.0 == kind) {
            return Err(bad("gym.not_equipment", format!("{} is not gym equipment", q.item)));
        }
        for _ in 0..q.count {
            queue.push((q.item.clone(), ty, kind.clone(), sizes[&q.item]));
        }
    }
    // A cell inside the room: every corner of its body and margins.
    let fits = |x0: f64, x1: f64, y0: f64, y1: f64| [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].iter().all(|q| inside_or_on(*q, &local_ring));
    let layout = crate::id::GlobalId::new().as_str().to_owned();
    let mut placed = Vec::new();
    let mut unplaced: BTreeMap<String, u32> = BTreeMap::new();
    // Rows: `y` is where the row's bodies start (their backs), less their back margins.
    let (mut y, mut row, mut x, mut prev_side) = (0.0, 0u32, a0, 0.0);
    let (mut row_end, mut row_front) = (0.0f64, 0.0f64);
    let mut k = 0;
    while k < queue.len() {
        let (item, ty, kind, size) = &queue[k];
        let (w, l) = (size[0], size[1]);
        let (side, back, front) = margins(kind);
        // To the wall or the previous piece, whichever needs more.
        let gap = if x == a0 { side } else { side.max(prev_side) };
        let (bx0, by0) = (x + gap, y + back);
        let fits_here = fits(bx0 - side, bx0 + w + side, by0 - back, by0 + l + front);
        if fits_here {
            // Its back toward the head wall, its front into the room.
            let c = [bx0 + w / 2.0, by0 + l / 2.0];
            let point = [o[0] + u[0] * c[0] + n[0] * c[1], o[1] + u[1] * c[0] + n[1] * c[1]];
            // Its front (local −y) into the room, along `n`.
            let rotation = n[0].atan2(-n[1]);
            let level_ref = level_ref(&level)?;
            let id = crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(*ty), point, level: level_ref, offset: 0.0, rotation, mirror: false, scale: 1.0 })?;
            let tag = json!({ "layout": layout, "row": row });
            txn.modify(id, |e| {
                e.params.insert(LAYOUT_KEY.into(), tag);
            })?;
            placed.push(Placed { item: item.clone(), element: id, row });
            x = bx0 + w;
            prev_side = side;
            row_end = row_end.max(by0 + l);
            row_front = row_front.max(front);
            k += 1;
            continue;
        }
        if x == a0 {
            // Not even first in a row: it does not fit the room at this depth.
            *unplaced.entry(item.clone()).or_default() += 1;
            k += 1;
            if row_end == 0.0 {
                continue;
            }
        }
        // The next row, past this one's fronts and an aisle.
        if row_end == 0.0 && x == a0 {
            continue;
        }
        y = row_end + row_front.max(aisle);
        row += 1;
        x = a0;
        prev_side = 0.0;
        row_end = 0.0;
        row_front = 0.0;
        if y > local_ring.iter().fold(f64::MIN, |m, q| m.max(q[1])) {
            for (item, ..) in &queue[k..] {
                *unplaced.entry(item.clone()).or_default() += 1;
            }
            break;
        }
    }
    let rows = placed.iter().map(|q| q.row + 1).max().unwrap_or(0);
    let _ = a1;
    Ok(Layout { layout, placed, unplaced, rows })
}

fn level_ref(guid: &str) -> Result<ElementRef> {
    crate::id::GlobalId::parse(guid).map(ElementRef::Guid).ok_or_else(|| bad("level.missing", "the room's level no longer exists"))
}

fn inside_or_on(p: [f64; 2], poly: &[[f64; 2]]) -> bool {
    crate::sco::inside(p, poly) || (0..poly.len()).any(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2.max(1e-12)).clamp(0.0, 1.0);
        (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dy * t) < 1e-6
    })
}

/// What a studio is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Studio {
    /// Indoor cycling: 1.5 × 2.0 m a bike (Spinning).
    Cycle,
    /// Yoga and mat Pilates: 25 ft² a person.
    Mat,
    /// HIIT and functional classes: 50 ft² a person.
    Hiit,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StudioParams {
    pub room_id: ElementRef,
    pub kind: Studio,
    /// The instructor's wall: the room's edge nearest this point (default its longest edge).
    pub head: Option<[f64; 2]>,
    /// For cycling, place the bikes (default true).
    pub place: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StudioResult {
    pub area: f64,
    /// Places by the planning density.
    pub capacity: u32,
    /// The bikes placed, facing the instructor's wall.
    pub placed: Vec<ElementId>,
    pub note: String,
}

/// Sizes a studio, and lays out a cycling studio's bikes (`gym.studio`).
pub(crate) fn studio(txn: &mut Transaction<'_>, p: StudioParams) -> Result<StudioResult> {
    let (ring, level) = room_outline(txn.store(), &p.room_id)?;
    let area = crate::sketch::signed_area(&ring).abs();
    const FT2: f64 = 0.092_903_04;
    let (per, note) = match p.kind {
        Studio::Cycle => (3.0, "1.5 × 2.0 m a bike (Spinning); Les Mills plans about 2 m²"),
        Studio::Mat => (25.0 * FT2, "25 ft² a person for yoga and mat Pilates (20–25 ft²; dynamic styles 25–30)"),
        Studio::Hiit => (50.0 * FT2, "50 ft² a person for HIIT and functional classes (35–50 ft²)"),
    };
    let capacity = (area / per).floor() as u32;
    let mut placed = Vec::new();
    if p.kind == Studio::Cycle && p.place.unwrap_or(true) {
        let (o, u, n) = head_frame(&ring, p.head);
        let local: Vec<[f64; 2]> = ring.iter().map(|q| [(q[0] - o[0]) * u[0] + (q[1] - o[1]) * u[1], (q[0] - o[0]) * n[0] + (q[1] - o[1]) * n[1]]).collect();
        let (a0, a1) = local.iter().fold((f64::MAX, f64::MIN), |(lo, hi), q| (lo.min(q[0]), hi.max(q[0])));
        let d1 = local.iter().fold(f64::MIN, |m, q| m.max(q[1]));
        let ty = crate::furnishings::load(txn, crate::furnishings::LoadParams { names: vec!["Indoor cycle".into()] })?["Indoor cycle"];
        // The instructor's podium and its 2 m in front of the head wall, then 1.5 × 2.0 m cells.
        let (cw, cd, start) = (1.5, 2.0, 2.0);
        let cols = ((a1 - a0) / cw).floor() as i32;
        let rows = ((d1 - start) / cd).floor() as i32;
        let x0 = a0 + ((a1 - a0) - f64::from(cols) * cw) / 2.0;
        // Riders face the instructor: each bike's console (its +y) toward the head wall, its front
        // (−y) into the room.
        let rotation = n[0].atan2(-n[1]);
        for r in 0..rows {
            for c in 0..cols {
                let cx = x0 + (f64::from(c) + 0.5) * cw;
                let cy = start + (f64::from(r) + 0.5) * cd;
                let cell = [[cx - cw / 2.0, cy - cd / 2.0], [cx + cw / 2.0, cy - cd / 2.0], [cx + cw / 2.0, cy + cd / 2.0], [cx - cw / 2.0, cy + cd / 2.0]];
                if !cell.iter().all(|q| inside_or_on(*q, &local)) {
                    continue;
                }
                let point = [o[0] + u[0] * cx + n[0] * cy, o[1] + u[1] * cx + n[1] * cy];
                placed.push(crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(ty), point, level: level_ref(&level)?, offset: 0.0, rotation, mirror: false, scale: 1.0 })?);
            }
        }
    }
    Ok(StudioResult { area, capacity, placed, note: note.into() })
}
