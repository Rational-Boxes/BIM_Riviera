// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A pool's treatment plant as equipment (ARC-POOL-014): the balance tank, the circulation pumps,
//! the filters and the chemical tanks sized by `pool.treatment` (ARC-POOL-009) and set out in a
//! row in the plant room.
//!
//! - **Filters**: one vertical vessel each, its diameter from its share of the filter area,
//!   1.8 m high.
//! - **Pumps**: a duty and a standby pump, each for the whole flow against `head` (default 15 m:
//!   the filters, strainer and pipework), at 60 % efficiency; the motor the next standard size up.
//! - **Balance tank**: the surge or balance volume 1.2 m deep, 1.5 m high, twice as long as wide.
//! - **Chemicals**: a 200 L sodium hypochlorite tank and a 200 L acid tank, each in its bund,
//!   kept 2 m apart (Sport England: store acid and chlorine apart).
//!
//! The row stands 0.5 m off the room's back wall, 0.8 m between items and 0.6 m from the ends,
//! with 1.0 m in front for access. Each item is a generated object type, so it can be scheduled
//! and saved as `IfcTank`, `IfcPump` and `IfcFilter`; running it again replaces the pool's plant.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::bath_components::Shape;
use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::mesh_objects::Category;
use crate::pool_treatment::{Basis, TreatmentParams};
use crate::store::Store;
use crate::txn::Transaction;

