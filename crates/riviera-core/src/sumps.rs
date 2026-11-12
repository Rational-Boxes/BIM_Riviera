// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Basement sumps and groundwater drainage (spec Ch 4, MEP-PIPE-011): a perimeter drain round a
//! basement, a sump pit in its floor and duplex pumps lifting the water to the storm drain.
//!
//! - **The drain:** a 100 mm subsoil pipe in a ring `drain_offset` outside the edge of the
//!   basement's floors (default 0.6 m), `drain_depth` below the level (default 0.5 m, beside the
//!   strip footings), in the storm drainage system; a branch dives under the footings and the
//!   slab, 0.9 m down, into the pit.
//! - **The inflow:** `inflow` (L/s), or the drain's length times `seepage` (default 0.01 L/s a
//!   metre: an allowance until the site investigation gives one); the design flow is 1.5 times it.
//! - **The pumps:** a duty and a standby submersible pump, each for the design flow against the
//!   lift from the pit's floor to the discharge plus 3 m for the pipework, at 50 % efficiency, the
//!   motor the next standard size up.
//! - **The pit:** its working volume holds the pumps to `starts` an hour (default 10): V = Q·T/4
//!   with T = 3600/N s; the working depth 0.6 m, the pit `depth` deep (default 1.2 m) and at
//!   least 1.0 m square, with 150 mm concrete walls and base; the floor is opened over it.
//! - **The discharge:** a rising main from the pumps up `rise` (default 3.0 m) and out through
//!   the nearest wall to the drain's line, where it meets the storm drain.
//!
//! Not built yet: the drain's fall to the pit, non-return and isolating valves, the high-level
//! alarm and controls, an underfloor drainage grid, oil interceptors, and the pumps' curves.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::mesh_objects::Category;
use crate::store::Store;
use crate::txn::Transaction;

/// On each part: the pit's GlobalId.
pub const KEY: &str = "rxd:sump";
const MOTORS: [f64; 12] = [0.37, 0.55, 0.75, 1.1, 1.5, 2.2, 3.0, 4.0, 5.5, 7.5, 11.0, 15.0];
const WALL: f64 = 0.15;

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateParams {
    /// The basement's level.
    pub level: ElementRef,
    /// The pit's centre in plan.
    pub point: [f64; 2],
    /// Groundwater inflow, L/s (default the drain's length times `seepage`).
    pub inflow: Option<f64>,
    /// L/s a metre of drain (default 0.01).
    pub seepage: Option<f64>,
    /// Pump starts an hour (default 10).
    pub starts: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub depth: Option<f64>,
    /// The perimeter drain (default true).
    pub drain: Option<bool>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub drain_offset: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub drain_depth: Option<f64>,
    /// The rising main's height above the level before it turns out (default 3.0 m).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub rise: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Sump {
    pub pit: ElementId,
    pub pumps: Vec<ElementId>,
    /// The pit's side and depth, and its working volume (m³).
    pub pit_size: f64,
    pub pit_depth: f64,
    pub working_volume: f64,
    /// The drain's length (m), the inflow and design flow (L/s).
    pub drain_length: f64,
    pub inflow: f64,
    pub design_flow: f64,
    /// Each pump's head (m), hydraulic power and motor (kW).
    pub head: f64,
    pub power: f64,
    pub motor: f64,
    /// The drain, branch and rising main's segments and fittings.
    pub pipes: Vec<ElementId>,
    /// Where the rising main meets the storm drain.
    pub outfall: [f64; 2],
    pub notes: Vec<String>,
}

fn circle(r: f64) -> Vec<[f64; 2]> {
    (0..16).map(|k| {
        let a = std::f64::consts::TAU * k as f64 / 16.0;
        [r * a.cos(), r * a.sin()]
    })
    .collect()
}

/// A generated type made once by name.
fn ty(txn: &mut Transaction<'_>, name: &str, category: Category, shape: &crate::bath_components::Shape, color: [f32; 4], predefined: &str) -> Result<ElementId> {
    if let Some(e) = txn.store().iter().find(|e| e.name.as_deref() == Some(name) && crate::mesh_objects::mesh_object_type(e).is_some()) {
        return Ok(e.id());
    }
    crate::mesh_objects::create_generated_type(txn, name, category, &shape.p, &shape.t, color, Vec::new(), Some(predefined))
}

fn tag(txn: &mut Transaction<'_>, id: ElementId, pit: &str) -> Result<()> {
    txn.modify(id, |e| {
        e.params.insert(KEY.into(), Value::from(pit));
    })
}

