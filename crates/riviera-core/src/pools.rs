// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Swimming pools (ARC-POOL-001, `POOLS_PLAN.md` §3): one element from an outline or a template,
//! its depth profile, edge and coping, with its shell, water and read-outs generated from them.
//!
//! The **axis** runs from the shallow end to the deep end; `t` is the distance along it. The
//! floor's depth below the water is linear between the profile's stations and constant beyond the
//! first and last, and constant across the axis. Each part is cut into strips between stations,
//! so each strip's floor is one plane:
//! - the **water**, from the floor up to the water level, inside the outline;
//! - the **base slab** under the floor, the shell's thickness deep;
//! - the **shell walls**, the shell's thickness outside the outline, from the base slab's bottom
//!   up to the coping (or the deck);
//! - the **coping** round the edges that have it, overhanging the water.
//!
//! The pool cuts its outline, grown by the shell or the coping, out of the floors on its level
//! (each a vertical opening, ARC-OPEN-002, made by the pool and deleted with it). In IFC it is an
//! `IfcElementAssembly` aggregating its walls, base slab and coping, with the water an `IfcSpace`
//! (WATER): the owner's answer to the plan's Question 1.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::element::{Element, NewElement};
use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::polygons::{Ring, Shape};
use crate::store::Store;
use crate::txn::Transaction;

pub const RECIPE: &str = "recipe.pool";
const RECIPE_KEY: &str = crate::walls::RECIPE_KEY;
pub const CLASS: &str = "IfcElementAssembly";
/// On a floor opening the pool made: the pool's GlobalId.
pub const CUT_KEY: &str = "rxd:pool_cut";
const FT: f64 = 0.3048;
/// The break point's depth: 5 ft, where slope limits change (research/23 A.2).
pub const BREAK: f64 = 5.0 * FT;

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

/// A pool's edge, per outline edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Edge {
    /// Skimmers in the wall below the coping.
    #[default]
    Skimmer,
    /// A deck-level overflow gutter (Finnish, Wiesbaden or Zurich), its width.
    DeckLevel {
        #[serde(default)]
        gutter: Gutter,
        #[serde(default = "gutter_width")]
        width: f64,
    },
    /// A vanishing edge over a catch basin, its width and depth.
    Infinity {
        #[serde(default = "basin_width")]
        basin_width: f64,
        #[serde(default = "basin_depth")]
        basin_depth: f64,
    },
    /// The wall raised above the deck.
    Raised { height: f64 },
}

