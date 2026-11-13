// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Sun-control baffles (spec Ch 3, ARC-GLZ-019; `GLAZED_ENCLOSURES_PLAN.md` §5.2, phase 3): rows
//! of blades hung under (or over) a glazed roof, and how much of the direct sun they let onto
//! the areas below.
//!
//! - **A baffle array** (`shading.baffles`) is an `IfcShadingDevice` (USERDEFINED, ObjectType
//!   `Baffle`): parallel blades `spacing` apart running along `direction` across a plan region,
//!   their top edges `height` above a level, each `depth` deep and `thickness` thick, tilted
//!   `tilt` from vertical (toward the blades' left).
//! - **Their effect** on a sun direction, in the plane across the blades: a blade's shadow on the
//!   plane of the blades' tops is |depth·sin tilt + depth·cos tilt · tan θ| + thickness, where θ
//!   is the sun's profile angle from vertical across the blades; the share blocked is that over
//!   the spacing, at most all of it. The cut-off is the profile angle beyond which nothing gets
//!   through.
//! - **The sun check** (`shading.sun_check`) follows the sun hour by hour on chosen days (by
//!   default the solstices and equinoxes) at the project's location, from points on each area (a
//!   pool's water, a room's floor, or an outline) toward the sun. A ray through a baffle array's
//!   region is cut to what its blades let through; others pass. Each area's share of the clear
//!   sky's direct sun on it is the hours' admitted shares weighted by the sun's height (cos of the
//!   zenith), and is held to `max_share`.
//!
//! Not built yet: louvres, fins and brise-soleil on façades, operable blades and their control,
//! light shelves, screens, the glazing and the building's own shading in the check, diffuse light,
//! and the devices in the energy and daylight analyses.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::bodies::Mesh;
use crate::element::{Element, NewElement};
use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::store::Store;
use crate::txn::Transaction;
use crate::walls::RECIPE_KEY;

pub const RECIPE: &str = "recipe.baffles";

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

/// A baffle array (`recipe.baffles`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BaffleRecipe {
    pub kind: String,
    /// The level (GlobalId) and the blades' tops above it.
    pub level: String,
    pub height: f64,
    /// The region the blades fill, counter-clockwise.
    pub outline: Vec<[f64; 2]>,
    /// The blades' direction in plan (radians, counter-clockwise from x).
    pub direction: f64,
    pub spacing: f64,
    pub depth: f64,
    pub thickness: f64,
    /// From vertical, radians; positive leans the blades' bottoms to their left.
    pub tilt: f64,
}

pub fn recipe(e: &Element) -> Option<BaffleRecipe> {
    let v = e.param(RECIPE_KEY)?;
    (v.get("kind")?.as_str()? == RECIPE).then(|| serde::Deserialize::deserialize(v).ok())?
}

impl BaffleRecipe {
    fn axes(&self) -> ([f64; 2], [f64; 2]) {
        let (s, c) = self.direction.sin_cos();
        ([c, s], [-s, c])
    }

    /// The share of a sun direction (unit, z up) the blades block.
    pub fn blocked(&self, sun: [f64; 3]) -> f64 {
        if sun[2] <= 1e-9 {
            return 1.0;
        }
        let (_, n) = self.axes();
        let across = sun[0] * n[0] + sun[1] * n[1];
        let shadow = (self.depth * self.tilt.sin() + self.depth * self.tilt.cos() * across / sun[2]).abs() + self.thickness;
        (shadow / self.spacing).min(1.0)
    }

    /// The profile angles (from vertical, degrees, toward the blades' left and right) beyond
    /// which no direct sun gets through.
    pub fn cutoff(&self) -> [f64; 2] {
        let open = (self.spacing - self.thickness) / self.depth;
        let (s, c) = self.tilt.sin_cos();
        let c = c.max(1e-9);
        [((open - s) / c).atan().to_degrees(), ((open + s) / c).atan().to_degrees()]
    }

