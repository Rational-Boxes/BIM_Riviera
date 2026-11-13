// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Rooftop photovoltaic arrays (spec Ch 4, MEP-ELEC-022): rows of modules filling what is left of
//! a roof once its glazing, equipment and other things standing on it are kept clear of.
//!
//! - **The region:** `outline`, or the floors on the level (their outer outlines), less a
//!   `setback` from its edge (default 4 ft, a fire-service pathway).
//! - **What stands on it:** every element whose body rises above the roof (from 50 mm to 8 m over
//!   the level) is kept `clearance` clear (default 1.2 m, for maintenance): roofs and glazing,
//!   equipment, lift overruns and the like, by their plan extents. Railings, floors, coverings and
//!   spaces are not.
//! - **The modules** (`module`, default 1.722 × 1.134 m and 400 W) are tilted `tilt` (default 10°)
//!   up from the side facing `azimuth` (degrees from true north, clockwise; default facing the
//!   equator), in rows along it, `gap` apart (default 20 mm), on racks 0.3 m up at their low
//!   edge. Rows are `row_pitch` apart, by default far enough that a row does not shade the next at
//!   noon on the winter solstice: L cos tilt + L sin tilt / tan(altitude), the altitude
//!   90° − |latitude| − 23.44°, at least 15°.
//! - **The array** is an `IfcSolarDevice` (SOLARPANEL) with a body of all its modules; it reports
//!   the modules, rows, area and peak power. The yield is for the solar analysis (ANA-SOLAR-001).
//!
//! Not built yet: strings, inverters and their circuits, east–west and portrait layouts,
//! modules on sloped roofs and façades, ballast and wind uplift, the IFC's 3 ft ridge setbacks for
//! homes, and the array in the energy model.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::bodies::Mesh;
use crate::element::{Element, NewElement};
use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::store::Store;
use crate::txn::Transaction;
use crate::walls::RECIPE_KEY;

pub const RECIPE: &str = "recipe.pv_array";

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Module {
    /// Along the row, and up the slope.
    pub width: f64,
    pub length: f64,
    /// Watts at standard test conditions.
    pub watts: f64,
}

impl Default for Module {
    fn default() -> Self {
        Module { width: 1.134, length: 1.722, watts: 400.0 }
    }
}

/// An array as placed (`recipe.pv_array`): its modules' low-front corners are worked out again
/// from these.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PvRecipe {
    pub kind: String,
    pub level: String,
    pub module: Module,
    /// Radians.
    pub tilt: f64,
    /// The direction the modules face in plan (unit, model axes).
    pub facing: [f64; 2],
    /// Each module's plan rectangle's centre.
    pub modules: Vec<[f64; 2]>,
}

pub fn recipe(e: &Element) -> Option<PvRecipe> {
    let v = e.param(RECIPE_KEY)?;
    (v.get("kind")?.as_str()? == RECIPE).then(|| serde::Deserialize::deserialize(v).ok())?
}

impl PvRecipe {
    /// The modules' solids: tilted plates 35 mm thick, their low edge 0.3 m over the roof.
    pub fn meshes(&self, z: f64) -> Vec<Mesh> {
        let f = self.facing;
        let along = [-f[1], f[0]];
        let (w, l) = (self.module.width, self.module.length);
        let (run, rise) = (l * self.tilt.cos(), l * self.tilt.sin());
        let normal = [-f[0] * self.tilt.sin(), -f[1] * self.tilt.sin(), self.tilt.cos()];
        self.modules
            .iter()
            .map(|c| {
                // Low edge toward the facing side, high edge away from it.
                let at = |a: f64, b: f64, h: f64| [c[0] + along[0] * a + f[0] * b, c[1] + along[1] * a + f[1] * b, z + 0.3 + h];
                let quad = vec![at(-w / 2.0, run / 2.0, 0.0), at(w / 2.0, run / 2.0, 0.0), at(w / 2.0, -run / 2.0, rise), at(-w / 2.0, -run / 2.0, rise)];
                crate::steel_fab::prism(&quad, normal, 0.035)
            })
            .collect()
    }
}

