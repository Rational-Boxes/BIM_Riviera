// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Commercial kitchens (`KITCHENS_PLAN.md` §6.2, §7.1, §7.3): equipment line-ups under their
//! hoods (ARC-KIT-003) and the kitchen check (ARC-KIT-004).
//!
//! The equipment is the Riviera Foodservice Equipment library ([`crate::furnishings`]): each type
//! carries `rxd:foodservice`, its role in the kitchen, its hood duty class (CMC 508.10, research/14
//! B6), whether it has open flames or makes grease-laden vapour, its cooking surface's height, and
//! whether it drains indirectly to a floor sink. A line-up places pieces back to the wall along a
//! line, puts a steel baffle between a fryer and an open flame, and hangs a wall-canopy hood
//! (`IfcAirTerminal` USERDEFINED "KitchenHood", the owner's answer to Q4) over the cooking
//! equipment and a Type II hood over a dish machine, sized and rated from the pieces under it.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use schemars::JsonSchema;

use crate::bath_components::Shape;
use crate::element::Element;
use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::mesh_objects::Category;
use crate::store::Store;
use crate::txn::Transaction;

/// What a piece of equipment is to the kitchen, on its type.
pub const KEY: &str = "rxd:foodservice";
/// A hood's sizing, on the hood.
pub const HOOD_KEY: &str = "rxd:hood";
/// The line-up a piece, baffle or hood belongs to: `{lineup}`.
pub const LINEUP_KEY: &str = "rxd:lineup";

const IN: f64 = 0.0254;
const FT: f64 = 0.3048;
const CFM: f64 = 0.000_471_947_443;
/// A hood's overhang past the cooking surface on its open sides (IMC 507.12 via Brighton, S1226).
const OVERHANG: f64 = 6.0 * IN;
/// The highest a canopy's lip may be over the cooking surface (IMC 507.11 via Brighton, S1226).
const LIP_MAX: f64 = 4.0 * FT;
/// A canopy's height, and the thickness of its sheet.
const CANOPY: f64 = 24.0 * IN;
const SHEET: f64 = 0.02;
/// A fryer's distance from an adjacent appliance's open flames (NFPA 96 summary, S1228).
const FRYER_GAP: f64 = 16.0 * IN;
const HOOD_STEEL: [f32; 4] = [0.72, 0.74, 0.77, 1.0];

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

/// Hood duty classes (CMC 508.10 lists, research/14 B6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Duty {
    Light,
    Medium,
    Heavy,
    ExtraHeavy,
}

impl Duty {
    /// A wall-mounted canopy's exhaust in cfm a linear foot (CMC tables 508.10.1.2–.5, S1225).
    pub fn wall_canopy_cfm(self) -> f64 {
        match self {
            Duty::Light => 200.0,
            Duty::Medium => 300.0,
            Duty::Heavy => 400.0,
            Duty::ExtraHeavy => 550.0,
        }
    }
}

/// A piece's `rxd:foodservice`.
#[derive(Debug, Clone, Deserialize)]
pub struct Info {
    pub role: String,
    #[serde(default)]
    pub duty: Option<Duty>,
    #[serde(default)]
    pub flame: bool,
    #[serde(default)]
    pub grease: bool,
    #[serde(default)]
    pub surface: f64,
    #[serde(default)]
    pub indirect: bool,
}

/// A copy's foodservice information, from its type.
pub fn info(store: &Store, e: &Element) -> Option<Info> {
    let (ty, _) = crate::mesh_objects::type_of(store, e)?;
    serde_json::from_value(store.get(ty)?.param(KEY)?.clone()).ok()
}

