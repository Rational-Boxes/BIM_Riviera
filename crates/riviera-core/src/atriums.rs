// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Atriums (spec Ch 3, ARC-GLZ-018; `GLAZED_ENCLOSURES_PLAN.md` §6, phase 4): a void through a
//! building's floors with guards round it on every level it opens, under a glazed roof that may
//! reach past the void (over a roof terrace or pool), and the atrium's code summary and smoke
//! exhaust.
//!
//! - **Made of ordinary elements:** the void is a shaft opening (`opening.create_shaft`) from just
//!   above the atrium floor through the top level's floor; the guards are railings 0.1 m back
//!   from each opened floor's edge, of an *Atrium guard* type 42 in high; the roof is a footprint
//!   roof (flat, ridge or pyramid) or an arched roof (barrel) of an *Atrium glazing* sloped glazing
//!   type. The atrium is an `IfcSpace` (ObjectType `Atrium`) holding their GlobalIds.
//! - **The check** (`atrium.check`) counts the storeys open to the atrium and reports IBC §404
//!   (2017 public text, research/10 §2.1) and the NFPA 92B (1991) axisymmetric plume (research/10
//!   §2.4): the smoke layer held 6 ft above the highest walkway open to it, the plume's mass flow
//!   there for a steady design fire, its temperature and the exhaust volume; and the time to fill
//!   to that height with no exhaust.
//!
//! Not built yet: linear, attached and edge atriums, space-frame and ETFE roofs, smoke vents and
//! their EN 12101-2 areas, balcony spill plumes, stack ventilation and stratification, the
//! atrium's daylight and energy. It is made again as one with `layout.redo` (Edit Layout), or its
//! parts are edited as themselves.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::element::{Element, NewElement};
use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::store::Store;
use crate::txn::Transaction;
use crate::walls::RECIPE_KEY;

pub const RECIPE: &str = "recipe.atrium";
const FT: f64 = 0.3048;
/// The guard's height (IBC 1015.3: 42 in).
pub const GUARD: f64 = 42.0 * 0.0254;
/// How far the guards stand back from the floor's edge.
const GUARD_BACK: f64 = 0.1;

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoofForm {
    Flat,
    /// Two slopes rising from the longest edge and the one opposite it (default).
    #[default]
    Ridge,
    Pyramid,
    /// A glazed vault springing from the longest edge and the one opposite it.
    Barrel,
}

/// An atrium (`recipe.atrium`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AtriumRecipe {
    pub kind: String,
    /// The void, counter-clockwise.
    pub outline: Vec<[f64; 2]>,
    /// The atrium floor's level and the top level it opens (GlobalIds).
    pub base_level: String,
    pub top_level: String,
    pub form: RoofForm,
    /// Its parts (GlobalIds).
    pub shaft: String,
    pub roof: String,
    #[serde(default)]
    pub guards: Vec<String>,
    /// The roof's footprint and its plate above the top level.
    #[serde(default)]
    pub roof_outline: Vec<[f64; 2]>,
    #[serde(default)]
    pub roof_offset: f64,
    /// The glazed walls closing the roof's sides (GlobalIds).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub walls: Vec<String>,
}

pub fn recipe(e: &Element) -> Option<AtriumRecipe> {
    let v = e.param(RECIPE_KEY)?;
    (v.get("kind")?.as_str()? == RECIPE).then(|| serde::Deserialize::deserialize(v).ok())?
}

fn resolve(store: &Store, r: &ElementRef) -> Result<ElementId> {
    store.resolve(r).ok_or_else(|| bad("element.not_found", format!("no element {r:?}")))
}

fn guid(store: &Store, id: ElementId) -> String {
    store.get(id).expect("exists").guid().to_string()
}