/// Every array's modules (one mesh for each array), for drawing and IFC.
pub fn all(store: &Store) -> Vec<(ElementId, Vec<Mesh>)> {
    store
        .iter()
        .filter_map(|e| {
            let r = recipe(e)?;
            let z = crate::levels::elevation_of(store, &r.level).ok()?;
            Some((e.id(), vec![crate::steel_fab::join(r.meshes(z))]))
        })
        .collect()
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArrayParams {
    pub level: ElementRef,
    /// The region (default the level's floors).
    pub outline: Option<Vec<[f64; 2]>>,
    pub module: Option<Module>,
    /// Degrees (default 10).
    pub tilt: Option<f64>,
    /// Degrees clockwise from true north the modules face (default toward the equator).
    pub azimuth: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub setback: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub clearance: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub gap: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub row_pitch: Option<f64>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Obstacle {
    pub element: ElementId,
    pub class: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Array {
    pub array: ElementId,
    pub modules: usize,
    pub rows: usize,
    /// Kilowatts peak, and the modules' area (m²).
    pub kwp: f64,
    pub area: f64,
    pub row_pitch: f64,
    /// What it was kept clear of.
    pub kept_clear_of: Vec<Obstacle>,
    pub notes: Vec<String>,
}

const SKIP: &[&str] = &["IfcRailing", "IfcSlab", "IfcCovering", "IfcSpace", "IfcBuildingStorey", "IfcOpeningElement", "IfcGroup", "IfcSolarDevice", "IfcBeam", "IfcColumn"];

/// `pv.array`.
pub(crate) fn array(txn: &mut Transaction<'_>, p: ArrayParams) -> Result<Array> {
    let store = txn.store().clone();
    let level = store.resolve(&p.level).and_then(|id| store.get(id)).filter(|e| e.class == "IfcBuildingStorey").ok_or_else(|| bad("pv.level", "choose the roof's level"))?;
    let level_guid = level.guid().to_string();
    let z = crate::levels::elevation_of(&store, &level_guid)?;
    let mut notes = Vec::new();
    // The regions: the outline, or the level's floors.
    let regions: Vec<Vec<[f64; 2]>> = match &p.outline {
        Some(o) => vec![o.clone()],
        None => store
            .iter()
            .filter(|e| crate::floors::floor_recipe(e).is_some_and(|r| r.level == level_guid))
            .filter_map(|e| crate::floors::regenerate(&store, e).ok())
            .flat_map(|g| g.profiles.into_iter().map(|p| p.outer))
            .collect(),
    };
    if regions.is_empty() {
        return Err(bad("pv.region", "the level has no floor to put the array on; give an outline"));
    }
    let setback = p.setback.unwrap_or(4.0 * 0.3048);
    let clearance = p.clearance.unwrap_or(1.2);
    let module = p.module.unwrap_or_default();
    if !(module.width > 0.1 && module.length > 0.1 && module.watts > 0.0) {
        return Err(bad("pv.module", "a module needs a width, a length and its watts"));
    }
    let tilt = p.tilt.unwrap_or(10.0).clamp(0.0, 60.0).to_radians();
    let gap = p.gap.unwrap_or(0.02);
    // Facing: toward the equator unless given, turned from true north into the model's axes.
    let loc = crate::location::get(&store);
    let lat = loc.latitude.unwrap_or_else(|| {
        notes.push("the project has no location: the array faces south and its rows are spaced for 40° N".into());
        40.0
    });
    let azimuth = p.azimuth.unwrap_or(if lat >= 0.0 { 180.0 } else { 0.0 }).to_radians();
    let t = loc.true_north.to_radians();
    let (e, n) = (azimuth.sin(), azimuth.cos());
    let facing = [e * t.cos() - n * t.sin(), e * t.sin() + n * t.cos()];
    let along = [-facing[1], facing[0]];
    let altitude = (90.0 - lat.abs() - 23.44).max(15.0).to_radians();
    let (run, rise) = (module.length * tilt.cos(), module.length * tilt.sin());
    let pitch = p.row_pitch.unwrap_or(run + rise / altitude.tan()).max(run);
    // What stands on the roof: plan boxes grown by the clearance.
    let mut boxes: std::collections::BTreeMap<ElementId, ([f64; 3], [f64; 3])> = std::collections::BTreeMap::new();
    for b in crate::bodies::all(&store) {
        let Some((lo, hi)) = b.bounds() else { continue };
        if hi[2] <= z + 0.05 || lo[2] >= z + 8.0 {
            continue;
        }
        let Some(el) = store.get(b.element) else { continue };
        if SKIP.contains(&el.class.as_str()) || crate::floors::floor_recipe(el).is_some() {
            continue;
        }
        let e = boxes.entry(b.element).or_insert((lo, hi));
        for k in 0..3 {
            e.0[k] = e.0[k].min(lo[k]);
            e.1[k] = e.1[k].max(hi[k]);
        }
    }
    let obstacles: Vec<Vec<[f64; 2]>> = boxes.values().map(|(lo, hi)| vec![[lo[0] - clearance, lo[1] - clearance], [hi[0] + clearance, lo[1] - clearance], [hi[0] + clearance, hi[1] + clearance], [lo[0] - clearance, hi[1] + clearance]]).collect();
    let kept_clear_of: Vec<Obstacle> = boxes.keys().map(|id| Obstacle { element: *id, class: store.get(*id).map(|e| e.class.clone()).unwrap_or_default() }).collect();
    // Rows across each region, in the frame of `along` and `facing`.
    let mut centres: Vec<[f64; 2]> = Vec::new();
    let mut rows = 0;
    for region in &regions {
        let mut r = region.clone();
        if crate::polygons::area(&r) < 0.0 {
            r.reverse();
        }
        let inner = crate::pools::offset(&r, -setback);
        let loc_of = |q: [f64; 2]| [q[0] * along[0] + q[1] * along[1], q[0] * facing[0] + q[1] * facing[1]];
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for q in &inner {
            let l = loc_of(*q);
            lo = [lo[0].min(l[0]), lo[1].min(l[1])];
            hi = [hi[0].max(l[0]), hi[1].max(l[1])];
        }
        let world = |a: f64, b: f64| [along[0] * a + facing[0] * b, along[1] * a + facing[1] * b];
        let mut v = lo[1] + run / 2.0;
        while v + run / 2.0 <= hi[1] + 1e-9 {
            let mut placed_in_row = false;
            let mut u = lo[0] + module.width / 2.0;
            while u + module.width / 2.0 <= hi[0] + 1e-9 {
                let corners: Vec<[f64; 2]> = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]].iter().map(|s| world(u + s[0] * module.width / 2.0, v + s[1] * run / 2.0)).collect();
                let inside = corners.iter().all(|q| crate::sco::inside(*q, &inner));
                let clear = inside
                    && obstacles.iter().all(|o| {
                        let shape = crate::polygons::Shape { outer: corners.clone(), holes: Vec::new() };
                        crate::polygons::intersection(&[shape], &[crate::polygons::Shape { outer: o.clone(), holes: Vec::new() }]).iter().map(crate::polygons::Shape::area).sum::<f64>() < 1e-6
                    });
                if clear {
                    centres.push(world(u, v));
                    placed_in_row = true;
                }
                u += module.width + gap;
            }
            if placed_in_row {
                rows += 1;
            }
            v += pitch;
        }
    }
    if centres.is_empty() {
        notes.push("no module fits: the roof is covered, or too small for the setback and clearances".into());
    }
    let rec = PvRecipe { kind: RECIPE.into(), level: level_guid, module, tilt, facing, modules: centres.clone() };
    let array = txn.create(
        NewElement::new("IfcSolarDevice")
            .name(p.name.as_deref().unwrap_or("PV array"))
            .param("@PredefinedType", "SOLARPANEL")
            .param(RECIPE_KEY, serde_json::to_value(&rec).expect("serialises")),
    )?;
    Ok(Array {
        array,
        modules: centres.len(),
        rows,
        kwp: centres.len() as f64 * module.watts / 1000.0,
        area: centres.len() as f64 * module.width * module.length,
        row_pitch: pitch,
        kept_clear_of,
        notes,
    })
}