/// What `items` are placed in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    /// As listed.
    #[default]
    Given,
    /// The cooking equipment first, heaviest duty toward the middle of the hood (CEC design guide,
    /// S1227), then the rest as listed.
    Duty,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Hoods {
    /// A Type I or II canopy over the cooking equipment, and a Type II over a dish machine.
    #[default]
    Auto,
    None,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LineupParams {
    /// The line-up's back line, along the wall: from here …
    pub start: [f64; 2],
    /// … to here.
    pub end: [f64; 2],
    pub level: ElementRef,
    /// The pieces, by name in the Riviera Foodservice Equipment library (`furnishing.catalogue`),
    /// from the start.
    pub items: Vec<String>,
    #[serde(default)]
    pub order: Order,
    /// Between pieces (default none: a line-up stands side by side).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub gap: Option<f64>,
    /// A point in front of it; default away from the wall behind it.
    pub toward: Option<[f64; 2]>,
    #[serde(default)]
    pub hoods: Hoods,
    /// The hoods are listed (UL 710): makeup air at 80 % of exhaust, not 100 % (S1225); their
    /// exhaust is still the code table's until a product gives its listing's rate.
    #[serde(default)]
    pub listed: bool,
    /// The hoods' lip above the floor (default 78 in, 6 ft 6 in).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub lip: Option<f64>,
    /// A steel baffle between a fryer and an open flame closer than 16 in (default true).
    #[serde(default = "yes")]
    pub baffles: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Placed {
    pub name: String,
    pub element: ElementId,
    /// Along the line from its start.
    pub from: f64,
    pub to: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Hood {
    pub element: ElementId,
    /// "I" (grease) or "II" (heat and vapour).
    pub hood_type: String,
    pub style: String,
    pub duty: Option<Duty>,
    pub length: f64,
    pub depth: f64,
    /// Its lip above the floor.
    pub lip: f64,
    pub exhaust_cfm: f64,
    pub makeup_cfm: f64,
    pub listed: bool,
    /// What it is over.
    pub over: Vec<ElementId>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Lineup {
    /// The line-up's id (its parts' tag).
    pub lineup: String,
    pub length: f64,
    /// How much of it the pieces take.
    pub used: f64,
    pub placed: Vec<Placed>,
    pub baffles: Vec<ElementId>,
    pub hoods: Vec<Hood>,
    pub notes: Vec<String>,
}

/// A catalogue piece's plan extents: (x min, x max, y min, y max, top).
fn extents(s: &Shape) -> [f64; 5] {
    let mut b = [f64::MAX, f64::MIN, f64::MAX, f64::MIN, f64::MIN];
    for q in &s.p {
        b = [b[0].min(q[0]), b[1].max(q[0]), b[2].min(q[1]), b[3].max(q[1]), b[4].max(q[2])];
    }
    b
}

fn duty_info(name: &str) -> Option<Info> {
    let item = crate::furnishings::catalogue().into_iter().find(|i| i.name == name)?;
    item.params().into_iter().find(|(k, _)| *k == KEY).and_then(|(_, v)| serde_json::from_value(v).ok())
}

/// Lays out a line of foodservice equipment (`foodservice.lineup`).
pub(crate) fn lineup(txn: &mut Transaction<'_>, p: LineupParams) -> Result<Lineup> {
    if p.items.is_empty() {
        return Err(bad("foodservice.no_items", "name the pieces to line up"));
    }
    let store = txn.store().clone();
    let level = store.resolve(&p.level).and_then(|l| store.get(l)).ok_or_else(|| bad("element.not_found", "no such level"))?;
    let z0 = crate::levels::elevation(level);
    let (length, d, front, rotation) = crate::cabinets::line_frame(&store, p.start, p.end, z0, p.toward)?;
    let catalogue = crate::furnishings::catalogue();
    let mut names = Vec::new();
    for n in &p.items {
        if !catalogue.iter().any(|i| i.name == n) {
            return Err(bad("foodservice.no_item", format!("no piece {n:?}; see furnishing.catalogue with library foodservice")));
        }
        names.push(n.clone());
    }
    if p.order == Order::Duty {
        let weight = |n: &String| duty_info(n).and_then(|i| i.duty);
        let (mut cooking, rest): (Vec<String>, Vec<String>) = names.into_iter().partition(|n| weight(n).is_some());
        cooking.sort_by_key(|n| weight(n));
        // Lightest first: each heavier one alternately to the back and the front, so the heaviest
        // end up in the middle.
        let mut line = std::collections::VecDeque::new();
        for (k, n) in cooking.into_iter().enumerate() {
            if k % 2 == 0 { line.push_back(n) } else { line.push_front(n) }
        }
        names = line.into_iter().chain(rest).collect();
    }
    let gap = p.gap.unwrap_or(0.0).max(0.0);
    let infos: Vec<Option<Info>> = names.iter().map(|n| duty_info(n)).collect();
    let shapes: Vec<[f64; 5]> = names.iter().map(|n| extents(&catalogue.iter().find(|i| i.name == n).expect("checked").shape())).collect();
    let fryer_next_to_flame = |a: usize, b: usize| {
        let (x, y) = (infos[a].as_ref(), infos[b].as_ref());
        let fryer = |i: Option<&Info>| i.is_some_and(|i| i.role == "fryer");
        let flame = |i: Option<&Info>| i.is_some_and(|i| i.flame);
        (fryer(x) && flame(y)) || (flame(x) && fryer(y))
    };
    // Where each piece goes along the line, and the baffles between.
    let baffle_w = 0.006;
    let mut spans = Vec::new();
    let mut baffle_at = Vec::new();
    let mut cursor = 0.0;
    for k in 0..names.len() {
        if k > 0 {
            cursor += gap;
            if p.baffles && gap < FRYER_GAP && fryer_next_to_flame(k - 1, k) {
                baffle_at.push((cursor + baffle_w / 2.0, if infos[k].as_ref().is_some_and(|i| i.role == "fryer") { k } else { k - 1 }));
                cursor += baffle_w;
            }
        }
        let w = shapes[k][1] - shapes[k][0];
        spans.push((cursor, cursor + w));
        cursor += w;
    }
    if cursor > length + 1e-6 {
        return Err(bad("foodservice.too_long", format!("the pieces take {cursor:.3} m and the line is {length:.3} m: {:.3} m too long", cursor - length)));
    }
    let types = crate::furnishings::load(txn, crate::furnishings::LoadParams { names: names.clone() })?;
    let lineup = crate::id::GlobalId::new().as_str().to_owned();
    let tag = json!({ "lineup": lineup });
    // A piece's local x runs along the line one way or the other, and its local y away from the
    // front: its origin goes where its back lands on the line.
    let along_sign = (rotation.cos() * d[0] + rotation.sin() * d[1]).signum();
    let at = |c: f64, out: f64| [p.start[0] + d[0] * c + front[0] * out, p.start[1] + d[1] * c + front[1] * out];
    let place = |txn: &mut Transaction<'_>, ty: ElementId, from: f64, to: f64, e: [f64; 5], offset: f64| -> Result<ElementId> {
        let xc = (e[0] + e[1]) / 2.0;
        let c = (from + to) / 2.0 - along_sign * xc;
        let id = crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(ty), point: at(c, e[3]), level: p.level.clone(), offset, rotation, mirror: false, scale: 1.0 })?;
        txn.modify(id, |el| {
            el.params.insert(LINEUP_KEY.into(), tag.clone());
        })?;
        Ok(id)
    };
    let mut placed = Vec::new();
    for (k, n) in names.iter().enumerate() {
        let id = place(txn, types[n], spans[k].0, spans[k].1, shapes[k], 0.0)?;
        placed.push(Placed { name: n.clone(), element: id, from: spans[k].0, to: spans[k].1 });
    }
    let mut baffles = Vec::new();
    if !baffle_at.is_empty() {
        let ty = crate::furnishings::load(txn, crate::furnishings::LoadParams { names: vec!["Fryer baffle".into()] })?["Fryer baffle"];
        let e = extents(&catalogue.iter().find(|i| i.name == "Fryer baffle").expect("in the catalogue").shape());
        for (c, fryer) in baffle_at {
            // Centred on the fryer's depth.
            let fryer_d = shapes[fryer][3] - shapes[fryer][2];
            let id = place(txn, ty, c - baffle_w / 2.0, c + baffle_w / 2.0, [e[0], e[1], e[2], e[3] + ((fryer_d - (e[3] - e[2])) / 2.0).max(0.0), e[4]], 0.0)?;
            baffles.push(id);
        }
    }
    // The hoods.
    let mut hoods = Vec::new();
    let mut notes = Vec::new();
    let lip = p.lip.unwrap_or(78.0 * IN);
    if p.hoods == Hoods::Auto {
        let cooking: Vec<usize> = (0..names.len()).filter(|k| infos[*k].as_ref().is_some_and(|i| i.duty.is_some())).collect();
        if !cooking.is_empty() {
            let grease = cooking.iter().any(|k| infos[*k].as_ref().is_some_and(|i| i.grease));
            let duty = cooking.iter().filter_map(|k| infos[*k].as_ref().and_then(|i| i.duty)).max();
            let (a, b) = (cooking.iter().map(|k| spans[*k].0).fold(f64::MAX, f64::min), cooking.iter().map(|k| spans[*k].1).fold(f64::MIN, f64::max));
            let deep = cooking.iter().map(|k| shapes[*k][3] - shapes[*k][2]).fold(0.0, f64::max);
            let surface = cooking.iter().filter_map(|k| infos[*k].as_ref().map(|i| i.surface)).fold(0.0, f64::max);
            if surface > 0.0 && lip - surface > LIP_MAX + 1e-9 {
                notes.push(format!("the hood's lip is {:.0} in over the cooking surface; a canopy's may be 4 ft at most (IMC 507.11)", (lip - surface) / IN));
            }
            let over = cooking.iter().map(|k| placed[*k].element).collect();
            hoods.push(hood(txn, &p, &tag, &at, rotation, (a - OVERHANG, b + OVERHANG), deep + OVERHANG, lip, if grease { "I" } else { "II" }, if grease { duty } else { None }, over)?);
            if names.iter().any(|n| n.starts_with("Combi oven")) {
                notes.push("a combi oven's steam surges at its door: UK guidance (DW/172 notes) asks 600 mm of hood past its front, more than the 6 in here".into());
            }
        }
        for k in (0..names.len()).filter(|k| infos[*k].as_ref().is_some_and(|i| i.role == "dish_machine")) {
            let deep = shapes[k][3] - shapes[k][2];
            hoods.push(hood(txn, &p, &tag, &at, rotation, (spans[k].0 - OVERHANG, spans[k].1 + OVERHANG), deep + OVERHANG, lip, "II", None, vec![placed[k].element])?);
        }
    }
    Ok(Lineup { lineup, length, used: cursor, placed, baffles, hoods, notes })
}

/// A wall canopy along the line from `span.0` to `span.1`, `depth` out from the wall.
#[allow(clippy::too_many_arguments)]
fn hood(txn: &mut Transaction<'_>, p: &LineupParams, tag: &Value, at: &dyn Fn(f64, f64) -> [f64; 2], rotation: f64, span: (f64, f64), depth: f64, lip: f64, kind: &str, duty: Option<Duty>, over: Vec<ElementId>) -> Result<Hood> {
    let length = span.1 - span.0;
    // Type I by its heaviest duty class; Type II, which the tables do not cover, at the light
    // duty rate (EXPERT_REVIEW E-139).
    let per_ft = duty.unwrap_or(Duty::Light).wall_canopy_cfm();
    let exhaust_cfm = (per_ft * length / FT).round();
    let makeup_cfm = (exhaust_cfm * if p.listed { 0.8 } else { 1.0 }).round();
    let (w, dd) = (length / 2.0, depth / 2.0);
    // Open below: a top, a front and two ends, its back on the wall.
    let sh = Shape::default()
        .cuboid([-w, w], [-dd, dd], [CANOPY - SHEET, CANOPY])
        .cuboid([-w, w], [-dd, -dd + SHEET], [0.0, CANOPY - SHEET])
        .cuboid([-w, -w + SHEET], [-dd + SHEET, dd], [0.0, CANOPY - SHEET])
        .cuboid([w - SHEET, w], [-dd + SHEET, dd], [0.0, CANOPY - SHEET]);
    // A round collar at 1,500 fpm (the CEC's recommended grease-duct velocity, S1227).
    let area = exhaust_cfm / 1500.0 * FT * FT;
    let collar = (4.0 * area / std::f64::consts::PI).sqrt();
    let connectors = vec![
        crate::mep::duct("Exhaust", [0.0, dd / 2.0, CANOPY], [0.0, 0.0, 1.0], crate::mep::Classification::ExhaustAir, crate::mep::FlowDirection::Out, crate::mep::FlowConfig::Preset, exhaust_cfm * CFM, collar),
        crate::mep::power("Lights", [w - 0.1, dd, CANOPY / 2.0], [0.0, 1.0, 0.0], crate::mep::FlowDirection::In, 120.0, 1, 200.0, "Lighting"),
    ];
    let connectors = connectors.into_iter().map(crate::mep::validate).collect::<Result<Vec<_>>>()?;
    let name = format!("Kitchen hood, Type {kind} wall canopy {:.0} × {:.0}", length * 1000.0, depth * 1000.0);
    let existing = txn.store().iter().find(|e| e.name.as_deref() == Some(name.as_str()) && crate::mesh_objects::mesh_object_type(e).is_some()).map(Element::id);
    let ty = match existing {
        Some(t) => t,
        None => {
            let t = crate::mesh_objects::create_generated_type(txn, &name, Category::AirTerminal, &sh.p, &sh.t, HOOD_STEEL, connectors, Some("USERDEFINED"))?;
            txn.modify(t, |e| {
                e.params.insert("@ElementType".into(), "KitchenHood".into());
                e.params.insert("@Description".into(), format!("A Type {kind} kitchen hood, a wall-mounted canopy (KITCHENS_PLAN §7.1); hoods have no IFC class, so it is an IfcAirTerminal USERDEFINED as BDNS maps them.").into());
            })?;
            t
        }
    };
    let c = (span.0 + span.1) / 2.0;
    let id = crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(ty), point: at(c, dd), level: p.level.clone(), offset: lip, rotation, mirror: false, scale: 1.0 })?;
    let h = Hood { element: id, hood_type: kind.into(), style: "wall_canopy".into(), duty, length, depth, lip, exhaust_cfm, makeup_cfm, listed: p.listed, over };
    let value = serde_json::to_value(&h).expect("serialises");
    txn.modify(id, |e| {
        e.params.insert(HOOD_KEY.into(), value);
        e.params.insert(LINEUP_KEY.into(), tag.clone());
    })?;
    Ok(h)
}