fn pipe(txn: &mut Transaction<'_>, level: ElementId, points: Vec<[f64; 3]>, diameter: f64) -> Result<crate::mep_runs::RunReport> {
    let p: crate::mep_runs::CreateParams = serde_json::from_value(json!({ "points": points, "level": level, "shape": "round", "diameter": diameter, "system_type_id": "storm_drainage" })).map_err(|e| bad("sump.params", e.to_string()))?;
    crate::mep_runs::create_pipe(txn, p)
}

/// `sump.create`.
pub(crate) fn create(txn: &mut Transaction<'_>, p: CreateParams) -> Result<Sump> {
    let store = txn.store().clone();
    let level = store.resolve(&p.level).and_then(|id| store.get(id)).filter(|e| e.class == "IfcBuildingStorey").ok_or_else(|| bad("sump.level", "choose the basement's level"))?;
    let (level_id, level_guid) = (level.id(), level.guid().to_string());
    let mut notes = Vec::new();
    // The drain's ring: the level's floors' outline, grown.
    let floors: Vec<Vec<[f64; 2]>> = store
        .iter()
        .filter(|e| crate::floors::floor_recipe(e).is_some_and(|r| r.level == level_guid))
        .filter_map(|e| crate::floors::regenerate(&store, e).ok())
        .flat_map(|g| g.profiles.into_iter().map(|p| p.outer))
        .collect();
    let mut outline = floors.into_iter().max_by(|a, b| crate::polygons::area(a).abs().total_cmp(&crate::polygons::area(b).abs())).ok_or_else(|| bad("sump.floor", "the level has no floor: draw the basement's slab first"))?;
    if crate::polygons::area(&outline) < 0.0 {
        outline.reverse();
    }
    if !crate::sco::inside(p.point, &outline) {
        return Err(bad("sump.point", "put the pit inside the basement's floor"));
    }
    let ring = crate::pools::offset(&outline, p.drain_offset.unwrap_or(0.6));
    let n = ring.len();
    let drain_length: f64 = (0..n).map(|k| (ring[(k + 1) % n][0] - ring[k][0]).hypot(ring[(k + 1) % n][1] - ring[k][1])).sum();
    let inflow = p.inflow.unwrap_or(drain_length * p.seepage.unwrap_or(0.01));
    if p.inflow.is_none() {
        notes.push(format!("the inflow is {:.3} L/s a metre of drain, an allowance: take the groundwater inflow from the site investigation", p.seepage.unwrap_or(0.01)));
    }
    let design = 1.5 * inflow;
    let q = design / 1000.0;
    // The pit: working volume Q·T/4 at `starts` an hour over 0.6 m of working depth.
    let starts = p.starts.unwrap_or(10.0).max(1.0);
    let volume = q * 3600.0 / starts / 4.0;
    let side = ((volume / 0.6).sqrt().max(1.0) * 10.0).ceil() / 10.0;
    let depth = p.depth.unwrap_or(1.2).max(0.9);
    let rise = p.rise.unwrap_or(3.0);
    let head = depth + rise + 3.0;
    let power = 9.81 * q * head / 0.5;
    let motor = MOTORS.iter().copied().find(|m| *m >= power).unwrap_or(MOTORS[MOTORS.len() - 1]);
    if power > MOTORS[MOTORS.len() - 1] {
        notes.push(format!("{power:.1} kW is more than a sump pump: dewater the excavation or divide the drainage"));
    }
    // The pit: a hollow concrete box, the floor opened over it.
    let (h, o) = (side / 2.0, side / 2.0 + WALL);
    let shape = crate::bath_components::Shape::default()
        .cuboid([-o, o], [-o, o], [-depth - WALL, -depth])
        .cuboid([-o, o], [-o, -h], [-depth, 0.0])
        .cuboid([-o, o], [h, o], [-depth, 0.0])
        .cuboid([-o, -h], [-h, h], [-depth, 0.0])
        .cuboid([h, o], [-h, h], [-depth, 0.0]);
    let pit_type = ty(txn, &format!("Sump pit {:.0} × {:.0} mm, {:.0} mm deep", side * 1000.0, side * 1000.0, depth * 1000.0), Category::Chamber, &shape, [0.62, 0.62, 0.60, 1.0], "SUMP")?;
    let (x, y) = (p.point[0], p.point[1]);
    let outer = vec![[x - o, y - o], [x + o, y - o], [x + o, y + o], [x - o, y + o]];
    let shaft: crate::slab_openings::CreateShaftParams = serde_json::from_value(json!({ "loops": [outer], "base_level": level_id, "base_offset": -0.45, "top_constraint": { "unconnected": 0.5 } })).expect("valid");
    let opening = crate::slab_openings::create_shaft(txn, shaft)?;
    let pit = crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(pit_type), point: p.point, level: ElementRef::Id(level_id), offset: 0.0, rotation: 0.0, mirror: false, scale: 1.0 })?;
    let pit_guid = txn.store().get(pit).expect("placed").guid().to_string();
    tag(txn, opening, &pit_guid)?;
    tag(txn, pit, &pit_guid)?;
    // The pumps, duty and standby, on the pit's floor.
    let pump_shape = crate::bath_components::Shape::default().prism(&circle(0.15), 0.0, 0.55);
    let pump_type = ty(txn, &format!("Submersible sump pump {motor} kW"), Category::Pump, &pump_shape, [0.20, 0.35, 0.65, 1.0], "SUBMERSIBLEPUMP")?;
    let mut pumps = Vec::new();
    for dx in [-side / 4.0, side / 4.0] {
        let e = crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(pump_type), point: [x + dx, y], level: ElementRef::Id(level_id), offset: -depth, rotation: 0.0, mirror: false, scale: 1.0 })?;
        tag(txn, e, &pit_guid)?;
        pumps.push(e);
    }
    let mut pipes = Vec::new();
    let mut keep = |txn: &mut Transaction<'_>, r: crate::mep_runs::RunReport| -> Result<()> {
        for id in r.segments.iter().chain(&r.fittings) {
            tag(txn, *id, &pit_guid)?;
            pipes.push(*id);
        }
        Ok(())
    };
    // The nearest point of the drain's line to the pit.
    let nearest = (0..n)
        .map(|k| {
            let (a, b) = (ring[k], ring[(k + 1) % n]);
            let ab = [b[0] - a[0], b[1] - a[1]];
            let t = (((x - a[0]) * ab[0] + (y - a[1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12)).clamp(0.0, 1.0);
            [a[0] + ab[0] * t, a[1] + ab[1] * t]
        })
        .min_by(|a, b| (a[0] - x).hypot(a[1] - y).total_cmp(&(b[0] - x).hypot(b[1] - y)))
        .expect("a ring");
    // The pit's face toward it.
    let toward = [nearest[0] - x, nearest[1] - y];
    let face = if toward[0].abs() > toward[1].abs() { [x + o * toward[0].signum(), y] } else { [x, y + o * toward[1].signum()] };
    let drain_z = -p.drain_depth.unwrap_or(0.5);
    if p.drain.unwrap_or(true) {
        // From the middle of the first side round to it again, so the ring closes on a straight.
        let mid = [(ring[0][0] + ring[1][0]) / 2.0, (ring[0][1] + ring[1][1]) / 2.0, drain_z];
        let mut pts: Vec<[f64; 3]> = vec![mid];
        pts.extend(ring.iter().skip(1).map(|q| [q[0], q[1], drain_z]));
        pts.push([ring[0][0], ring[0][1], drain_z]);
        pts.push(mid);
        let r = pipe(txn, level_id, pts, 0.1)?;
        keep(txn, r)?;
        // The branch: down beside the footing, under it and the slab, into the pit.
        let under = -0.9;
        let start = [nearest[0], nearest[1]];
        let bend = if toward[0].abs() > toward[1].abs() { [start[0], face[1]] } else { [face[0], start[1]] };
        let mut b = vec![[start[0], start[1], drain_z - 0.12], [start[0], start[1], under]];
        if (bend[0] - start[0]).hypot(bend[1] - start[1]) > 0.05 {
            b.push([bend[0], bend[1], under]);
        }
        b.push([face[0], face[1], under]);
        let r = pipe(txn, level_id, b, 0.1)?;
        keep(txn, r)?;
    }
    // The rising main: up from between the pumps and out to the drain's line.
    let main = (q / 1.5 / (std::f64::consts::PI / 4.0)).sqrt().max(0.08);
    let main = [0.08, 0.1, 0.125, 0.15, 0.2].into_iter().find(|d| *d >= main).unwrap_or(0.2);
    let r = pipe(txn, level_id, vec![[x, y, -depth + 0.7], [x, y, rise], [nearest[0], nearest[1], rise]], main)?;
    keep(txn, r)?;
    notes.push("lay the drain to fall 1:200 to the pit; fit a non-return valve and an isolating valve on each pump's discharge, a high-level alarm in the pit, and discharge to the storm drain above its flood level".into());
    Ok(Sump { pit, pumps, pit_size: side, pit_depth: depth, working_volume: volume, drain_length, inflow, design_flow: design, head, power, motor, pipes, outfall: nearest, notes })
}

/// The sumps in a model: each pit and its parts.
pub fn all(store: &Store) -> Vec<(ElementId, Vec<ElementId>)> {
    store
        .iter()
        .filter(|e| e.class == "IfcDistributionChamberElement" && e.param(KEY).is_some())
        .map(|pit| {
            let g = pit.guid().to_string();
            (pit.id(), store.iter().filter(|e| e.id() != pit.id() && e.param(KEY).and_then(Value::as_str) == Some(g.as_str())).map(crate::element::Element::id).collect())
        })
        .collect()
}