/// Marks a placed plant item with its pool's GlobalId.
const PLANT_KEY: &str = "rxd:pool_plant";
const BACK: f64 = 0.5;
const GAP: f64 = 0.8;
const END: f64 = 0.6;
const FRONT: f64 = 1.0;
const FILTER_HEIGHT: f64 = 1.8;
const EFFICIENCY: f64 = 0.6;
/// Standard motor ratings, kW (IEC 60072).
const MOTORS: [f64; 22] = [0.37, 0.55, 0.75, 1.1, 1.5, 2.2, 3.0, 4.0, 5.5, 7.5, 11.0, 15.0, 18.5, 22.0, 30.0, 37.0, 45.0, 55.0, 75.0, 90.0, 110.0, 132.0];

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlantParams {
    /// The pool; the only one when omitted.
    pub pool_id: Option<ElementRef>,
    /// A room to set the plant out in: along its longer side from its lower corner, and its area
    /// checked. Or give `level` and `point`.
    pub room: Option<ElementRef>,
    pub level: Option<ElementRef>,
    /// Where the row's back left corner goes; a point inside a room on `level` takes that room.
    pub point: Option<[f64; 2]>,
    /// The row's direction, counter-clockwise from +x (default 0, or along the room).
    #[schemars(extend("x-riviera-data-type" = "angle"))]
    pub rotation: Option<f64>,
    /// As `pool.treatment`.
    pub basis: Option<Basis>,
    pub filters: Option<u32>,
    /// The pumps' head, m (default 15).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub head: Option<f64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PlantItem {
    pub element: ElementId,
    /// balance_tank, pump, filter, hypochlorite or acid.
    pub role: String,
    pub name: String,
    /// Length along the row, depth, height.
    pub size: [f64; 3],
    /// Its centre in plan.
    pub at: [f64; 2],
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Plant {
    pub pool: ElementId,
    pub basis: Basis,
    /// m³/h, the pumps' head (m), the power the water takes over the efficiency (kW), and the
    /// motor chosen.
    pub flow: f64,
    pub head: f64,
    pub pump_power: f64,
    pub motor: f64,
    pub filter_diameter: f64,
    pub balance_tank: f64,
    pub items: Vec<PlantItem>,
    /// The row's length and depth with its clearances, and their area.
    pub footprint: [f64; 2],
    pub area: f64,
    /// m², the range `pool.treatment` gives.
    pub plant_room: [f64; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<ElementId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_area: Option<f64>,
    pub notes: Vec<String>,
}

fn bad(code: &'static str, m: impl Into<String>) -> Error {
    Error::new(code, m.into())
}

fn up(v: f64, step: f64) -> f64 {
    (v / step - 1e-9).ceil() * step
}

fn circle(r: f64) -> Vec<[f64; 2]> {
    (0..24).map(|k| {
        let a = std::f64::consts::TAU * f64::from(k) / 24.0;
        [r * a.cos(), r * a.sin()]
    }).collect()
}

/// What goes in the row: role, type name, category, colour, shape (centred on its footprint,
/// from z = 0), and its length, depth and height.
struct Spec {
    role: &'static str,
    name: String,
    category: Category,
    color: [f32; 4],
    shape: Shape,
    size: [f64; 3],
}

fn tank(role: &'static str, name: String, l: f64, d: f64, h: f64) -> Spec {
    Spec { role, name, category: Category::Tank, color: [0.30, 0.45, 0.60, 1.0], shape: Shape::default().cuboid([-l / 2.0, l / 2.0], [-d / 2.0, d / 2.0], [0.0, h]), size: [l, d, h] }
}

fn drum(role: &'static str, name: &str, color: [f32; 4]) -> Spec {
    // A 200 L drum, 0.6 m across and 0.9 m high, in a 0.8 m square bund 0.15 m high.
    let shape = Shape::default().cuboid([-0.4, 0.4], [-0.4, 0.4], [0.0, 0.15]).prism(&circle(0.3), 0.15, 1.05);
    Spec { role, name: name.to_owned(), category: Category::Tank, color, shape, size: [0.8, 0.8, 1.05] }
}

/// Lays out and places the pool's plant (`pool.plant`); it replaces any plant placed before.
pub(crate) fn plant(txn: &mut Transaction<'_>, p: PlantParams) -> Result<Plant> {
    let store = txn.store().clone();
    let pool = match &p.pool_id {
        Some(r) => store.resolve(r).ok_or_else(|| bad("element.not_found", "no such pool"))?,
        None => {
            let all: Vec<ElementId> = crate::pools::all(&store).into_iter().map(|(id, _)| id).collect();
            match all.as_slice() {
                [one] => *one,
                [] => return Err(bad("pool.none", "the model has no pool")),
                _ => return Err(bad("pool.which", "the model has more than one pool: give pool_id")),
            }
        }
    };
    let t = crate::pool_treatment::treatment(&store, Some(TreatmentParams { pool_id: Some(ElementRef::Id(pool)), basis: p.basis, turnover: None, filter_rate: None, filters: p.filters }))?
        .pop()
        .ok_or_else(|| bad("pool.not_a_pool", format!("element {} is not a pool", pool.0)))?;
    let head = p.head.unwrap_or(15.0);
    if head <= 0.0 {
        return Err(bad("pool.bad_plant", "the pumps' head must be above zero"));
    }
    let mut notes = Vec::new();

    // Where: a room's lower corner along its longer side, or the point given.
    let in_room = match (&p.room, &p.level, p.point) {
        (None, Some(l), Some(pt)) => store.resolve(l).and_then(|l| crate::rooms::room_at(&store, l, pt)).map(ElementRef::Id),
        _ => None,
    };
    let (level, origin, rotation, room, room_box) = match (p.room.as_ref().or(in_room.as_ref()), &p.level, p.point) {
        (Some(r), _, _) => {
            let id = store.resolve(r).ok_or_else(|| bad("element.not_found", "no such room"))?;
            let e = store.get(id).expect("resolved");
            let rec = crate::rooms::room_recipe(e).ok_or_else(|| bad("room.not_a_room", format!("element {} is not a room", id.0)))?;
            let g = crate::rooms::regenerate(&store, e)?;
            let region = g.region.as_ref().ok_or_else(|| bad("room.not_enclosed", "the room is not enclosed"))?;
            let ring = region.outer_ring();
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for q in &ring {
                lo = [lo[0].min(q[0]), lo[1].min(q[1])];
                hi = [hi[0].max(q[0]), hi[1].max(q[1])];
            }
            let level = crate::id::GlobalId::parse(&rec.level).and_then(|g| store.resolve(&ElementRef::Guid(g))).ok_or_else(|| bad("element.not_found", "the room's level is gone"))?;
            let (w, d) = (hi[0] - lo[0], hi[1] - lo[1]);
            // Along x with the south wall behind; or along y with the east wall behind.
            let (origin, rot, along, across) = if w >= d { (lo, 0.0, w, d) } else { ([hi[0], lo[1]], std::f64::consts::FRAC_PI_2, d, w) };
            (level, origin, p.rotation.unwrap_or(rot), Some((id, g.area())), Some([along, across]))
        }
        (None, Some(l), Some(pt)) => (store.resolve(l).ok_or_else(|| bad("element.not_found", "no such level"))?, pt, p.rotation.unwrap_or(0.0), None, None),
        _ => return Err(bad("pool.bad_plant", "give a room, or a level and a point")),
    };

    // What.
    let flow = t.flow;
    let pump_power = 1000.0 * 9.81 * flow / 3600.0 * head / EFFICIENCY / 1000.0;
    let motor = MOTORS.iter().copied().find(|m| *m >= pump_power - 1e-9).unwrap_or_else(|| up(pump_power, 10.0));
    let filter_diameter = up((4.0 * t.filter_area_each / std::f64::consts::PI).sqrt(), 0.05);
    if filter_diameter > 3.0 {
        notes.push(format!("each filter is {filter_diameter:.2} m across; over 3 m, use more filters (filters) or horizontal vessels"));
    }
    let balance = t.surge_volume;
    let area = (balance / 1.2).max(0.64);
    let tank_w = (area / 2.0).sqrt().max(0.8);
    let tank_l = area / tank_w;
    let mut specs = vec![tank("balance_tank", format!("Pool balance tank {balance:.1} m³"), up(tank_l, 0.1), up(tank_w, 0.1), 1.5)];
    let (pl, pw, ph) = (0.6 + 0.03 * motor, 0.3 + 0.008 * motor, 0.35 + 0.01 * motor);
    for duty in ["duty", "standby"] {
        let shape = Shape::default().cuboid([-pl / 2.0, pl / 2.0], [-pw / 2.0, pw / 2.0], [0.0, 0.05]).cuboid([-pl / 2.0 + 0.05, pl / 2.0 - 0.05], [-pw / 2.0 + 0.03, pw / 2.0 - 0.03], [0.05, ph]);
        specs.push(Spec { role: "pump", name: format!("Pool circulation pump {motor} kW ({duty})"), category: Category::Pump, color: [0.22, 0.47, 0.75, 1.0], shape, size: [pl, pw, ph] });
    }
    for k in 0..t.filters {
        let shape = Shape::default().prism(&circle(filter_diameter / 2.0), 0.0, FILTER_HEIGHT);
        specs.push(Spec { role: "filter", name: format!("Pool filter Ø{:.0} mm ({})", filter_diameter * 1000.0, k + 1), category: Category::Filter, color: [0.35, 0.55, 0.40, 1.0], shape, size: [filter_diameter, filter_diameter, FILTER_HEIGHT] });
    }
    specs.push(drum("hypochlorite", "Sodium hypochlorite tank 200 L", [0.85, 0.80, 0.30, 1.0]));
    specs.push(drum("acid", "Acid tank 200 L", [0.80, 0.35, 0.30, 1.0]));

    // Replace the pool's plant placed before.
    let guid = store.get(pool).expect("resolved").guid().to_string();
    for e in store.iter().filter(|e| e.params.get(PLANT_KEY).and_then(serde_json::Value::as_str) == Some(guid.as_str())) {
        txn.delete(e.id())?;
    }

    // Set out along the row: 0.5 m off the back, 0.8 m between items, the two chemicals 2 m
    // apart.
    let (ux, uy) = ([rotation.cos(), rotation.sin()], [-rotation.sin(), rotation.cos()]);
    let depth = specs.iter().map(|s| s.size[1]).fold(0.0, f64::max);
    let mut x = END;
    let mut items = Vec::new();
    for (k, s) in specs.iter().enumerate() {
        if k > 0 {
            x += if s.role == "acid" { 2.0 } else { GAP };
        }
        let c = [x + s.size[0] / 2.0, BACK + depth / 2.0];
        let at = [origin[0] + ux[0] * c[0] + uy[0] * c[1], origin[1] + ux[1] * c[0] + uy[1] * c[1]];
        x += s.size[0];
        let ty = match store.iter().find(|e| e.name.as_deref() == Some(s.name.as_str()) && crate::mesh_objects::mesh_object_type(e).is_some()) {
            Some(e) => e.id(),
            None => {
                let predefined = match s.category {
                    Category::Tank => "STORAGE",
                    Category::Filter => "WATERFILTER",
                    _ => "NOTDEFINED",
                };
                crate::mesh_objects::create_generated_type(txn, &s.name, s.category, &s.shape.p, &s.shape.t, s.color, Vec::new(), Some(predefined))?
            }
        };
        let element = crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(ty), point: at, level: ElementRef::Id(level), offset: 0.0, rotation, mirror: false, scale: 1.0 })?;
        let g = guid.clone();
        txn.modify(element, |e| {
            e.params.insert(PLANT_KEY.into(), g.into());
        })?;
        items.push(PlantItem { element, role: s.role.into(), name: s.name.clone(), size: s.size, at });
    }
    let footprint = [x + END, BACK + depth + FRONT];
    let area = footprint[0] * footprint[1];

    // The room: its area against the range, and whether the row fits.
    let room_area = room.map(|(_, a)| a);
    if let Some(a) = room_area {
        if a < t.plant_room[0] - 1e-6 {
            notes.push(format!("the plant room is {a:.1} m²; Sport England asks for 15–30 % of the water ({:.0}–{:.0} m²)", t.plant_room[0], t.plant_room[1]));
        }
        if let Some([along, across]) = room_box
            && (footprint[0] > along + 1e-6 || footprint[1] > across + 1e-6)
        {
            notes.push(format!("the plant's row needs {:.1} × {:.1} m with its clearances; the room is {along:.1} × {across:.1} m", footprint[0], footprint[1]));
        }
    } else if area > t.plant_room[1] {
        notes.push(format!("the row takes {area:.0} m², more than the {:.0} m² the plant room's range allows", t.plant_room[1]));
    }
    notes.push("the hypochlorite and acid tanks stand 2 m apart in their own bunds; store them in separate ventilated rooms where the authority asks (Sport England; MAHC 4.9.2)".into());
    notes.push(format!("the pumps' {head:.0} m head is an allowance for the filters, strainer and pipework; the pipes are not drawn"));
    Ok(Plant { pool, basis: t.basis, flow, head, pump_power, motor, filter_diameter, balance_tank: balance, items, footprint, area, plant_room: t.plant_room, room: room.map(|(id, _)| id), room_area, notes })
}

/// Reads back what `plant` placed for each pool.
pub fn placed(store: &Store, pool: ElementId) -> Vec<ElementId> {
    let Some(e) = store.get(pool) else { return Vec::new() };
    let guid = e.guid().to_string();
    store.iter().filter(|e| e.params.get(PLANT_KEY).and_then(serde_json::Value::as_str) == Some(guid.as_str())).map(crate::element::Element::id).collect()
}