    /// The blades' centre lines in plan, clipped to the region.
    pub fn lines(&self) -> Vec<[[f64; 2]; 2]> {
        let (u, n) = self.axes();
        let r = &self.outline;
        if r.len() < 3 || self.spacing <= 0.0 {
            return Vec::new();
        }
        let across: Vec<f64> = r.iter().map(|p| p[0] * n[0] + p[1] * n[1]).collect();
        let (lo, hi) = (across.iter().copied().fold(f64::MAX, f64::min), across.iter().copied().fold(f64::MIN, f64::max));
        let mut out = Vec::new();
        let count = ((hi - lo) / self.spacing).floor() as i64;
        let first = lo + ((hi - lo) - count as f64 * self.spacing) / 2.0;
        for k in 0..=count {
            let c = first + k as f64 * self.spacing;
            // The line {p · n = c}: where the region's edges cross it, along u.
            let mut ts = Vec::new();
            for i in 0..r.len() {
                let (a, b) = (r[i], r[(i + 1) % r.len()]);
                let (da, db) = (across[i] - c, across[(i + 1) % r.len()] - c);
                if (da < 0.0) != (db < 0.0) {
                    let f = da / (da - db);
                    let p = [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f];
                    ts.push(p[0] * u[0] + p[1] * u[1]);
                }
            }
            ts.sort_by(f64::total_cmp);
            for w in ts.chunks(2) {
                if let [t0, t1] = *w
                    && t1 - t0 > 1e-6
                {
                    let at = |t: f64| [u[0] * t + n[0] * c, u[1] * t + n[1] * c];
                    out.push([at(t0), at(t1)]);
                }
            }
        }
        out
    }

    /// Each blade's solid.
    pub fn meshes(&self, z_top: f64) -> Vec<Mesh> {
        let (_, n) = self.axes();
        let (dx, dz) = (self.depth * self.tilt.sin(), self.depth * self.tilt.cos());
        let h = self.thickness / 2.0;
        self.lines()
            .into_iter()
            .map(|[a, b]| {
                let top = [[a[0] - n[0] * h, a[1] - n[1] * h], [b[0] - n[0] * h, b[1] - n[1] * h], [b[0] + n[0] * h, b[1] + n[1] * h], [a[0] + n[0] * h, a[1] + n[1] * h]];
                let bottom: Vec<[f64; 3]> = top.iter().map(|p| [p[0] + n[0] * dx, p[1] + n[1] * dx, z_top - dz]).collect();
                let mut v = bottom;
                v.extend(top.iter().map(|p| [p[0], p[1], z_top]));
                let i = vec![0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0, 4, 3, 4, 7];
                (v, i)
            })
            .collect()
    }
}

/// Every baffle array's blades, for drawing and IFC.
pub fn all(store: &Store) -> Vec<(ElementId, Vec<Mesh>)> {
    store
        .iter()
        .filter_map(|e| {
            let r = recipe(e)?;
            let z = crate::levels::elevation_of(store, &r.level).ok()? + r.height;
            Some((e.id(), r.meshes(z)))
        })
        .collect()
}

// ---- Commands ----

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BafflesParams {
    /// The region in plan.
    pub outline: Vec<[f64; 2]>,
    pub level: ElementRef,
    /// The blades' tops above the level.
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub height: f64,
    /// The blades' direction in plan (default 0: along x; radians or "90°").
    #[schemars(extend("x-riviera-data-type" = "angle"))]
    pub direction: Option<Value>,
    /// Default 0.6 m apart, 0.4 m deep, 25 mm thick, vertical.
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub spacing: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub depth: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub thickness: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "angle"))]
    pub tilt: Option<Value>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Baffles {
    pub device: ElementId,
    pub blades: usize,
    /// Metres of blade.
    pub length: f64,
    /// The cut-off profile angles from vertical, degrees, each side.
    pub cutoff: [f64; 2],
}

fn angle(v: &Option<Value>, default: f64) -> Result<f64> {
    match v {
        None => Ok(default),
        Some(Value::Number(n)) => Ok(n.as_f64().unwrap_or(default)),
        Some(Value::String(s)) => {
            let t = s.trim();
            match t.strip_suffix('°') {
                Some(d) => d.trim().parse::<f64>().map(f64::to_radians).map_err(|_| bad("shading.angle", format!("`{s}` is not an angle"))),
                None => t.parse::<f64>().map_err(|_| bad("shading.angle", format!("`{s}` is not an angle"))),
            }
        }
        _ => Err(bad("shading.angle", "an angle is radians or text such as \"30°\"")),
    }
}