/// A type by name, made with `make` the first time.
fn named_type(txn: &mut Transaction<'_>, name: &str, make: impl FnOnce(&mut Transaction<'_>) -> Result<ElementId>) -> Result<ElementId> {
    if let Some(e) = txn.store().iter().find(|e| crate::categories::is_type(e) && e.name.as_deref() == Some(name)) {
        return Ok(e.id());
    }
    make(txn)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateParams {
    /// The void in plan (at least three corners).
    pub outline: Vec<[f64; 2]>,
    /// The atrium floor's level: its floor is kept.
    pub base_level: ElementRef,
    /// The highest level whose floor is opened; the roof stands on it.
    pub top_level: ElementRef,
    pub form: Option<RoofForm>,
    /// Slopes for ridge and pyramid roofs (default 15°); a barrel's rise (default a quarter of its span).
    #[schemars(extend("x-riviera-data-type" = "angle"))]
    pub slope: Option<Value>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub rise: Option<f64>,
    /// The roof's footprint, when it reaches past the void (over a terrace or a pool); default the
    /// void grown by `overhang` (default 0.3 m) to bear on the floor round it.
    pub roof_outline: Option<Vec<[f64; 2]>>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub overhang: Option<f64>,
    /// The roof's plate above the top level (default 0).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub roof_offset: Option<f64>,
    /// A sloped glazing roof type (default *Atrium glazing*, made the first time).
    pub glazing_type: Option<ElementRef>,
    /// Glazed curtain walls on the top level closing the roof's sides, 0.3 m inside its edges,
    /// their tops attached to the roof (default false).
    pub walls: Option<bool>,
    /// The guards' height (default 42 in); 0 for none.
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub guard_height: Option<f64>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Created {
    pub atrium: ElementId,
    pub shaft: ElementId,
    pub roof: ElementId,
    pub guards: Vec<ElementId>,
    /// The glazed walls under the roof's edges.
    pub walls: Vec<ElementId>,
    /// The levels whose floors it opens.
    pub opened: Vec<String>,
}

fn ccw(mut r: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    if crate::polygons::area(&r) < 0.0 {
        r.reverse();
    }
    r
}

/// The edges' curves for a footprint, those of `slopes` (indices) sloping.
fn curves(r: &[[f64; 2]], slopes: &dyn Fn(usize) -> bool) -> Value {
    let n = r.len();
    json!((0..n).map(|k| json!({ "line": { "start": r[k], "end": r[(k + 1) % n] }, "defines_slope": slopes(k) })).collect::<Vec<_>>())
}

/// The longest edge and the edge most nearly opposite it.
fn long_pair(r: &[[f64; 2]]) -> (usize, usize) {
    let n = r.len();
    let dir = |k: usize| {
        let (a, b) = (r[k], r[(k + 1) % n]);
        let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
        ([(b[0] - a[0]) / l, (b[1] - a[1]) / l], l)
    };
    let first = (0..n).max_by(|a, b| dir(*a).1.total_cmp(&dir(*b).1)).unwrap_or(0);
    let d = dir(first).0;
    let second = (0..n).filter(|k| *k != first).min_by(|a, b| {
        let (x, y) = (dir(*a).0, dir(*b).0);
        (x[0] * d[0] + x[1] * d[1]).total_cmp(&(y[0] * d[0] + y[1] * d[1]))
    });
    (first, second.unwrap_or(first))
}

/// `atrium.create`.
pub(crate) fn create(txn: &mut Transaction<'_>, p: CreateParams) -> Result<Created> {
    let store = txn.store().clone();
    if p.outline.len() < 3 || crate::polygons::area(&p.outline).abs() < 1.0 {
        return Err(bad("atrium.outline", "an atrium's void needs at least three corners and 1 m²"));
    }
    let outline = ccw(p.outline.clone());
    let (base, top) = (resolve(&store, &p.base_level)?, resolve(&store, &p.top_level)?);
    let levels = crate::levels::list(&store);
    let z = |id: ElementId| levels.iter().find(|l| l.id == id).map(|l| l.elevation).ok_or_else(|| bad("atrium.level", format!("element {id} is not a level")));
    let (z0, z1) = (z(base)?, z(top)?);
    if z1 <= z0 + 1e-6 {
        return Err(bad("atrium.levels", "the top level must be above the atrium floor's"));
    }
    let opened: Vec<&crate::levels::LevelInfo> = levels.iter().filter(|l| l.elevation > z0 + 1e-6 && l.elevation <= z1 + 1e-6).collect();
    // The void: from 50 mm over the atrium floor through the top level's floor.
    let shaft: crate::slab_openings::CreateShaftParams = serde_json::from_value(json!({ "loops": [outline], "base_level": base, "base_offset": 0.05, "top_constraint": { "unconnected": z1 - z0 } })).expect("valid");
    let shaft = crate::slab_openings::create_shaft(txn, shaft)?;
    // Guards on every opened floor.
    let guard_height = p.guard_height.unwrap_or(GUARD);
    let mut guards = Vec::new();
    if guard_height > 0.0 {
        let name = format!("Atrium guard {:.0} mm", guard_height * 1000.0);
        let ty = named_type(txn, &name, |txn| {
            let t: crate::railings::CreateTypeParams = serde_json::from_value(json!({ "name": name, "height": guard_height, "baluster_spacing": 0.11 })).expect("valid");
            crate::railings::create_type(txn, t)
        })?;
        let path = crate::pools::offset(&outline, GUARD_BACK);
        for l in &opened {
            let r: crate::railings::CreateParams = serde_json::from_value(json!({ "type_id": ty, "path": path, "closed": true, "base_level": l.id })).expect("valid");
            guards.push(crate::railings::create(txn, r)?);
        }
    }
    // The glazed roof.
    let glazing = match &p.glazing_type {
        Some(r) => resolve(txn.store(), r)?,
        None => named_type(txn, "Atrium glazing", |txn| {
            let t: crate::roofs::glazing::CreateTypeParams = serde_json::from_value(json!({ "name": "Atrium glazing" })).expect("valid");
            crate::roofs::glazing::create_type(txn, t)
        })?,
    };
    let roof_outline = ccw(p.roof_outline.clone().unwrap_or_else(|| crate::pools::offset(&outline, p.overhang.unwrap_or(0.3))));
    let form = p.form.unwrap_or_default();
    let (a, b) = long_pair(&roof_outline);
    let slope = p.slope.clone().unwrap_or(json!("15°"));
    let offset = p.roof_offset.unwrap_or(0.0);
    let roof = match form {
        RoofForm::Barrel => {
            let span = {
                let n = roof_outline.len();
                let (p0, p1) = (roof_outline[a], roof_outline[(a + 1) % n]);
                let l = (p1[0] - p0[0]).hypot(p1[1] - p0[1]).max(1e-12);
                roof_outline.iter().map(|q| (((q[0] - p0[0]) * (p1[1] - p0[1]) - (q[1] - p0[1]) * (p1[0] - p0[0])) / l).abs()).fold(0.0, f64::max)
            };
            let r: crate::roofs::curved::CreateArchedParams = serde_json::from_value(json!({
                "type_id": glazing, "curves": curves(&roof_outline, &|_| false), "level_id": top, "base_offset": offset,
                "spring_edges": [a as u32 + 1, b as u32 + 1], "rise": p.rise.unwrap_or(span / 4.0),
            }))
            .map_err(|e| bad("atrium.roof", e.to_string()))?;
            crate::roofs::curved::create_arched(txn, r)?
        }
        _ => {
            let r: crate::roofs::CreateParams = serde_json::from_value(json!({
                "type_id": glazing,
                "curves": curves(&roof_outline, &|k| match form { RoofForm::Flat => false, RoofForm::Pyramid => true, _ => k == a || k == b }),
                "level_id": top, "base_offset": offset, "slope": slope,
            }))
            .map_err(|e| bad("atrium.roof", e.to_string()))?;
            crate::roofs::create(txn, r)?
        }
    };
    // The roof's sides closed in glass: curtain walls 0.3 m inside its edges, attached to it.
    let mut walls = Vec::new();
    if p.walls.unwrap_or(false) {
        let inner = crate::pools::offset(&roof_outline, -0.3);
        let n = inner.len();
        for k in 0..n {
            // Every other wall stops short of the corners by a mullion, so the corners meet once.
            let (mut a, mut b) = (inner[k], inner[(k + 1) % n]);
            if k % 2 == 1 {
                let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
                let d = [(b[0] - a[0]) / l * 0.1, (b[1] - a[1]) / l * 0.1];
                a = [a[0] + d[0], a[1] + d[1]];
                b = [b[0] - d[0], b[1] - d[1]];
            }
            let c: crate::curtain::CreateParams = serde_json::from_value(json!({ "curve": { "start": a, "end": b }, "base_level": top, "top_constraint": { "unconnected": offset + 1.0 } })).expect("valid");
            walls.push(crate::curtain::create(txn, c)?);
        }
        let a: crate::walls::attach::AttachParams = serde_json::from_value(json!({ "wall_ids": walls, "target_id": roof })).expect("valid");
        crate::walls::attach::attach(txn, a)?;
    }
    let s = txn.store().clone();
    let rec = AtriumRecipe {
        kind: RECIPE.into(),
        outline,
        base_level: guid(&s, base),
        top_level: guid(&s, top),
        form,
        shaft: guid(&s, shaft),
        roof: guid(&s, roof),
        guards: guards.iter().map(|g| guid(&s, *g)).collect(),
        roof_outline: roof_outline.clone(),
        roof_offset: offset,
        walls: walls.iter().map(|w| guid(&s, *w)).collect(),
    };
    let atrium = txn.create(
        NewElement::new("IfcSpace")
            .name(p.name.as_deref().unwrap_or("Atrium"))
            .param("@PredefinedType", "USERDEFINED")
            .param("@ObjectType", "Atrium")
            .param(RECIPE_KEY, serde_json::to_value(&rec).expect("serialises")),
    )?;
    Ok(Created { atrium, shaft, roof, guards, walls, opened: opened.iter().map(|l| l.name.clone()).collect() })
}

// ---- The check ----

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckParams {
    pub atrium_id: Option<ElementRef>,
    /// The steady design fire, kW (default 5,000: an atrium with combustibles).
    pub fire: Option<f64>,
    /// The smoke layer's least height above the highest walkway open to the atrium (default 6 ft).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub clearance: Option<f64>,
    /// The air's temperature, °C (default 20).
    pub ambient: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Finding {
    pub rule: String,
    /// `error` or `note`.
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Smoke {
    /// The design fire and its convective part (0.7), kW.
    pub fire: f64,
    pub convective: f64,
    /// The smoke layer's interface above the atrium floor, and the flame's height, m.
    pub clear_height: f64,
    pub flame_height: f64,
    /// The plume's mass flow at the interface, kg/s; its mean temperature, °C; and the exhaust
    /// that holds the layer there, m³/s at that temperature.
    pub mass_flow: f64,
    pub plume_temperature: f64,
    pub exhaust: f64,
    /// Seconds to fill down to the interface with no exhaust (steady filling), where the
    /// correlation holds (A/H² from 0.9 to 14).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_time: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Report {
    pub atrium: ElementId,
    pub name: String,
    /// The storeys open to it, its floor's included.
    pub storeys: usize,
    pub levels: Vec<String>,
    /// Floor to the roof's plate, m; the void's area, m², and volume, m³.
    pub height: f64,
    pub area: f64,
    pub volume: f64,
    pub roof_area: f64,
    pub smoke: Smoke,
    pub findings: Vec<Finding>,
}

/// `atrium.check`: every atrium, or one.
pub(crate) fn check(store: &Store, p: Option<CheckParams>) -> Result<Vec<Report>> {
    let p = p.unwrap_or_default();
    let only = p.atrium_id.as_ref().map(|r| resolve(store, r)).transpose()?;
    let levels = crate::levels::list(store);
    let mut out = Vec::new();
    for e in store.iter().filter(|e| only.is_none_or(|o| o == e.id())) {
        let Some(rec) = recipe(e) else {
            if only.is_some() {
                return Err(bad("atrium.not_atrium", format!("element {} is not an atrium", e.id())));
            }
            continue;
        };
        let lz = |g: &str| levels.iter().find(|l| l.guid == g).map(|l| l.elevation);
        let (Some(z0), Some(z1)) = (lz(&rec.base_level), lz(&rec.top_level)) else { continue };
        let open: Vec<&crate::levels::LevelInfo> = levels.iter().filter(|l| l.elevation >= z0 - 1e-6 && l.elevation <= z1 + 1e-6).collect();
        // The roof's plate above the atrium floor.
        let roof_z = z1 + rec.roof_offset;
        let roof_area = (!rec.roof_outline.is_empty()).then(|| crate::polygons::area(&rec.roof_outline).abs());
        let height = roof_z - z0;
        let area = crate::polygons::area(&rec.outline).abs();
        let mut findings = Vec::new();
        let mut find = |rule: &str, error: bool, message: String| findings.push(Finding { rule: rule.into(), severity: if error { "error" } else { "note" }.into(), message });
        let n = open.len();
        // IBC §404 (research/10 §2.1).
        if n > 2 {
            find("IBC 404.5", false, format!("{n} storeys open to the atrium: smoke control is required (more than two)"));
        }
        find("IBC 404.3", false, "the building is sprinklered throughout".into());
        if height > 55.0 * FT {
            find("IBC 404.3", false, format!("its ceiling is {:.1} m ({:.0} ft) up: no sprinklers are needed at the ceiling (over 55 ft)", height, height / FT));
        }
        if n > 3 {
            find("IBC 404.6", false, format!("{n} storeys open: all but three must be separated from the atrium by a 1 h fire barrier or glass wetted by sprinklers 4–12 in from it at 6 ft"));
        }
        find("IBC 404.8", false, "wall and ceiling finishes in the atrium at least Class B".into());
        find("IBC 404.9", false, "exit-access travel within the atrium at most 200 ft (61 m) on levels other than the exit discharge".into());
        // Smoke (NFPA 92B 1991, research/10 §2.4): the layer held `clearance` above the highest
        // walkway open to it.
        let fire = p.fire.unwrap_or(5000.0);
        let qc = 0.7 * fire;
        let walkway = open.iter().map(|l| l.elevation).fold(z0, f64::max) - z0;
        let clearance = p.clearance.unwrap_or(6.0 * FT);
        let clear = (walkway + clearance).max(1.0);
        let zl = 0.166 * qc.powf(0.4);
        let m = if clear > zl { 0.071 * qc.powf(1.0 / 3.0) * clear.powf(5.0 / 3.0) + 0.0018 * qc } else { 0.032 * qc.powf(0.6) * clear };
        let t0 = p.ambient.unwrap_or(20.0);
        let tp = t0 + qc / (m * 1.0);
        let rho = 353.0 / (tp + 273.15);
        let exhaust = m / rho;
        let ah2 = area / (height * height).max(1e-9);
        let zh = clear / height.max(1e-9);
        let fill_time = ((0.9..=14.0).contains(&ah2) && zh >= 0.2 && zh < 1.0).then(|| ah2 * height.powf(4.0 / 3.0) / fire.powf(1.0 / 3.0) * ((1.11 - zh) / 0.28).exp());
        if clear >= height - 1e-6 {
            find("IBC 909 / NFPA 92", true, format!("the smoke layer cannot be held {:.2} m above the highest walkway ({:.1} m up) under a roof {:.1} m up: raise the roof or keep the top floor out of the smoke zone", clearance, walkway, height));
        } else if height - clear < 0.2 * height - 1e-6 {
            find("NFPA 92B", false, format!("the smoke layer is {:.1} m deep, under 20 % of the {:.1} m height: the 6 ft clearance and the layer depth conflict; use the alternative-methods route", height - clear, height));
        }
        find("NFPA 92", false, format!("a {:.0} MW fire's plume carries {m:.0} kg/s at {clear:.1} m; exhaust about {exhaust:.0} m³/s at {tp:.0} °C to hold the layer there (a steady axisymmetric plume, NFPA 92B 1991: check the current edition)", fire / 1000.0));
        out.push(Report {
            atrium: e.id(),
            name: e.name.clone().unwrap_or_else(|| "Atrium".into()),
            storeys: n,
            levels: open.iter().map(|l| l.name.clone()).collect(),
            height,
            area,
            volume: area * height,
            roof_area: roof_area.unwrap_or(area),
            smoke: Smoke { fire, convective: qc, clear_height: clear, flame_height: zl, mass_flow: m, plume_temperature: tp, exhaust, fill_time },
            findings,
        });
    }
    Ok(out)
}