// ---- The check (`foodservice.check`) ----

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckParams {
    /// The farthest a workstation may be from a hand sink (default 25 ft, Maricopa; CUPHD asks
    /// 15 ft in the same room).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub hand_sink_distance: Option<f64>,
    /// The aisle kept clear in front of cooking equipment (default 42 in).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub aisle: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Finding {
    pub rule: String,
    pub source: String,
    /// "error", "warning" or "note".
    pub severity: String,
    pub element: Option<ElementId>,
    pub message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct KitchenCheck {
    /// The pieces of equipment checked.
    pub pieces: u32,
    pub hoods: Vec<Hood>,
    /// Exhaust and makeup air over all the hoods (cfm).
    pub exhaust_cfm: f64,
    pub makeup_cfm: f64,
    pub findings: Vec<Finding>,
}

fn plan(it: &crate::rules::Item, q: &[[f64; 2]]) -> Vec<[f64; 2]> {
    q.iter().map(|q| it.frame.apply([q[0], q[1], 0.0])).map(|w| [w[0], w[1]]).collect()
}

fn rect(it: &crate::rules::Item, x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<[f64; 2]> {
    plan(it, &[[x0, y0], [x1, y0], [x1, y1], [x0, y1]])
}

fn foot(it: &crate::rules::Item) -> Vec<[f64; 2]> {
    rect(it, it.x[0], it.x[1], it.front, it.back)
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = vx * vx + vy * vy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * vx + (p[1] - a[1]) * vy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (p[0] - a[0] - t * vx).hypot(p[1] - a[1] - t * vy)
}

/// The gap between two convex outlines in plan; zero when they touch or overlap.
fn gap(a: &[[f64; 2]], b: &[[f64; 2]]) -> f64 {
    if a.iter().any(|q| crate::sco::inside(*q, b)) || b.iter().any(|q| crate::sco::inside(*q, a)) {
        return 0.0;
    }
    let mut best = f64::MAX;
    for (p, q) in [(a, b), (b, a)] {
        for v in p {
            for k in 0..q.len() {
                best = best.min(seg_dist(*v, q[k], q[(k + 1) % q.len()]));
            }
        }
    }
    best
}

fn centre(poly: &[[f64; 2]]) -> [f64; 2] {
    let n = poly.len() as f64;
    [poly.iter().map(|q| q[0]).sum::<f64>() / n, poly.iter().map(|q| q[1]).sum::<f64>() / n]
}

/// Whether `inner` lies within `outer` (convex), to a millimetre.
fn within(inner: &[[f64; 2]], outer: &[[f64; 2]]) -> bool {
    inner.iter().all(|q| crate::sco::inside(*q, outer) || outer.iter().enumerate().any(|(k, a)| seg_dist(*q, *a, outer[(k + 1) % outer.len()]) < 1e-3))
}

/// Checks the commercial kitchens' equipment (`foodservice.check`).
pub(crate) fn check(store: &Store, p: Option<CheckParams>) -> Result<KitchenCheck> {
    let p = p.unwrap_or_default();
    let hand_limit = p.hand_sink_distance.unwrap_or(25.0 * FT);
    let aisle = p.aisle.unwrap_or(42.0 * IN);
    let mut pieces: Vec<(Info, crate::rules::Item)> = Vec::new();
    let mut hoods: Vec<(Hood, crate::rules::Item)> = Vec::new();
    for e in store.iter() {
        if let Some(h) = e.param(HOOD_KEY).and_then(|v| serde_json::from_value::<Hood>(v.clone()).ok()) {
            if let Some(it) = crate::rules::item(store, e) {
                hoods.push((Hood { element: e.id(), ..h }, it));
            }
            continue;
        }
        if let Some(i) = info(store, e)
            && let Some(it) = crate::rules::item(store, e)
        {
            pieces.push((i, it));
        }
    }
    let mut findings = Vec::new();
    let mut note = |rule: &str, source: &str, severity: &str, element: Option<ElementId>, message: String| {
        findings.push(Finding { rule: rule.into(), source: source.into(), severity: severity.into(), element, message });
    };
    // A piece by its type's name ("Fryer, 40 lb gas"), else as the overlap tools name it.
    let name = |id: ElementId| {
        store.get(id).and_then(|e| crate::mesh_objects::type_of(store, e)).and_then(|(t, _)| store.get(t)?.name.clone()).unwrap_or_else(|| crate::sco::name(store, id))
    };
    let same_floor = |a: &crate::rules::Item, b: &crate::rules::Item| (a.floor - b.floor).abs() < 0.5;
    let of_role = |role: &str| pieces.iter().filter(move |(i, _)| i.role == role).map(|(_, it)| it).collect::<Vec<_>>();
    let equipment: Vec<&(Info, crate::rules::Item)> = pieces.iter().filter(|(i, _)| !matches!(i.role.as_str(), "baffle" | "floor_sink" | "extinguisher")).collect();
    // Hoods over the cooking equipment, with their overhang, and their lips.
    for (i, it) in pieces.iter().filter(|(i, _)| i.duty.is_some() || i.role == "dish_machine") {
        let need = if i.grease { "I" } else if i.role == "dish_machine" { "II" } else { "I or II" };
        let wanted = rect(it, it.x[0] - OVERHANG, it.x[1] + OVERHANG, it.front - OVERHANG, it.back);
        let covering: Vec<&(Hood, crate::rules::Item)> = hoods.iter().filter(|(h, hi)| same_floor(it, hi) && (!i.grease || h.hood_type == "I") && gap(&foot(it), &foot(hi)) == 0.0).collect();
        match covering.iter().find(|(_, hi)| within(&wanted, &foot(hi))) {
            Some((h, hi)) => {
                if i.surface > 0.0 && hi.z[0] - it.floor - i.surface > LIP_MAX + 1e-6 {
                    note("Hood height", "S1226", "error", Some(h.element), format!("the hood's lip is {:.0} in over {}'s cooking surface; a canopy's may be 4 ft at most (IMC 507.11)", (hi.z[0] - it.floor - i.surface) / IN, name(it.element)));
                }
            }
            None if !covering.is_empty() => note("Hood overhang", "S1226", "error", Some(it.element), format!("the hood over {} passes it by less than 6 in on an open side (IMC 507.12)", name(it.element))),
            None if i.role == "dish_machine" => note("Type II hood", "S1217", "warning", Some(it.element), format!("{} has no Type II hood over it: a high-temperature machine needs one unless it is ventless (Maricopa)", name(it.element))),
            None => note("Type I hood", "S1226 S1231", if i.grease { "error" } else { "warning" }, Some(it.element), format!("{} has no Type {need} hood over it (IMC 507.2)", name(it.element))),
        }
    }
    // Fryers 16 in from open flames, unless a baffle stands between.
    let baffles = of_role("baffle");
    for f in of_role("fryer") {
        for (_, o) in pieces.iter().filter(|(i, o)| i.flame && o.element != f.element && same_floor(f, o)) {
            let g = gap(&foot(f), &foot(o));
            if g < FRYER_GAP - 1e-6 && !baffles.iter().any(|b| gap(&foot(b), &foot(f)) < 0.01 && gap(&foot(b), &foot(o)) < 0.01) {
                note("Fryer separation", "S1228", "error", Some(f.element), format!("{} is {:.0} in from {}'s open flames: keep 16 in or put a steel baffle between (NFPA 96)", name(f.element), g / IN, name(o.element)));
            }
        }
    }
    // Hand sinks within reach of each workstation (straight line, not yet by path).
    let hand = of_role("hand_sink");
    let stations = ["cooking", "fryer", "prep", "prep_sink", "ware_sink", "dish_machine", "dish_table", "ice"];
    if hand.is_empty() && equipment.iter().any(|(i, _)| stations.contains(&i.role.as_str())) {
        note("Hand sinks", "S1216", "error", None, "the kitchen has no hand sink: at least one is required, and more for convenient use in prep, dispensing and ware-washing areas (Food Code 5-203.11)".into());
    } else {
        for (_, it) in equipment.iter().filter(|(i, _)| stations.contains(&i.role.as_str())) {
            let near = hand.iter().filter(|h| same_floor(it, h)).map(|h| gap(&foot(it), &foot(h))).fold(f64::MAX, f64::min);
            if near > hand_limit + 1e-6 {
                note("Hand sink distance", "S1217 S1220", "warning", Some(it.element), format!("{} is {:.0} ft from the nearest hand sink; Maricopa asks about {:.0} ft (CUPHD 15 ft in the same room)", name(it.element), near / FT, hand_limit / FT));
            }
        }
    }
    // A Class K extinguisher within 30 ft of each cooking appliance (straight line, not travel).
    let ext = of_role("extinguisher");
    for (_, it) in pieces.iter().filter(|(i, _)| i.duty.is_some()) {
        let near = ext.iter().filter(|x| same_floor(it, x)).map(|x| gap(&foot(it), &foot(x))).fold(f64::MAX, f64::min);
        if near > 30.0 * FT {
            note("Class K extinguisher", "S1230", "error", Some(it.element), format!("{} has no Class K extinguisher within 30 ft (IFC 904.12.5)", name(it.element)));
        }
    }
    // A floor sink at each indirect waste.
    let floor_sinks = of_role("floor_sink");
    for (_, it) in pieces.iter().filter(|(i, _)| i.indirect) {
        let near = floor_sinks.iter().filter(|x| same_floor(it, x)).map(|x| centre(&foot(x))).map(|c| if crate::sco::inside(c, &foot(it)) { 0.0 } else { gap(&foot(it), &[c]) }).fold(f64::MAX, f64::min);
        if near > 5.0 * FT {
            note("Floor sink", "S1217 S1216", "warning", Some(it.element), format!("{} drains indirectly, but no floor sink is within 5 ft of it (Food Code 5-402.11; Maricopa)", name(it.element)));
        }
    }
    // A mop sink.
    if !equipment.is_empty() && of_role("mop_sink").is_empty() {
        note("Mop sink", "S1216", "error", None, "the kitchen has no mop sink or curbed cleaning area (Food Code 5-203.13)".into());
    }
    // The aisle in front of cooking equipment.
    for (_, it) in pieces.iter().filter(|(i, _)| i.duty.is_some()) {
        let clear = rect(it, it.x[0], it.x[1], it.front - aisle, it.front - 0.01);
        for (_, o) in equipment.iter().filter(|(_, o)| o.element != it.element && same_floor(it, o)) {
            if gap(&clear, &foot(o)) == 0.0 {
                note("Aisle", "S1222 S1247", "warning", Some(it.element), format!("{} is within {:.0} in of {}'s front: keep a {:.0} in work aisle (protruding doors and kettles want 42–54 in)", name(o.element), aisle / IN, name(it.element), aisle / IN));
            }
        }
    }
    findings.sort_by_key(|f| match f.severity.as_str() {
        "error" => 0,
        "warning" => 1,
        _ => 2,
    });
    let hoods: Vec<Hood> = hoods.into_iter().map(|(h, _)| h).collect();
    Ok(KitchenCheck { pieces: equipment.len() as u32, exhaust_cfm: hoods.iter().map(|h| h.exhaust_cfm).sum(), makeup_cfm: hoods.iter().map(|h| h.makeup_cfm).sum(), hoods, findings })
}
