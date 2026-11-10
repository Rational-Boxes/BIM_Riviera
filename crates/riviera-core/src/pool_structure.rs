// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A pool's structural loads (ARC-POOL-010, `POOLS_PLAN.md` §6, research/23 C.4).
//!
//! - **Full**: the water at γw = 9.81 kN/m³ over the floor (γw·h at the deepest point), the shell
//!   at its kind's unit weight (cast concrete and shotcrete 24.5 kN/m³, a one-piece shell
//!   18 kN/m³) and tiling at 1.25 kN/m² (screed and tiles, 1.0–1.5); the walls as cantilevers
//!   of water: base pressure γw·h, shear γw·h²/2 and moment γw·h³/6 a metre.
//! - **Empty**, with the water table `groundwater` above the pool's level: the uplift γw·hw over
//!   the shell's underside, sampled over its footprint, against the shell's and finishes' weight
//!   with a factor (default 1.1: EN 1997's UPL check 0.9·G ≥ 1.0·U gives 1.11).
//! - **Point loads** from the pool equipment by it, advisory values for the anchorages.
//! - **Elevated** pools (a level below): the loads per m² for the slab, the slab's deflection limit
//!   (span/360) and the rooftop notes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::pools::{self, ShellKind};
use crate::store::Store;

/// Water's unit weight (kN/m³).
pub const WATER: f64 = 9.81;
/// Tiling and its screed (kN/m²).
pub const TILING: f64 = 1.25;

/// A shell's unit weight (kN/m³).
pub fn unit_weight(kind: ShellKind) -> f64 {
    match kind {
        ShellKind::CastConcrete | ShellKind::Shotcrete | ShellKind::Panel => 24.5,
        ShellKind::OnePiece => 18.0,
    }
}