/// `shading.baffles`.
pub(crate) fn baffles(txn: &mut Transaction<'_>, p: BafflesParams) -> Result<Baffles> {
    let store = txn.store().clone();
    let level = store.resolve(&p.level).and_then(|id| store.get(id)).filter(|e| e.class == "IfcBuildingStorey").ok_or_else(|| bad("shading.level", "choose a level"))?;
    let mut outline = p.outline.clone();
    if crate::polygons::area(&outline) < 0.0 {
        outline.reverse();
    }
    let rec = BaffleRecipe {
        kind: RECIPE.into(),
        level: level.guid().to_string(),
        height: p.height,
        outline,
        direction: angle(&p.direction, 0.0)?,
        spacing: p.spacing.unwrap_or(0.6),
        depth: p.depth.unwrap_or(0.4),
        thickness: p.thickness.unwrap_or(0.025),
        tilt: angle(&p.tilt, 0.0)?,
    };
    if rec.outline.len() < 3 || crate::polygons::area(&rec.outline) < 0.1 {
        return Err(bad("shading.outline", "baffles need a region of at least three corners"));
    }
    if !(rec.spacing > rec.thickness && rec.depth > 0.0 && rec.thickness > 0.0 && rec.tilt.abs() < 80f64.to_radians()) {
        return Err(bad("shading.blades", "blades need a depth, a thickness less than their spacing, and a tilt under 80°"));
    }
    let lines = rec.lines();
    let length = lines.iter().map(|[a, b]| (b[0] - a[0]).hypot(b[1] - a[1])).sum();
    let device = txn.create(
        NewElement::new("IfcShadingDevice")
            .name(p.name.as_deref().unwrap_or("Baffles"))
            .param("@PredefinedType", "USERDEFINED")
            .param("@ObjectType", "Baffle")
            .param(RECIPE_KEY, serde_json::to_value(&rec).expect("serialises")),
    )?;
    Ok(Baffles { device, blades: lines.len(), length, cutoff: rec.cutoff() })
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Area {
    pub name: String,
    pub outline: Vec<[f64; 2]>,
    /// Its height (absolute, metres).
    pub z: f64,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SunCheckParams {
    /// Pools (their water) and rooms (their floors) to check.
    #[serde(default)]
    pub elements: Vec<ElementRef>,
    /// Or areas given by outline.
    #[serde(default)]
    pub areas: Vec<Area>,
    /// The baffle arrays that count (default all).
    #[serde(default)]
    pub devices: Vec<ElementRef>,
    /// Days as [month, day] (default 21 March, June, September and December).
    #[serde(default)]
    pub days: Vec<[u8; 2]>,
    /// The most of the direct sun an area may get (default 0.25).
    pub max_share: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DayResult {
    pub day: [u8; 2],
    /// Each hour with the sun up: the local standard hour and the share admitted.
    pub hours: Vec<(f64, f64)>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AreaResult {
    pub name: String,
    pub area: f64,
    /// The share of the clear sky's direct sun reaching it, weighted by the sun's height.
    pub share: f64,
    /// The highest hour's share, and when.
    pub peak: f64,
    pub peak_at: ([u8; 2], f64),
    pub days: Vec<DayResult>,
    pub passes: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SunCheck {
    pub latitude: f64,
    pub longitude: f64,
    pub max_share: f64,
    pub areas: Vec<AreaResult>,
    pub notes: Vec<String>,
}

/// Points on an area: a grid at about a twentieth of its size, at most 400.
fn points(r: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in r {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    let step = ((hi[0] - lo[0]).max(hi[1] - lo[1]) / 20.0).max(0.25);
    let mut out = Vec::new();
    let mut y = lo[1] + step / 2.0;
    while y < hi[1] {
        let mut x = lo[0] + step / 2.0;
        while x < hi[0] {
            if crate::sco::inside([x, y], r) {
                out.push([x, y]);
            }
            x += step;
        }
        y += step;
    }
    if out.is_empty() {
        let n = r.len().max(1) as f64;
        out.push([r.iter().map(|p| p[0]).sum::<f64>() / n, r.iter().map(|p| p[1]).sum::<f64>() / n]);
    }
    out.truncate(400);
    out
}

/// `shading.sun_check`.
pub(crate) fn sun_check(store: &Store, p: Option<SunCheckParams>) -> Result<SunCheck> {
    let p = p.unwrap_or_default();
    let loc = crate::location::get(store);
    let mut notes = Vec::new();
    let (lat, lon) = match (loc.latitude, loc.longitude) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            notes.push("the project has no location (Manage › Location): the sun is followed at 40° N, 0° E".into());
            (40.0, 0.0)
        }
    };
    let tz = loc.time_zone.unwrap_or((lon / 15.0).round());
    let input = crate::solar::SolarInput { latitude: lat, longitude: lon, time_zone: tz, true_north: loc.true_north, from: [1, 1], to: [12, 31], hours: Vec::new(), faces: Vec::new(), shading: Vec::new(), grid: 1.0, pv: None };
    // The devices and the planes of their blades' tops.
    let wanted: Vec<ElementId> = p.devices.iter().map(|r| store.resolve(r).ok_or_else(|| bad("element.not_found", format!("no element {r:?}")))).collect::<Result<_>>()?;
    let devices: Vec<(BaffleRecipe, f64)> = store
        .iter()
        .filter(|e| wanted.is_empty() || wanted.contains(&e.id()))
        .filter_map(|e| {
            let r = recipe(e)?;
            let z = crate::levels::elevation_of(store, &r.level).ok()? + r.height;
            Some((r, z))
        })
        .collect();
    if devices.is_empty() {
        notes.push("no baffles: every area gets the whole of the direct sun".into());
    }
    let mut areas: Vec<Area> = p.areas.clone();
    for r in &p.elements {
        let id = store.resolve(r).ok_or_else(|| bad("element.not_found", format!("no element {r:?}")))?;
        let e = store.get(id).expect("resolved");
        if let Some(rec) = crate::pools::recipe(e) {
            let g = crate::pools::regenerate(store, e)?;
            areas.push(Area { name: e.name.clone().unwrap_or_else(|| "Pool".into()), outline: rec.outline, z: g.water_z });
        } else if crate::rooms::room_recipe(e).is_some() {
            let g = crate::rooms::regenerate(store, e)?;
            let region = g.region.ok_or_else(|| bad("shading.room", format!("room {id} is not enclosed")))?;
            let (number, name) = crate::rooms::number_and_name(e);
            areas.push(Area { name: format!("{number} {name}").trim().to_owned(), outline: region.outer.iter().map(|x| x.line.start).collect(), z: g.z_base });
        } else {
            return Err(bad("shading.area", format!("element {id} is neither a pool nor a room")));
        }
    }
    if areas.is_empty() {
        return Err(bad("shading.area", "choose the areas to check: pools, rooms or outlines"));
    }
    let days = if p.days.is_empty() { vec![[3, 21], [6, 21], [9, 21], [12, 21]] } else { p.days.clone() };
    let max_share = p.max_share.unwrap_or(0.25);
    let doy = |[m, d]: [u8; 2]| -> u32 { [0u32, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][(m.clamp(1, 12) - 1) as usize] + u32::from(d) };
    let mut out = Vec::new();
    for a in &areas {
        let pts = points(&a.outline);
        let (mut num, mut den, mut peak, mut peak_at) = (0.0, 0.0, 0.0, ([0u8, 0u8], 0.0));
        let mut results = Vec::new();
        for day in &days {
            let mut hours = Vec::new();
            for h in 0..24 {
                // At the middle of each hour (local standard time).
                let hour = h as f64 + 0.5;
                let Some(sun) = crate::solar::sun(&input, doy(*day), hour) else { continue };
                // Each point's admitted share through the arrays its ray crosses.
                let share = pts
                    .iter()
                    .map(|q| {
                        devices
                            .iter()
                            .filter(|(_, z)| *z > a.z)
                            .map(|(d, z)| {
                                let t = (z - a.z) / sun[2];
                                let x = [q[0] + sun[0] * t, q[1] + sun[1] * t];
                                if crate::sco::inside(x, &d.outline) { 1.0 - d.blocked(sun) } else { 1.0 }
                            })
                            .product::<f64>()
                    })
                    .sum::<f64>()
                    / pts.len() as f64;
                num += share * sun[2];
                den += sun[2];
                if share > peak + 1e-12 {
                    peak = share;
                    peak_at = (*day, hour);
                }
                hours.push((hour, share));
            }
            results.push(DayResult { day: *day, hours });
        }
        let share = if den > 0.0 { num / den } else { 0.0 };
        out.push(AreaResult { name: a.name.clone(), area: crate::polygons::area(&a.outline).abs(), share, peak, peak_at, days: results, passes: share <= max_share + 1e-9 });
    }
    Ok(SunCheck { latitude: lat, longitude: lon, max_share, areas: out, notes })
}