fn gutter_width() -> f64 {
    0.3
}
fn basin_width() -> f64 {
    0.45
}
fn basin_depth() -> f64 {
    0.6
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Gutter {
    #[default]
    Finnish,
    Wiesbaden,
    Zurich,
}

impl Edge {
    fn has_coping(&self) -> bool {
        matches!(self, Edge::Skimmer | Edge::Raised { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Coping {
    /// Its width from the pool's inner face (default 0.3 m).
    pub width: f64,
    /// Over the water (default 40 mm: the MAHC's 1–2 in).
    pub overhang: f64,
    /// Default 60 mm (the MAHC's 2.5 in at most).
    pub thickness: f64,
}

impl Default for Coping {
    fn default() -> Self {
        Coping { width: 0.3, overhang: 0.04, thickness: 0.06 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellKind {
    #[default]
    CastConcrete,
    Shotcrete,
    Panel,
    OnePiece,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Shell {
    pub kind: ShellKind,
    /// Wall and floor thickness (default 0.3 m).
    pub thickness: f64,
    /// A tiling layer's thickness, if tiled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tile: Option<f64>,
    pub membrane: bool,
}

impl Default for Shell {
    fn default() -> Self {
        Shell { kind: ShellKind::CastConcrete, thickness: 0.3, tile: None, membrane: false }
    }
}

/// A profile station: its distance along the axis and the depth below the water there.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Station {
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub at: f64,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub depth: f64,
}

/// A pool (`recipe.pool`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PoolRecipe {
    pub kind: String,
    /// The level (GlobalId).
    pub level: String,
    /// The deck's height above the level.
    pub deck: f64,
    /// The outline, counter-clockwise; its first edge is the shallow end.
    pub outline: Ring,
    /// The water level's depth below the deck.
    pub water_level: f64,
    /// From the shallow end toward the deep end.
    pub axis: [[f64; 2]; 2],
    pub profile: Vec<Station>,
    /// The wall–floor cove's radius.
    pub cove: f64,
    /// One per outline edge.
    pub edges: Vec<Edge>,
    pub coping: Coping,
    pub shell: Shell,
    /// The template it was made from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

pub fn recipe(e: &Element) -> Option<PoolRecipe> {
    let v = e.param(RECIPE_KEY)?;
    if v.get("kind")?.as_str()? != RECIPE {
        return None;
    }
    serde_json::from_value(v.clone()).ok()
}

/// Whether the pool `e` depends on `guid` (its level).
pub fn uses(e: &Element, guid: &str) -> bool {
    recipe(e).is_some_and(|r| r.level == guid) || e.param(CUT_KEY).and_then(Value::as_str) == Some(guid)
}

// ---- Geometry ----

type Mesh = crate::bodies::Mesh;

/// A pool as it is now.
#[derive(Debug, Clone)]
pub struct PoolGeometry {
    pub water: Vec<Mesh>,
    pub base: Vec<Mesh>,
    pub walls: Vec<Mesh>,
    pub coping: Vec<Mesh>,
    pub deck_z: f64,
    pub water_z: f64,
    pub area: f64,
    pub perimeter: f64,
    pub volume: f64,
    pub shell_volume: f64,
}

impl PoolRecipe {
    fn axis_dir(&self) -> ([f64; 2], [f64; 2]) {
        let [a, b] = self.axis;
        let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
        (a, [(b[0] - a[0]) / l, (b[1] - a[1]) / l])
    }

    /// The distance along the axis of a plan point.
    pub fn t(&self, p: [f64; 2]) -> f64 {
        let (a, u) = self.axis_dir();
        (p[0] - a[0]) * u[0] + (p[1] - a[1]) * u[1]
    }

    /// The depth below the water at a distance along the axis.
    pub fn depth_at(&self, t: f64) -> f64 {
        let s = &self.profile;
        if s.is_empty() {
            return 0.0;
        }
        if t <= s[0].at {
            return s[0].depth;
        }
        for w in s.windows(2) {
            if t <= w[1].at {
                let f = if w[1].at - w[0].at > 1e-9 { (t - w[0].at) / (w[1].at - w[0].at) } else { 1.0 };
                return w[0].depth + (w[1].depth - w[0].depth) * f;
            }
        }
        s[s.len() - 1].depth
    }
}

/// The ring offset by `d` (outward for a counter-clockwise ring), its corners mitred.
pub fn offset(r: &[[f64; 2]], d: f64) -> Ring {
    let n = r.len();
    let normal = |i: usize| {
        let (a, b) = (r[i], r[(i + 1) % n]);
        let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
        // Outward of a counter-clockwise ring: to the right of each edge.
        [(b[1] - a[1]) / l, -(b[0] - a[0]) / l]
    };
    (0..n)
        .map(|i| {
            let (n0, n1) = (normal((i + n - 1) % n), normal(i));
            let m = [n0[0] + n1[0], n0[1] + n1[1]];
            let dot = (m[0] * n1[0] + m[1] * n1[1]).max(0.2);
            [r[i][0] + m[0] * d / dot, r[i][1] + m[1] * d / dot]
        })
        .collect()
}

/// `region` cut into strips between the stations, each with its own plane of floor.
fn strips(rec: &PoolRecipe, region: &[Shape]) -> Vec<Shape> {
    let (a, u) = rec.axis_dir();
    let v = [-u[1], u[0]];
    let big = 1000.0;
    let mut cuts: Vec<f64> = rec.profile.iter().map(|s| s.at).collect();
    cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    let mut bounds = vec![-big];
    bounds.extend(cuts);
    bounds.push(big);
    let mut out = Vec::new();
    for w in bounds.windows(2) {
        let (t0, t1) = (w[0], w[1]);
        let p = |t: f64, s: f64| [a[0] + u[0] * t + v[0] * s, a[1] + u[1] * t + v[1] * s];
        let rect = Shape { outer: vec![p(t0, -big), p(t1, -big), p(t1, big), p(t0, big)], holes: Vec::new() };
        out.extend(crate::polygons::intersection(region, &[rect]));
    }
    out
}

fn extrude(s: &Shape, bottom: &dyn Fn([f64; 2]) -> f64, top: &dyn Fn([f64; 2]) -> f64) -> Mesh {
    let mut outer = s.outer.clone();
    if crate::polygons::area(&outer) < 0.0 {
        outer.reverse();
    }
    let holes: Vec<Ring> = s
        .holes
        .iter()
        .map(|h| {
            let mut h = h.clone();
            if crate::polygons::area(&h) > 0.0 {
                h.reverse();
            }
            h
        })
        .collect();
    crate::bodies::extrude_between(&outer, &holes, bottom, top)
}

/// The volume between two functions over a strip, exactly for planes (the triangles' centroids).
fn mesh_volume(m: &Mesh) -> f64 {
    let (p, idx) = m;
    idx.as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let [a, b, c] = [p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]];
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0
        })
        .sum::<f64>()
        .abs()
}

/// Regenerates a pool.
pub fn regenerate(store: &Store, e: &Element) -> Result<PoolGeometry> {
    let rec = recipe(e).ok_or_else(|| bad("pool.not_a_pool", "not a pool"))?;
    let z0 = crate::levels::elevation_of(store, &rec.level)?;
    let deck_z = z0 + rec.deck;
    let water_z = deck_z - rec.water_level;
    let floor = |p: [f64; 2]| water_z - rec.depth_at(rec.t(p));
    let t = rec.shell.thickness;
    let outline = Shape { outer: rec.outline.clone(), holes: Vec::new() };
    let coping_all = rec.edges.iter().all(Edge::has_coping);
    let any_coping = rec.edges.iter().any(Edge::has_coping);
    let wall_top = deck_z - if any_coping { rec.coping.thickness } else { 0.0 };
    let water: Vec<Mesh> = strips(&rec, std::slice::from_ref(&outline)).iter().map(|s| extrude(s, &floor, &|_| water_z)).collect();
    let base: Vec<Mesh> = strips(&rec, std::slice::from_ref(&outline)).iter().map(|s| extrude(s, &|p| floor(p) - t, &floor)).collect();
    let ring = crate::polygons::difference(&[Shape { outer: offset(&rec.outline, t), holes: Vec::new() }], std::slice::from_ref(&rec.outline));
    let walls: Vec<Mesh> = strips(&rec, &ring).iter().map(|s| extrude(s, &|p| floor(p) - t, &|_| wall_top)).collect();
    let mut coping = Vec::new();
    if any_coping {
        let c = rec.coping;
        let inner = offset(&rec.outline, -c.overhang);
        let outer = offset(&rec.outline, c.width);
        let region = if coping_all {
            crate::polygons::difference(&[Shape { outer, holes: Vec::new() }], &[inner])
        } else {
            // Each edge that has it, square-ended.
            let n = rec.outline.len();
            (0..n)
                .filter(|i| rec.edges.get(*i).is_some_and(Edge::has_coping))
                .flat_map(|i| {
                    let (a, b) = (rec.outline[i], rec.outline[(i + 1) % n]);
                    let quad = vec![a, b];
                    let o = offset_segment(&quad, c.width);
                    let ii = offset_segment(&quad, -c.overhang);
                    crate::polygons::union(&[vec![ii[0], ii[1], o[1], o[0]]])
                })
                .collect()
        };
        for s in region {
            coping.push(extrude(&s, &|_| deck_z - c.thickness, &|_| deck_z));
        }
    }
    let area = crate::polygons::area(&rec.outline).abs();
    let n = rec.outline.len();
    let perimeter = (0..n).map(|i| {
        let (a, b) = (rec.outline[i], rec.outline[(i + 1) % n]);
        (b[0] - a[0]).hypot(b[1] - a[1])
    }).sum();
    let volume = water.iter().map(mesh_volume).sum();
    let shell_volume = base.iter().chain(&walls).map(mesh_volume).sum();
    Ok(PoolGeometry { water, base, walls, coping, deck_z, water_z, area, perimeter, volume, shell_volume })
}

/// A segment offset outward (to its right) by `d`.
fn offset_segment(s: &[[f64; 2]], d: f64) -> [[f64; 2]; 2] {
    let (a, b) = (s[0], s[1]);
    let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
    let n = [(b[1] - a[1]) / l * d, -(b[0] - a[0]) / l * d];
    [[a[0] + n[0], a[1] + n[1]], [b[0] + n[0], b[1] + n[1]]]
}

/// Every pool that regenerates.
pub fn all(store: &Store) -> Vec<(ElementId, PoolGeometry)> {
    store.iter().filter(|e| recipe(e).is_some()).filter_map(|e| Some((e.id(), regenerate(store, e).ok()?))).collect()
}

// ---- Commands ----

/// What a template makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TemplateKind {
    Rectangle,
    Lap,
    /// 25 yd short course (22.86 m), with touch pads.
    #[serde(rename = "competition_25yd")]
    Competition25yd,
    /// 25 m, with touch pads.
    #[serde(rename = "competition_25m")]
    Competition25m,
    /// 50 m, with touch pads.
    #[serde(rename = "competition_50m")]
    Competition50m,
    /// 12 × 7 m, 0.9 m deep (Sport England).
    Teaching,
    /// 0.45 m deep.
    Wading,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub kind: TemplateKind,
    /// The pool's centre.
    pub origin: [f64; 2],
    /// The axis's direction, counter-clockwise from +x (default 0: shallow end west).
    #[serde(default)]
    #[schemars(extend("x-riviera-data-type" = "angle"))]
    pub rotation: f64,
    /// Rectangle, lap pool: length along the axis and width.
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub length: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub width: Option<f64>,
    /// Competition pools: lanes (default 8 for 50 m, 6 otherwise) and their width (default 2.5 m).
    pub lanes: Option<u32>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub lane_width: Option<f64>,
}

/// Touch pads at one end: 0.03 m (research/23 D.1.1).
const PADS: f64 = 0.03;

fn template(t: &Template) -> Result<(Ring, Vec<Station>, f64)> {
    use TemplateKind::*;
    let lanes = |default: u32| f64::from(t.lanes.unwrap_or(default)) * t.lane_width.unwrap_or(2.5);
    let (l, w, profile): (f64, f64, Option<Vec<Station>>) = match t.kind {
        Rectangle => (t.length.unwrap_or(10.0), t.width.unwrap_or(5.0), None),
        Lap => (t.length.unwrap_or(20.0), t.width.unwrap_or(2.5), Some(vec![Station { at: 0.0, depth: 1.2 }])),
        Competition25yd => (22.86 + PADS, lanes(6), Some(vec![Station { at: 0.0, depth: 2.0 }])),
        Competition25m => (25.0 + PADS, lanes(6), Some(vec![Station { at: 0.0, depth: 2.0 }])),
        Competition50m => (50.0 + PADS, lanes(8), Some(vec![Station { at: 0.0, depth: 2.0 }])),
        Teaching => (t.length.unwrap_or(12.0), t.width.unwrap_or(7.0), Some(vec![Station { at: 0.0, depth: 0.9 }])),
        Wading => (t.length.unwrap_or(6.0), t.width.unwrap_or(4.0), Some(vec![Station { at: 0.0, depth: 0.45 }])),
    };
    if !(l > 0.5 && w > 0.5) {
        return Err(bad("pool.too_small", "a pool needs to be more than 0.5 m each way"));
    }
    let (c, s) = (t.rotation.cos(), t.rotation.sin());
    let at = |x: f64, y: f64| [t.origin[0] + x * c - y * s, t.origin[1] + x * s + y * c];
    // Counter-clockwise from the shallow end's wall (its first edge).
    let ring = vec![at(-l / 2.0, w / 2.0), at(-l / 2.0, -w / 2.0), at(l / 2.0, -w / 2.0), at(l / 2.0, w / 2.0)];
    Ok((ring, profile.unwrap_or_else(|| default_profile(l)), l))
}

/// A public pool's: 1.0 m deep, to 5 ft at 60 % of its length, to 2.0 m at the deep end.
fn default_profile(l: f64) -> Vec<Station> {
    vec![Station { at: 0.0, depth: 1.0 }, Station { at: 0.6 * l, depth: BREAK }, Station { at: l, depth: 2.0 }]
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateParams {
    pub level: ElementRef,
    /// A closed outline in plan (metres); its first edge is the shallow end. Or a template.
    pub outline: Option<Ring>,
    pub template: Option<Template>,
    /// The deck's height above the level (default 0).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub deck: Option<f64>,
    /// The water's depth below the deck (default 0.15 m; 0 when every edge is deck-level).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub water_level: Option<f64>,
    /// From the shallow end toward the deep end (default from the first edge's middle, square to it).
    pub axis: Option<[[f64; 2]; 2]>,
    /// The depths along the axis (default the template's, or 1.0 m to 2.0 m with the break at 60 %).
    pub profile: Option<Vec<Station>>,
    /// The wall–floor cove's radius (default 150 mm).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub cove: Option<f64>,
    /// One edge for all, or one per outline edge (default skimmer).
    pub edges: Option<Vec<Edge>>,
    pub coping: Option<Coping>,
    pub shell: Option<Shell>,
    pub name: Option<String>,
}

fn level_guid(store: &Store, r: &ElementRef) -> Result<String> {
    store
        .resolve(r)
        .and_then(|id| store.get(id))
        .filter(|e| e.class == crate::levels::CLASS)
        .map(|e| e.guid().as_str().to_owned())
        .ok_or_else(|| bad("element.not_found", "no such level"))
}

/// The axis from the first edge's middle, square to it into the pool, as far as the pool reaches.
fn default_axis(r: &[[f64; 2]]) -> [[f64; 2]; 2] {
    let (a, b) = (r[0], r[1]);
    let m = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
    // Inward of a counter-clockwise ring: to the left of the edge.
    let n = [-(b[1] - a[1]) / l, (b[0] - a[0]) / l];
    let reach = r.iter().map(|p| (p[0] - m[0]) * n[0] + (p[1] - m[1]) * n[1]).fold(0.0, f64::max);
    [m, [m[0] + n[0] * reach.max(0.1), m[1] + n[1] * reach.max(0.1)]]
}

fn check(rec: &PoolRecipe) -> Result<()> {
    if rec.outline.len() < 3 || crate::polygons::area(&rec.outline).abs() < 0.25 {
        return Err(bad("pool.bad_outline", "a pool's outline needs three corners and some area"));
    }
    if rec.profile.is_empty() || rec.profile.iter().any(|s| s.depth.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)) {
        return Err(bad("pool.bad_profile", "every station needs a depth above zero"));
    }
    if rec.profile.windows(2).any(|w| w[1].at < w[0].at) {
        return Err(bad("pool.bad_profile", "the stations must run from the shallow end to the deep end"));
    }
    if rec.edges.len() != rec.outline.len() {
        return Err(bad("pool.bad_edges", format!("give one edge or one per outline edge ({})", rec.outline.len())));
    }
    if rec.shell.thickness.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) || rec.water_level.partial_cmp(&0.0).is_none_or(std::cmp::Ordering::is_lt) {
        return Err(bad("pool.bad_shell", "the shell needs a thickness, and the water must be at or below the deck"));
    }
    Ok(())
}

fn edges_for(n: usize, edges: Option<Vec<Edge>>) -> Vec<Edge> {
    match edges {
        Some(e) if e.len() == 1 => vec![e[0]; n],
        Some(e) => e,
        None => vec![Edge::Skimmer; n],
    }
}

/// Makes a pool (`pool.create`).
pub(crate) fn create(txn: &mut Transaction<'_>, p: CreateParams) -> Result<ElementId> {
    let store = txn.store().clone();
    let level = level_guid(&store, &p.level)?;
    let (mut outline, tprofile, kind) = match (&p.outline, &p.template) {
        (Some(o), None) => (o.clone(), None, None),
        (None, Some(t)) => {
            let (r, prof, _) = template(t)?;
            (r, Some(prof), Some(serde_json::to_value(t.kind).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()))
        }
        _ => return Err(bad("pool.outline_or_template", "give an outline or a template")),
    };
    if crate::polygons::area(&outline) < 0.0 {
        // Counter-clockwise, keeping the first edge first.
        outline.reverse();
        outline.rotate_right(1);
    }
    let axis = p.axis.unwrap_or_else(|| default_axis(&outline));
    let length = (axis[1][0] - axis[0][0]).hypot(axis[1][1] - axis[0][1]);
    let edges = edges_for(outline.len(), p.edges);
    let deck_level = edges.iter().all(|e| matches!(e, Edge::DeckLevel { .. }));
    let rec = PoolRecipe {
        kind: RECIPE.into(),
        level,
        deck: p.deck.unwrap_or(0.0),
        water_level: p.water_level.unwrap_or(if deck_level { 0.0 } else { 0.15 }),
        profile: p.profile.or(tprofile).unwrap_or_else(|| default_profile(length)),
        outline,
        axis,
        cove: p.cove.unwrap_or(0.15),
        edges,
        coping: p.coping.unwrap_or_default(),
        shell: p.shell.unwrap_or_default(),
        template: kind,
    };
    check(&rec)?;
    let id = txn.create(
        NewElement::new(CLASS)
            .name(p.name.unwrap_or_else(|| "Swimming pool".into()))
            .param("@PredefinedType", "USERDEFINED")
            .param("@ObjectType", "SWIMMINGPOOL")
            .param(RECIPE_KEY, serde_json::to_value(&rec).expect("serialises")),
    )?;
    cut(txn, id)?;
    Ok(id)
}

/// The pool's openings in its level's floors: made again for its outline now.
fn cut(txn: &mut Transaction<'_>, id: ElementId) -> Result<()> {
    let store = txn.store().clone();
    let e = store.get(id).expect("the pool");
    let rec = recipe(e).expect("a pool");
    let guid = e.guid().as_str().to_owned();
    for old in store.iter().filter(|o| o.param(CUT_KEY).and_then(Value::as_str) == Some(guid.as_str())).map(Element::id).collect::<Vec<_>>() {
        txn.delete(old)?;
    }
    let grow = rec.shell.thickness.max(if rec.edges.iter().any(Edge::has_coping) { rec.coping.width } else { 0.0 });
    let hole = offset(&rec.outline, grow);
    let hole_shape = [Shape { outer: hole.clone(), holes: Vec::new() }];
    for (fid, f) in crate::floors::all(&store) {
        let Some(fe) = store.get(fid) else { continue };
        let Some((crate::floors::SlabKind::Floor, fr)) = crate::floors::slab_recipe(fe) else { continue };
        if fr.level != rec.level {
            continue;
        }
        let meets = f.profiles.iter().any(|pr| !crate::polygons::intersection(&[Shape { outer: pr.outer.clone(), holes: pr.holes.clone() }], &hole_shape).is_empty());
        if !meets {
            continue;
        }
        let o = crate::slab_openings::create(txn, crate::slab_openings::CreateParams { host_id: ElementRef::Id(fid), loops: vec![hole.clone()], curves: Vec::new() })?;
        let g = guid.clone();
        txn.modify(o, |e| {
            e.params.insert(CUT_KEY.into(), Value::String(g));
        })?;
    }
    Ok(())
}

fn resolve(store: &Store, r: &ElementRef) -> Result<(ElementId, PoolRecipe)> {
    let id = store.resolve(r).ok_or_else(|| bad("element.not_found", "no such pool"))?;
    let rec = recipe(store.get(id).expect("resolved")).ok_or_else(|| bad("pool.not_a_pool", format!("element {} is not a pool", id.0)))?;
    Ok((id, rec))
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetParams {
    pub pool_id: ElementRef,
    pub outline: Option<Ring>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub deck: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub water_level: Option<f64>,
    pub axis: Option<[[f64; 2]; 2]>,
    pub profile: Option<Vec<Station>>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub cove: Option<f64>,
    pub edges: Option<Vec<Edge>>,
    pub coping: Option<Coping>,
    pub shell: Option<Shell>,
}

/// Changes a pool (`pool.set`); its shell, water and openings follow.
pub(crate) fn set(txn: &mut Transaction<'_>, p: SetParams) -> Result<()> {
    let (id, mut rec) = resolve(txn.store(), &p.pool_id)?;
    if let Some(mut o) = p.outline {
        if crate::polygons::area(&o) < 0.0 {
            o.reverse();
            o.rotate_right(1);
        }
        if p.axis.is_none() {
            rec.axis = default_axis(&o);
        }
        if p.edges.is_none() && o.len() != rec.outline.len() {
            rec.edges = vec![rec.edges.first().copied().unwrap_or_default(); o.len()];
        }
        rec.outline = o;
        rec.template = None;
    }
    if let Some(v) = p.deck {
        rec.deck = v;
    }
    if let Some(v) = p.water_level {
        rec.water_level = v;
    }
    if let Some(v) = p.axis {
        rec.axis = v;
    }
    if let Some(v) = p.profile {
        rec.profile = v;
    }
    if let Some(v) = p.cove {
        rec.cove = v;
    }
    if let Some(v) = p.edges {
        rec.edges = edges_for(rec.outline.len(), Some(v));
    }
    if let Some(v) = p.coping {
        rec.coping = v;
    }
    if let Some(v) = p.shell {
        rec.shell = v;
    }
    check(&rec)?;
    let v = serde_json::to_value(&rec).expect("serialises");
    txn.modify(id, |e| {
        e.params.insert(RECIPE_KEY.into(), v);
    })?;
    cut(txn, id)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InfoParams {
    pub pool_id: ElementRef,
    /// Plan points to give the depth at.
    #[serde(default)]
    pub points: Vec<[f64; 2]>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Segment {
    pub from: f64,
    pub to: f64,
    /// Rise over run (depth change per metre along the axis).
    pub slope: f64,
    /// As "1:n".
    pub ratio: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Info {
    pub area: f64,
    pub perimeter: f64,
    pub volume: f64,
    /// The depth below the water at each point asked.
    pub depths: Vec<f64>,
    pub segments: Vec<Segment>,
    pub deepest: f64,
    pub shallowest: f64,
    pub shell_volume: f64,
    pub water_level: f64,
}

/// A pool's numbers (`pool.info`).
pub(crate) fn info(store: &Store, p: InfoParams) -> Result<Info> {
    let (id, rec) = resolve(store, &p.pool_id)?;
    let g = regenerate(store, store.get(id).expect("resolved"))?;
    // Depths over the pool: at its corners and wherever a station's line crosses it.
    let ts: Vec<f64> = rec.outline.iter().map(|q| rec.t(*q)).collect();
    let (t0, t1) = ts.iter().fold((f64::MAX, f64::MIN), |(a, b), t| (a.min(*t), b.max(*t)));
    let mut samples: Vec<f64> = rec.profile.iter().map(|s| s.at).filter(|t| (t0..=t1).contains(t)).collect();
    samples.extend([t0, t1]);
    let depths: Vec<f64> = samples.iter().map(|t| rec.depth_at(*t)).collect();
    let segments = rec
        .profile
        .windows(2)
        .filter(|w| w[1].at > w[0].at + 1e-9)
        .map(|w| {
            let slope = (w[1].depth - w[0].depth) / (w[1].at - w[0].at);
            let ratio = if slope.abs() < 1e-9 { "level".into() } else { format!("1:{:.1}", 1.0 / slope.abs()) };
            Segment { from: w[0].at, to: w[1].at, slope, ratio }
        })
        .collect();
    Ok(Info {
        area: g.area,
        perimeter: g.perimeter,
        volume: g.volume,
        depths: p.points.iter().map(|q| rec.depth_at(rec.t(*q))).collect(),
        segments,
        deepest: depths.iter().copied().fold(f64::MIN, f64::max),
        shallowest: depths.iter().copied().fold(f64::MAX, f64::min),
        shell_volume: g.shell_volume,
        water_level: g.water_z,
    })
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Listed {
    pub id: ElementId,
    pub name: String,
    pub area: f64,
    pub volume: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

/// The model's pools (`pool.list`).
pub(crate) fn list(store: &Store, _: Option<Value>) -> Result<Vec<Listed>> {
    Ok(all(store)
        .into_iter()
        .map(|(id, g)| {
            let e = store.get(id).expect("listed");
            Listed { id, name: e.name.clone().unwrap_or_default(), area: g.area, volume: g.volume, template: recipe(e).and_then(|r| r.template) }
        })
        .collect())
}