/// Advisory point loads (kN) for the anchorage of the Riviera Pools equipment (object type,
/// load), for review: the maker's data governs.
pub const POINT_LOADS: &[(&str, f64)] = &[
    ("Diving Board", 6.0),
    ("Pool Lift", 4.0),
    ("Lifeguard Chair", 2.5),
    ("Starting Platform", 2.0),
    ("Pool Ladder", 1.5),
    ("Grab Rail", 1.0),
];

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StructureParams {
    /// One pool; every pool when omitted.
    pub pool_id: Option<ElementRef>,
    /// The water table's height above the pool's level (m, negative below it); uplift is
    /// checked when given.
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub groundwater: Option<f64>,
    /// The factor the empty pool's weight must carry the uplift by (default 1.1).
    pub factor: Option<f64>,
    /// Whether it stands on a structure (default: when a level is below its own).
    pub elevated: Option<bool>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Wall {
    /// The water's depth at the deepest wall (m).
    pub depth: f64,
    /// At its base: kPa, kN/m and kN·m/m.
    pub pressure: f64,
    pub shear: f64,
    pub moment: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Uplift {
    /// The water table's height (m, model).
    pub groundwater: f64,
    /// The greatest head on the underside (m) and the uplift in all (kN).
    pub head: f64,
    pub force: f64,
    /// The empty pool's weight (kN): its shell and finishes.
    pub resisting: f64,
    /// resisting / force, and the factor asked.
    pub ratio: f64,
    pub factor: f64,
    pub ok: bool,
    /// The groundwater's pressure on the empty pool's walls at their base (kPa).
    pub wall_pressure: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PointLoad {
    pub element: ElementId,
    pub name: String,
    pub object_type: String,
    /// kN, advisory.
    pub load: f64,
    /// Plan point.
    pub at: [f64; 2],
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Structure {
    pub pool: ElementId,
    pub name: String,
    pub shell: ShellKind,
    pub thickness: f64,
    /// m², the water's and the shell's (outside its walls).
    pub area: f64,
    pub footprint: f64,
    /// kN.
    pub water_weight: f64,
    pub shell_weight: f64,
    pub finishes: f64,
    /// kN/m² over the footprint, full and empty.
    pub full: f64,
    pub empty: f64,
    /// kN/m² on the floor at its deepest, full: the water, the base and its tiles (the walls
    /// bear as line loads).
    pub peak: f64,
    /// kN/m along the walls: their weight and their tiles'.
    pub wall_line_load: f64,
    /// kPa, the water on the floor at its deepest.
    pub floor_pressure: f64,
    pub wall: Wall,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uplift: Option<Uplift>,
    pub point_loads: Vec<PointLoad>,
    pub elevated: bool,
    /// The slab's deflection limit under it (mm, the short span over 360), elevated pools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deflection_limit: Option<f64>,
    pub notes: Vec<String>,
}

/// A ring's length.
fn length(r: &[[f64; 2]]) -> f64 {
    (0..r.len()).map(|i| (r[(i + 1) % r.len()][0] - r[i][0]).hypot(r[(i + 1) % r.len()][1] - r[i][1])).sum()
}

/// The plan width across the axis and the length along it.
fn spans(rec: &pools::PoolRecipe) -> (f64, f64) {
    let [a, b] = rec.axis;
    let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
    let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
    let along: Vec<f64> = rec.outline.iter().map(|q| (q[0] - a[0]) * u[0] + (q[1] - a[1]) * u[1]).collect();
    let across: Vec<f64> = rec.outline.iter().map(|q| (q[0] - a[0]) * -u[1] + (q[1] - a[1]) * u[0]).collect();
    let span = |s: &[f64]| s.iter().copied().fold(f64::MIN, f64::max) - s.iter().copied().fold(f64::MAX, f64::min);
    (span(&across), span(&along))
}

/// The water table's push on the empty shell: (greatest head, force in kN), sampled on a 0.2 m
/// grid over the footprint.
fn uplift(rec: &pools::PoolRecipe, water_z: f64, table: f64) -> (f64, f64) {
    let t = rec.shell.thickness;
    let outer = pools::offset(&rec.outline, t);
    let (x, y) = outer.iter().fold(([f64::MAX, f64::MIN], [f64::MAX, f64::MIN]), |(x, y), p| ([x[0].min(p[0]), x[1].max(p[0])], [y[0].min(p[1]), y[1].max(p[1])]));
    let step = 0.2;
    let (mut head, mut force) = (0.0f64, 0.0);
    let mut px = x[0] + step / 2.0;
    while px < x[1] {
        let mut py = y[0] + step / 2.0;
        while py < y[1] {
            if crate::sco::inside([px, py], &outer) {
                let underside = water_z - rec.depth_at(rec.t([px, py])) - t;
                let h = (table - underside).max(0.0);
                head = head.max(h);
                force += WATER * h * step * step;
            }
            py += step;
        }
        px += step;
    }
    (head, force)
}

/// A pool's loads (`pool.structure`).
pub(crate) fn structure(store: &Store, p: Option<StructureParams>) -> Result<Vec<Structure>> {
    let p = p.unwrap_or_default();
    let only = match &p.pool_id {
        Some(r) => Some(store.resolve(r).ok_or_else(|| Error::new("element.not_found", "no such pool"))?),
        None => None,
    };
    let factor = p.factor.unwrap_or(1.1);
    let mut out = Vec::new();
    for (id, g) in pools::all(store) {
        if only.is_some_and(|o| o != id) {
            continue;
        }
        let e = store.get(id).expect("listed");
        let rec = pools::recipe(e).expect("a pool");
        let level_z = crate::levels::elevation_of(store, &rec.level)?;
        let t = rec.shell.thickness;
        let footprint = crate::polygons::area(&pools::offset(&rec.outline, t)).abs();
        let deepest = rec.profile.iter().map(|s| s.depth).fold(0.0, f64::max);
        let wall_depth = rec.outline.iter().map(|q| rec.depth_at(rec.t(*q))).fold(0.0, f64::max);
        let gamma = unit_weight(rec.shell.kind);
        let water_weight = WATER * g.volume;
        let shell_weight = gamma * g.shell_volume;
        let tiled = rec.shell.tile.is_some();
        // Tiles over the floor and the walls' faces below the deck.
        let finishes = if tiled { TILING * (g.area + g.perimeter * (rec.water_level + (deepest + rec.profile.iter().map(|s| s.depth).fold(f64::MAX, f64::min)) / 2.0)) } else { 0.0 };
        let elevated = p.elevated.unwrap_or_else(|| store.iter().filter(|o| o.class == crate::levels::CLASS).any(|o| crate::levels::elevation(o) < level_z - 0.5));
        let mut notes = Vec::new();
        let up = p.groundwater.map(|gw| {
            let table = level_z + gw;
            let (head, force) = uplift(&rec, g.water_z, table);
            let resisting = shell_weight + finishes;
            let ratio = if force > 1e-9 { resisting / force } else { f64::INFINITY };
            let underside = g.water_z - deepest - t;
            Uplift { groundwater: table, head, force, resisting, ratio, factor, ok: ratio >= factor - 1e-9, wall_pressure: WATER * (table - underside).max(0.0) }
        });
        match &up {
            Some(u) if !u.ok => notes.push(format!(
                "Empty, the water table lifts it with {:.0} kN against {:.0} kN of shell ({:.2}, under the {:.2} asked): thicken the base, add a heel the soil bears on, anchor it, or lower the water table while it is empty. A hydrostatic relief valve at the main drain relieves the uplift but is no substitute for designing against flotation.",
                u.force, u.resisting, u.ratio, u.factor
            )),
            Some(u) if u.force > 0.0 => notes.push(format!("Empty, the shell's weight carries the uplift {:.2} times (the {:.2} asked); design the walls for the soil and the groundwater with the pool empty too.", u.ratio, u.factor)),
            Some(_) => {}
            None if !elevated => notes.push("No water table given: uplift is not checked (set `groundwater`, the water table's height above the pool's level).".into()),
            None => {}
        }
        match rec.shell.kind {
            ShellKind::Panel => notes.push("Panel walls weigh less than the concrete counted here: the empty weight is overstated; take the maker's figures.".into()),
            ShellKind::OnePiece => notes.push("A one-piece shell is light: keep it full, or anchor it, wherever the water table can rise above its floor.".into()),
            _ => {}
        }
        let (short, _) = spans(&rec);
        if elevated {
            notes.push(format!(
                "On a structure: {:.1} kN/m² full over the footprint on average ({:.1} on the floor at its deepest, the walls bearing as line loads), before the deck's live load. Keep the slab's deflection under its span/360 ({:.0} mm across {:.1} m); support it evenly, since uneven support cracks the shell.",
                (water_weight + shell_weight + finishes) / footprint,
                WATER * deepest + gamma * t + if tiled { TILING } else { 0.0 },
                short / 360.0 * 1000.0,
                short
            ));
            notes.push("Under the shell: a waterproofing membrane and a drainage layer falling 2 % (1/4 in a foot) to a drain; about 100 mm of structural geofoam between the shell and the slab; no plumbing cast into the shell; flood-test it (ASTM D5957) or test it electronically (ASTM D7877).".into());
        }
        notes.push("The deck takes an assembly live load (ASCE 7 100 psf, 4.79 kN/m², or EN 1991-1-1 C3 5.0 kN/m²); check the code that applies.".into());
        let point_loads = store
            .iter()
            .filter(|o| o.id() != id)
            .filter_map(|o| {
                let (object_type, load) = POINT_LOADS.iter().find(|(n, _)| crate::rules::Selector { object_types: vec![(*n).into()], ..Default::default() }.selects_element(store, o))?;
                let object_type = (*object_type).to_string();
                let load = *load;
                let it = crate::rules::item(store, o)?;
                let c = it.frame.apply([(it.x[0] + it.x[1]) / 2.0, (it.front + it.back) / 2.0, 0.0]);
                let at = [c[0], c[1]];
                (crate::sco::inside(at, &rec.outline) || pools::edge_distance(at, &rec.outline) <= 2.0).then(|| PointLoad { element: o.id(), name: o.name.clone().unwrap_or_default(), object_type, load, at })
            })
            .collect();
        out.push(Structure {
            pool: id,
            name: e.name.clone().unwrap_or_default(),
            shell: rec.shell.kind,
            thickness: t,
            area: g.area,
            footprint,
            water_weight,
            shell_weight,
            finishes,
            full: (water_weight + shell_weight + finishes) / footprint,
            empty: (shell_weight + finishes) / footprint,
            peak: WATER * deepest + gamma * t + if tiled { TILING } else { 0.0 },
            wall_line_load: ((shell_weight - gamma * g.area * t).max(0.0) + if tiled { finishes - TILING * g.area } else { 0.0 }) / length(&pools::offset(&rec.outline, t / 2.0)),
            floor_pressure: WATER * deepest,
            wall: Wall { depth: wall_depth, pressure: WATER * wall_depth, shear: WATER * wall_depth * wall_depth / 2.0, moment: WATER * wall_depth.powi(3) / 6.0 },
            uplift: up,
            point_loads,
            elevated,
            deflection_limit: elevated.then_some(short / 360.0 * 1000.0),
            notes,
        });
    }
    Ok(out)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyParams {
    pub pool_id: ElementRef,
    /// The analytical panel (the slab under it).
    pub panel_id: ElementRef,
    /// The case for the shell and finishes.
    pub dead_case_id: ElementRef,
    /// The case for the water (a fluid load).
    pub water_case_id: ElementRef,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Applied {
    pub dead: ElementId,
    pub water: ElementId,
    /// N/m², the totals spread over the panel.
    pub dead_pressure: f64,
    pub water_pressure: f64,
    /// m².
    pub panel_area: f64,
    pub footprint: f64,
}

/// Puts a pool's weight on the slab's analytical panel (`pool.apply_loads`): two uniform area
/// loads, the shell and finishes in one case and the water in another, each its total spread
/// over the panel.
pub(crate) fn apply(txn: &mut crate::txn::Transaction<'_>, p: ApplyParams) -> Result<Applied> {
    let store = txn.store().clone();
    let s = structure(&store, Some(StructureParams { pool_id: Some(p.pool_id.clone()), elevated: Some(true), ..Default::default() }))?;
    let s = s.into_iter().next().ok_or_else(|| Error::new("pool.not_a_pool", "not a pool"))?;
    let panel = store
        .resolve(&p.panel_id)
        .and_then(|i| store.get(i))
        .and_then(|e| crate::analytical_panels::geometry(&store, e))
        .ok_or_else(|| Error::new("load.no_host", "the pool's loads go on an analytical panel"))?;
    let panel_area = panel.area();
    if panel_area < 1e-6 {
        return Err(Error::new("pool.panel_area", "the panel has no area"));
    }
    if panel_area < s.footprint * 0.5 {
        return Err(Error::new("pool.panel_small", format!("the panel ({panel_area:.1} m²) is much smaller than the pool's footprint ({:.1} m²)", s.footprint)));
    }
    let dead_pressure = (s.shell_weight + s.finishes) * 1000.0 / panel_area;
    let water_pressure = s.water_weight * 1000.0 / panel_area;
    let area = |txn: &mut crate::txn::Transaction<'_>, case: &ElementRef, v: f64| crate::loads::create_area(txn, crate::loads::AreaParams { host_id: p.panel_id.clone(), case_id: case.clone(), value: [0.0, 0.0, -v], orientation: Default::default(), projected: false });
    let dead = area(txn, &p.dead_case_id, dead_pressure)?;
    let water = area(txn, &p.water_case_id, water_pressure)?;
    Ok(Applied { dead, water, dead_pressure, water_pressure, panel_area, footprint: s.footprint })
}
