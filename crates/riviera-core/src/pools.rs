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

/// What a pool is for, which decides the rules that apply to it (ARC-POOL-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PoolClass {
    /// Open to the public, or to members, guests or residents (the MAHC's and ISPSC's public pools).
    #[default]
    Public,
    /// Marked for sanctioned competition.
    Competition,
    /// A dwelling's.
    Residential,
    /// Shallow, for young children.
    Wading,
}

/// A profile station: its distance along the axis and the depth below the water there.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Station {
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub at: f64,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub depth: f64,
}

/// What an entry built into the pool is (ARC-POOL-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// Steps down from the deck into the pool, projecting from its wall.
    Stairs,
    /// A ramp down from the deck into the water (ADA 1009.3).
    SlopedEntry,
    /// An underwater bench along the wall.
    Bench,
}

impl EntryKind {
    /// Its IFC class and the type name the pool rules count it by.
    pub fn counted_as(self) -> (&'static str, &'static str) {
        match self {
            EntryKind::Stairs => ("IfcStair", "Pool Stairs"),
            EntryKind::SlopedEntry => ("IfcRamp", "Sloped Entry"),
            EntryKind::Bench => ("IfcBuildingElementProxy", "Pool Bench"),
        }
    }
}

/// An entry built into the pool against one of its edges (ARC-POOL-003): it runs from the wall
/// into the pool, square to the edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Entry {
    pub kind: EntryKind,
    /// The outline edge it stands against, from 1 (the shallow end's wall).
    pub edge: usize,
    /// Its middle's distance along the edge from the edge's start.
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub at: f64,
    /// Along the edge (default 1.5 m for stairs, 1.0 m for a sloped entry, 2.0 m for a bench).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub width: Option<f64>,
    /// Stairs: each tread's depth (default 0.305 m, the MAHC's 12 in).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub tread: Option<f64>,
    /// Stairs: the highest riser allowed (default 0.25 m); the risers are made equal under it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub riser: Option<f64>,
    /// A sloped entry's slope, rise over run (default 1:12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slope: Option<f64>,
    /// How far below the water a sloped entry reaches (default 0.69 m, 27 in).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub reach: Option<f64>,
    /// A bench's seat below the water (default 0.45 m).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub seat: Option<f64>,
    /// A bench's depth out from the wall (default 0.45 m).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub depth: Option<f64>,
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
    /// What it is for; by default from its template, else public.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<PoolClass>,
    /// Stairs, sloped entries and benches built into it (ARC-POOL-003).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<Entry>,
}

impl PoolRecipe {
    /// Its class: as set, else competition or wading by its template, else public.
    pub fn class(&self) -> PoolClass {
        self.class.unwrap_or(match self.template.as_deref() {
            Some(t) if t.starts_with("competition") => PoolClass::Competition,
            Some("wading") => PoolClass::Wading,
            _ => PoolClass::Public,
        })
    }
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
    pub stairs: Vec<Mesh>,
    pub ramps: Vec<Mesh>,
    pub benches: Vec<Mesh>,
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

/// An entry as built: its numbers.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EntryInfo {
    /// From 1, in the recipe's order.
    pub index: usize,
    pub kind: EntryKind,
    pub edge: usize,
    pub width: f64,
    /// Its middle at the wall.
    pub at: [f64; 2],
    /// The water's depth at the wall there.
    pub water_depth: f64,
    /// Stairs: how many risers, each one's height, and the treads' depth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub riser: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tread: Option<f64>,
    /// A sloped entry's slope, its run from the wall, and the depth below the water it reaches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slope: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reaches: Option<f64>,
    /// A bench's seat below the water.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seat: Option<f64>,
}

/// The top of an entry's block: level, or a ramp falling from the wall.
#[derive(Clone, Copy)]
enum Top {
    Flat(f64),
    Ramp { z: f64, slope: f64, from: [f64; 2], n: [f64; 2] },
}

impl Top {
    fn at(&self, p: [f64; 2]) -> f64 {
        match *self {
            Top::Flat(z) => z,
            Top::Ramp { z, slope, from, n } => z - slope * ((p[0] - from[0]) * n[0] + (p[1] - from[1]) * n[1]).max(0.0),
        }
    }
}

/// How far from `c` on edge `from` the pool reaches along `dir`, to its first other edge.
fn across(r: &[[f64; 2]], from: usize, c: [f64; 2], dir: [f64; 2]) -> f64 {
    let n = r.len();
    (0..n)
        .filter(|i| *i != from)
        .filter_map(|i| {
            let (a, b) = (r[i], r[(i + 1) % n]);
            let e = [b[0] - a[0], b[1] - a[1]];
            let den = dir[0] * e[1] - dir[1] * e[0];
            if den.abs() < 1e-12 {
                return None;
            }
            let w = [a[0] - c[0], a[1] - c[1]];
            let t = (w[0] * e[1] - w[1] * e[0]) / den;
            let s = (w[0] * dir[1] - w[1] * dir[0]) / den;
            (t > 1e-6 && (-1e-9..=1.0 + 1e-9).contains(&s)).then_some(t)
        })
        .fold(f64::MAX, f64::min)
}

/// An entry's blocks (plan rectangles with their tops) and numbers; `None` when its edge is not
/// the pool's.
fn entry_layout(rec: &PoolRecipe, index: usize, e: &Entry, deck_z: f64) -> Option<(Vec<(Ring, Top)>, EntryInfo)> {
    let n = rec.outline.len();
    let i = e.edge.checked_sub(1).filter(|i| *i < n)?;
    let (a, b) = (rec.outline[i], rec.outline[(i + 1) % n]);
    let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
    let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
    // Inward of a counter-clockwise ring: to the left of the edge.
    let nin = [-u[1], u[0]];
    let c = [a[0] + u[0] * e.at, a[1] + u[1] * e.at];
    let w = e.width.unwrap_or(match e.kind {
        EntryKind::Stairs => 1.5,
        EntryKind::SlopedEntry => 1.0,
        EntryKind::Bench => 2.0,
    });
    let rect = |d0: f64, d1: f64| -> Ring {
        let q = |s: f64, d: f64| [c[0] + u[0] * s + nin[0] * d, c[1] + u[1] * s + nin[1] * d];
        vec![q(-w / 2.0, d0), q(w / 2.0, d0), q(w / 2.0, d1), q(-w / 2.0, d1)]
    };
    let water_depth = rec.depth_at(rec.t(c));
    let mut info = EntryInfo { index, kind: e.kind, edge: e.edge, width: w, at: c, water_depth, risers: None, riser: None, tread: None, slope: None, run: None, reaches: None, seat: None };
    let mut blocks = Vec::new();
    match e.kind {
        EntryKind::Stairs => {
            let (tread, max) = (e.tread.unwrap_or(0.305), e.riser.unwrap_or(0.25));
            let h = rec.water_level + water_depth;
            let risers = (h / max - 1e-9).ceil().max(1.0) as u32;
            let r = h / f64::from(risers);
            for k in 1..risers {
                blocks.push((rect(f64::from(k - 1) * tread, f64::from(k) * tread), Top::Flat(deck_z - f64::from(k) * r)));
            }
            (info.risers, info.riser, info.tread) = (Some(risers), Some(r), Some(tread));
        }
        EntryKind::SlopedEntry => {
            let (slope, reach) = (e.slope.unwrap_or(1.0 / 12.0), e.reach.unwrap_or(0.6858));
            // As far as the pool lets it.
            let run = ((rec.water_level + reach) / slope).min(across(&rec.outline, i, c, nin));
            blocks.push((rect(0.0, run), Top::Ramp { z: deck_z, slope, from: c, n: nin }));
            let foot = [c[0] + nin[0] * run, c[1] + nin[1] * run];
            let reaches = (slope * run - rec.water_level).min(rec.depth_at(rec.t(foot)));
            (info.slope, info.run, info.reaches) = (Some(slope), Some(run), Some(reaches));
        }
        EntryKind::Bench => {
            let seat = e.seat.unwrap_or(0.45);
            blocks.push((rect(0.0, e.depth.unwrap_or(0.45)), Top::Flat(deck_z - rec.water_level - seat)));
            info.seat = Some(seat);
        }
    }
    Some((blocks, info))
}

/// Each entry's numbers.
pub fn entries(store: &Store, rec: &PoolRecipe) -> Vec<EntryInfo> {
    let deck_z = crate::levels::elevation_of(store, &rec.level).unwrap_or(0.0) + rec.deck;
    rec.entries.iter().enumerate().filter_map(|(k, e)| entry_layout(rec, k + 1, e, deck_z).map(|x| x.1)).collect()
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
    // Entries: blocks from the floor up to their tops, inside the outline; the water they
    // displace comes off its volume.
    let (mut stairs, mut ramps, mut benches) = (Vec::new(), Vec::new(), Vec::new());
    let mut displaced = 0.0;
    for (k, en) in rec.entries.iter().enumerate() {
        let Some((blocks, _)) = entry_layout(&rec, k + 1, en, deck_z) else { continue };
        for (r, top) in blocks {
            let inside = crate::polygons::intersection(std::slice::from_ref(&outline), &[Shape { outer: r, holes: Vec::new() }]);
            for s in strips(&rec, &inside) {
                let up = |p: [f64; 2]| top.at(p).max(floor(p));
                if s.outer.iter().all(|p| up(*p) - floor(*p) < 1e-4) {
                    continue;
                }
                let m = extrude(&s, &floor, &up);
                displaced += mesh_volume(&extrude(&s, &floor, &|p| up(p).min(water_z)));
                match en.kind {
                    EntryKind::Stairs => stairs.push(m),
                    EntryKind::SlopedEntry => ramps.push(m),
                    EntryKind::Bench => benches.push(m),
                }
            }
        }
    }
    let area = crate::polygons::area(&rec.outline).abs();
    let n = rec.outline.len();
    let perimeter = (0..n).map(|i| {
        let (a, b) = (rec.outline[i], rec.outline[(i + 1) % n]);
        (b[0] - a[0]).hypot(b[1] - a[1])
    }).sum();
    let volume = water.iter().map(mesh_volume).sum::<f64>() - displaced;
    let shell_volume = base.iter().chain(&walls).map(mesh_volume).sum();
    Ok(PoolGeometry { water, base, walls, coping, stairs, ramps, benches, deck_z, water_z, area, perimeter, volume, shell_volume })
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
    /// What it is for (default from the template, else public).
    pub class: Option<PoolClass>,
    /// Stairs, sloped entries and benches (ARC-POOL-003).
    #[serde(default)]
    pub entries: Vec<Entry>,
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

fn validate(rec: &PoolRecipe) -> Result<()> {
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
    let n = rec.outline.len();
    for (k, e) in rec.entries.iter().enumerate() {
        let Some(i) = e.edge.checked_sub(1).filter(|i| *i < n) else {
            return Err(bad("pool.bad_entry", format!("entry {}: the pool has edges 1 to {n}", k + 1)));
        };
        let (a, b) = (rec.outline[i], rec.outline[(i + 1) % n]);
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        let positive = [e.width, e.tread, e.riser, e.slope, e.reach, e.seat, e.depth].iter().flatten().all(|v| *v > 0.0);
        if !(0.0..=l).contains(&e.at) || !positive {
            return Err(bad("pool.bad_entry", format!("entry {}: its middle must be on edge {} (0 to {l:.2} m) and its sizes above zero", k + 1, e.edge)));
        }
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
        class: p.class,
        entries: p.entries,
    };
    validate(&rec)?;
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
    pub class: Option<PoolClass>,
    /// Replaces its entries.
    pub entries: Option<Vec<Entry>>,
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
    if let Some(v) = p.class {
        rec.class = Some(v);
    }
    if let Some(v) = p.entries {
        rec.entries = v;
    }
    validate(&rec)?;
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
    pub entries: Vec<EntryInfo>,
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
        entries: entries(store, &rec),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddEntryParams {
    pub pool_id: ElementRef,
    pub kind: EntryKind,
    /// A plan point by the wall: the entry goes on the nearest edge, its middle there. Or give
    /// `edge` and `at`.
    pub point: Option<[f64; 2]>,
    pub edge: Option<usize>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub at: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub width: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub tread: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub riser: Option<f64>,
    pub slope: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub reach: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub seat: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub depth: Option<f64>,
}

/// The nearest edge to a plan point (from 1) and the distance along it.
fn nearest_edge(r: &[[f64; 2]], p: [f64; 2]) -> (usize, f64) {
    let n = r.len();
    (0..n)
        .map(|i| {
            let (a, b) = (r[i], r[(i + 1) % n]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = dx.hypot(dy).max(1e-12);
            let at = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l).clamp(0.0, l);
            let d = (p[0] - a[0] - at * dx / l).hypot(p[1] - a[1] - at * dy / l);
            (i + 1, at, d)
        })
        .min_by(|x, y| x.2.total_cmp(&y.2))
        .map(|(i, at, _)| (i, at))
        .unwrap_or((1, 0.0))
}

/// Builds an entry into a pool (`pool.add_entry`); its numbers back.
pub(crate) fn add_entry(txn: &mut Transaction<'_>, p: AddEntryParams) -> Result<EntryInfo> {
    let (id, mut rec) = resolve(txn.store(), &p.pool_id)?;
    let (edge, at) = match (p.point, p.edge, p.at) {
        (Some(q), None, None) => nearest_edge(&rec.outline, q),
        (None, Some(e), Some(a)) => (e, a),
        _ => return Err(bad("pool.bad_entry", "give a point by the wall, or an edge and a distance along it")),
    };
    rec.entries.push(Entry { kind: p.kind, edge, at, width: p.width, tread: p.tread, riser: p.riser, slope: p.slope, reach: p.reach, seat: p.seat, depth: p.depth });
    validate(&rec)?;
    let v = serde_json::to_value(&rec).expect("serialises");
    txn.modify(id, |e| {
        e.params.insert(RECIPE_KEY.into(), v);
    })?;
    entries(txn.store(), &rec).pop().ok_or_else(|| bad("pool.bad_entry", "the entry was not made"))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveEntryParams {
    pub pool_id: ElementRef,
    /// From 1, as `pool.info` lists them.
    pub index: usize,
}

/// Takes an entry out of a pool (`pool.remove_entry`).
pub(crate) fn remove_entry(txn: &mut Transaction<'_>, p: RemoveEntryParams) -> Result<()> {
    let (id, mut rec) = resolve(txn.store(), &p.pool_id)?;
    if p.index == 0 || p.index > rec.entries.len() {
        return Err(bad("pool.bad_entry", format!("the pool has entries 1 to {}", rec.entries.len())));
    }
    rec.entries.remove(p.index - 1);
    let v = serde_json::to_value(&rec).expect("serialises");
    txn.modify(id, |e| {
        e.params.insert(RECIPE_KEY.into(), v);
    })?;
    Ok(())
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

// ---- Checks (ARC-POOL-002) ----

/// How far a plan point is from a ring's edge.
fn edge_distance(p: [f64; 2], r: &[[f64; 2]]) -> f64 {
    (0..r.len())
        .map(|i| {
            let (a, b) = (r[i], r[(i + 1) % r.len()]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l2 = (dx * dx + dy * dy).max(1e-12);
            let f = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0);
            (p[0] - a[0] - f * dx).hypot(p[1] - a[1] - f * dy)
        })
        .fold(f64::MAX, f64::min)
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckParams {
    /// One pool; every pool when omitted.
    pub pool_id: Option<ElementRef>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PoolFinding {
    pub pack: String,
    pub rule: String,
    pub source: String,
    /// `error` (a minimum or accessible pack's verified or secondary rule) or `note`.
    pub severity: String,
    /// Where the value came from: `verified`, `secondary` (a summary or checklist: check the
    /// code's own text) or `advisory`.
    pub confidence: crate::rules::Confidence,
    pub pool: ElementId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<ElementId>,
    pub message: String,
}

/// A pack's bather or occupant load for a pool.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Load {
    pub pack: String,
    pub rule: String,
    /// Water area per bather (m²).
    pub per: f64,
    pub bathers: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PoolReport {
    pub pool: ElementId,
    pub name: String,
    pub class: PoolClass,
    pub area: f64,
    pub perimeter: f64,
    pub shallowest: f64,
    pub deepest: f64,
    pub entries: u32,
    pub accessible_entries: u32,
    pub drains: u32,
    pub loads: Vec<Load>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PoolCheck {
    /// The pool packs in use.
    pub packs: Vec<String>,
    pub pools: Vec<PoolReport>,
    pub findings: Vec<PoolFinding>,
}

type Cuts = Vec<(ElementId, [[f64; 2]; 2])>;

/// A placed element's plan point and plan size (its two extents, smaller first).
fn placed(store: &Store, e: &Element) -> Option<([f64; 2], [f64; 2])> {
    let it = crate::rules::item(store, e)?;
    let (a, b) = (it.x[1] - it.x[0], it.back - it.front);
    let o = it.frame.apply([(it.x[0] + it.x[1]) / 2.0, (it.front + it.back) / 2.0, 0.0]);
    Some(([o[0], o[1]], [a.min(b), a.max(b)]))
}

impl PoolClass {
    pub fn name(self) -> &'static str {
        match self {
            PoolClass::Public => "public",
            PoolClass::Competition => "competition",
            PoolClass::Residential => "residential",
            PoolClass::Wading => "wading",
        }
    }
}

/// "a–b", "at least a" or "at most b", each in `unit`.
fn bounds(min: Option<f64>, max: Option<f64>, unit: &dyn Fn(f64) -> String) -> String {
    match (min, max) {
        (Some(a), Some(b)) => {
            // One unit for both: "12–18 in".
            let (x, y) = (unit(a), unit(b));
            match x.rsplit_once(' ') {
                Some((n, u)) if y.ends_with(&format!(" {u}")) => format!("{n}–{y}"),
                _ => format!("{x}–{y}"),
            }
        }
        (Some(a), None) => format!("at least {}", unit(a)),
        (None, Some(b)) => format!("at most {}", unit(b)),
        _ => String::new(),
    }
}

fn outside(v: f64, min: Option<f64>, max: Option<f64>) -> bool {
    min.is_some_and(|m| v < m - 1e-6) || max.is_some_and(|m| v > m + 1e-6)
}

fn inches(m: f64) -> String {
    let i = m / 0.0254;
    if (i - i.round()).abs() < 0.05 { format!("{:.0} in", i) } else { format!("{:.1} in", i) }
}

fn feet(m: f64) -> String {
    let inches = (m / 0.0254).round() as i64;
    if inches % 12 == 0 { format!("{} ft", inches / 12) } else { format!("{}′ {}″", inches / 12, inches % 12) }
}

/// Checks pools against the pool packs in use (`pool.check`).
pub(crate) fn check(store: &Store, p: Option<CheckParams>) -> Result<PoolCheck> {
    use crate::rules::{Confidence, Layer, PoolMeasure as M};
    let only = match p.and_then(|p| p.pool_id) {
        Some(r) => Some(resolve(store, &r)?.0),
        None => None,
    };
    let packs = crate::rules::active(store, "pool");
    let mut out = PoolCheck { packs: packs.iter().map(|p| p.id.clone()).collect(), pools: Vec::new(), findings: Vec::new() };
    // Elements the rules count stand on the deck by design: they do not block it.
    let counted: Vec<&crate::rules::Selector> = packs.iter().flat_map(|p| p.pool.iter()).filter_map(|r| r.counts.as_ref()).collect();
    let is_counted = |e: &Element| counted.iter().any(|s| s.selects_element(store, e));
    // The deck's slice, at its height.
    let mut cuts_at: Option<(f64, Cuts)> = None;
    for (id, g) in all(store) {
        if only.is_some_and(|o| o != id) {
            continue;
        }
        let e = store.get(id).expect("listed");
        let rec = recipe(e).expect("a pool");
        let class = rec.class();
        let ring = &rec.outline;
        let ts: Vec<f64> = ring.iter().map(|q| rec.t(*q)).collect();
        let (t0, t1) = ts.iter().fold((f64::MAX, f64::MIN), |(a, b), t| (a.min(*t), b.max(*t)));
        let mut at: Vec<f64> = rec.profile.iter().map(|s| s.at).filter(|t| (t0..=t1).contains(t)).collect();
        at.extend([t0, t1]);
        let depths: Vec<f64> = at.iter().map(|t| rec.depth_at(*t)).collect();
        let shallowest = depths.iter().copied().fold(f64::MAX, f64::min);
        let deepest = depths.iter().copied().fold(f64::MIN, f64::max);
        // Segments inside the pool: (from, to, depth from, depth to, slope).
        let segments: Vec<(f64, f64, f64, f64, f64)> = rec
            .profile
            .windows(2)
            .filter_map(|w| {
                let (a, b) = (w[0].at.max(t0), w[1].at.min(t1));
                (b > a + 1e-9).then(|| (a, b, rec.depth_at(a), rec.depth_at(b), (w[1].depth - w[0].depth).abs() / (w[1].at - w[0].at)))
            })
            .collect();
        let near_items: Vec<(ElementId, [f64; 2], [f64; 2])> = store.iter().filter(|o| o.id() != id).filter_map(|o| placed(store, o).map(|(c, s)| (o.id(), c, s))).collect();
        let selected = |sel: &crate::rules::Selector, f: &dyn Fn([f64; 2]) -> bool| -> Vec<(ElementId, [f64; 2], [f64; 2])> {
            near_items.iter().filter(|(o, c, _)| f(*c) && store.get(*o).is_some_and(|el| sel.selects_element(store, el))).copied().collect()
        };
        let built = entries(store, &rec);
        // The pool's own entries a selector counts, and of those the ones `primary` selects.
        let own = |sel: &crate::rules::Selector| built.iter().filter(|b| {
            let (class, name) = b.kind.counted_as();
            sel.selects(class, None, Some(name))
        }).count();
        let mut report = PoolReport { pool: id, name: e.name.clone().unwrap_or_default(), class, area: g.area, perimeter: g.perimeter, shallowest, deepest, entries: 0, accessible_entries: 0, drains: 0, loads: Vec::new() };
        for pack in &packs {
            for r in pack.pool.iter().filter(|r| r.pools.is_empty() || r.pools.contains(&class)) {
                let severity = if pack.layer == Layer::Recommended || r.confidence == Confidence::Advisory { "note" } else { "error" };
                let mut find = |element: Option<ElementId>, note: bool, message: String| {
                    let severity = if note { "note" } else { severity };
                    out.findings.push(PoolFinding { pack: pack.id.clone(), rule: r.id.clone(), source: r.source.clone(), severity: severity.into(), confidence: r.confidence, pool: id, element, message });
                };
                let ratio = |s: f64| if s < 1e-9 { "level".to_owned() } else { format!("1:{:.1}", 1.0 / s) };
                match r.check {
                    M::ShallowSlope | M::DeepSlope => {
                        let Some(max) = r.max else { continue };
                        let shallow = r.check == M::ShallowSlope;
                        for &(a, b, da, db, s) in &segments {
                            let part = if shallow { da.min(db) < BREAK - 1e-6 } else { da.max(db) > BREAK + 1e-6 };
                            if part && s > max + 1e-6 {
                                let side = if shallow { "shallower" } else { "deeper" };
                                find(None, false, format!("the floor from {a:.2} m to {b:.2} m falls {} where the water is {side} than 5 ft; {} allows {}", ratio(s), pack.id, ratio(max)));
                            }
                        }
                    }
                    M::ShallowEndDepth => {
                        if r.min.is_some_and(|m| shallowest < m - 1e-6) || r.max.is_some_and(|m| shallowest > m + 1e-6) {
                            let range = match (r.min, r.max) {
                                (Some(a), Some(b)) => format!("{}–{}", feet(a), feet(b)),
                                (Some(a), None) => format!("at least {}", feet(a)),
                                (None, Some(b)) => format!("at most {}", feet(b)),
                                _ => continue,
                            };
                            find(None, false, format!("the shallow end is {shallowest:.2} m ({}) deep; it should be {range}", feet(shallowest)));
                        }
                    }
                    M::Depth => {
                        if let Some(max) = r.max
                            && deepest > max + 1e-6
                        {
                            find(None, false, format!("the pool is {deepest:.2} m ({}) deep at its deepest; at most {} for a {} pool", feet(deepest), feet(max), class.name()));
                        }
                    }
                    M::CoveRadius => {
                        if r.depth_up_to.is_some_and(|d| shallowest > d + 1e-6) {
                            continue;
                        }
                        if let Some(max) = r.max
                            && rec.cove > max + 1e-6
                        {
                            let where_ = r.depth_up_to.map(|d| format!(" where the water is {} deep or less", feet(d))).unwrap_or_default();
                            find(None, false, format!("the wall–floor cove's radius is {:.0} mm; at most {:.0} mm{where_}", rec.cove * 1000.0, max * 1000.0));
                        }
                    }
                    M::CopingOverhang | M::CopingThickness => {
                        if !rec.edges.iter().any(Edge::has_coping) {
                            continue;
                        }
                        let (what, v) = if r.check == M::CopingOverhang { ("overhang", rec.coping.overhang) } else { ("thickness", rec.coping.thickness) };
                        if r.min.is_some_and(|m| v < m - 1e-6) || r.max.is_some_and(|m| v > m + 1e-6) {
                            let range = match (r.min, r.max) {
                                (Some(a), Some(b)) => format!("{:.0}–{:.0} mm", a * 1000.0, b * 1000.0),
                                (Some(a), None) => format!("at least {:.0} mm", a * 1000.0),
                                (None, Some(b)) => format!("at most {:.0} mm", b * 1000.0),
                                _ => continue,
                            };
                            find(None, false, format!("the coping's {what} is {:.0} mm; it should be {range}", v * 1000.0));
                        }
                    }
                    M::DeckWidth => {
                        let Some(w) = r.min else { continue };
                        let h = g.deck_z + 0.3;
                        if cuts_at.as_ref().is_none_or(|(z, _)| (z - h).abs() > 1e-6) {
                            cuts_at = Some((h, crate::sco::slice(store, h)));
                        }
                        let cuts: Vec<_> = cuts_at.as_ref().expect("sliced").1.iter().filter(|(o, _)| *o != id && !store.get(*o).is_some_and(is_counted)).copied().collect();
                        let floors: Vec<Vec<[f64; 2]>> = crate::floors::all(store)
                            .into_iter()
                            .filter(|(f, _)| store.get(*f).and_then(crate::floors::slab_recipe).is_some_and(|(_, fr)| fr.level == rec.level))
                            .flat_map(|(_, f)| f.profiles.into_iter().map(|p| p.outer))
                            .collect();
                        for i in 0..ring.len() {
                            if matches!(rec.edges[i], Edge::Infinity { .. }) {
                                continue;
                            }
                            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                            let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
                            // Outward of a counter-clockwise ring: to the right of the edge.
                            let n = [(b[1] - a[1]) / l, -(b[0] - a[0]) / l];
                            let off = |q: [f64; 2], d: f64| [q[0] + n[0] * d, q[1] + n[1] * d];
                            let strip = [off(a, 0.01), off(b, 0.01), off(b, w), off(a, w)];
                            let probes = [0.1, 0.5, 0.9].map(|f| off([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f], w - 0.05));
                            if floors.is_empty() || probes.iter().any(|q| !floors.iter().any(|f| crate::sco::inside(*q, f))) {
                                find(None, false, format!("edge {} has no deck {} ({w:.2} m) wide along it", i + 1, feet(w)));
                            } else if let Some(by) = crate::sco::blocking(&cuts, &strip) {
                                find(Some(by), false, format!("{} stands in the {} ({w:.2} m) deck along edge {}", crate::sco::name(store, by), feet(w), i + 1));
                            }
                        }
                    }
                    M::InfinityEdge => {
                        let Some(max) = r.max else { continue };
                        let vanishing: f64 = (0..ring.len())
                            .filter(|i| matches!(rec.edges[*i], Edge::Infinity { .. }))
                            .map(|i| {
                                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                                (b[0] - a[0]).hypot(b[1] - a[1])
                            })
                            .sum();
                        let share = vanishing / g.perimeter.max(1e-9);
                        if share > max + 1e-6 {
                            find(None, false, format!("{:.0} % of the edge is vanishing; at most {:.0} %", share * 100.0, max * 100.0));
                        }
                    }
                    M::Entries => {
                        let Some(sel) = &r.counts else { continue };
                        let reach = r.near.unwrap_or(1.0);
                        let n = (selected(sel, &|c| crate::sco::inside(c, ring) || edge_distance(c, ring) <= reach).len() + own(sel)) as u32;
                        report.entries = report.entries.max(n);
                        if let Some(min) = r.min
                            && f64::from(n) < min - 1e-9
                        {
                            find(None, false, format!("{n} entries and exits (ladders, stairs, recessed steps) at the pool; at least {min:.0}"));
                        }
                    }
                    M::AccessibleEntries => {
                        let Some(sel) = &r.counts else { continue };
                        let reach = r.near.unwrap_or(1.5);
                        let found = selected(sel, &|c| crate::sco::inside(c, ring) || edge_distance(c, ring) <= reach);
                        let primary = r.primary.as_ref().map_or(found.len() + own(sel), |ps| found.iter().filter(|(o, ..)| store.get(*o).is_some_and(|el| ps.selects_element(store, el))).count() + own(ps));
                        let n = (found.len() + own(sel)) as u32;
                        report.accessible_entries = report.accessible_entries.max(n);
                        let need = match r.perimeter {
                            Some(p) if g.perimeter < p - 1e-6 => 1.0,
                            _ => r.min.unwrap_or(1.0),
                        };
                        let wall = format!("{:.0} ft of pool wall", g.perimeter / 0.3048);
                        if f64::from(n) < need - 1e-9 {
                            find(None, false, format!("{n} accessible means of entry; {wall} needs {need:.0}"));
                        } else if primary == 0 {
                            find(None, false, format!("{n} accessible means of entry, none of them a pool lift or sloped entry; at least one must be"));
                        }
                    }
                    M::Drains => {
                        let Some(sel) = &r.counts else { continue };
                        let found = selected(sel, &|c| crate::sco::inside(c, ring));
                        report.drains = report.drains.max(found.len() as u32);
                        if found.is_empty() {
                            find(None, true, "no main drains (suction outlets) are modelled; this rule applies to the ones the pool will have".into());
                            continue;
                        }
                        let unblockable = |s: [f64; 2]| r.cover.is_some_and(|c| s[0] >= c[0].min(c[1]) - 1e-6 && s[1] >= c[0].max(c[1]) - 1e-6);
                        if found.iter().any(|(.., s)| unblockable(*s)) {
                            continue;
                        }
                        if let Some(min) = r.min
                            && (found.len() as f64) < min - 1e-9
                        {
                            let cover = r.cover.map(|c| format!(", or a cover at least {:.0} × {:.0} in", c[0] / 0.0254, c[1] / 0.0254)).unwrap_or_default();
                            find(Some(found[0].0), false, format!("{} blockable main drain; at least {min:.0}{cover}", found.len()));
                        }
                        if let Some(sp) = r.spacing {
                            for (i, a) in found.iter().enumerate() {
                                for b in &found[i + 1..] {
                                    let d = (a.1[0] - b.1[0]).hypot(a.1[1] - b.1[1]);
                                    if d < sp - 1e-6 {
                                        find(Some(b.0), false, format!("main drains {d:.2} m apart; at least {} ({sp:.2} m)", feet(sp)));
                                    }
                                }
                            }
                        }
                    }
                    M::StairTread | M::StairRiser | M::StairWidth | M::SlopedEntrySlope | M::SlopedEntryDepth | M::BenchSeat => {
                        let kind = match r.check {
                            M::SlopedEntrySlope | M::SlopedEntryDepth => EntryKind::SlopedEntry,
                            M::BenchSeat => EntryKind::Bench,
                            _ => EntryKind::Stairs,
                        };
                        for b in built.iter().filter(|b| b.kind == kind) {
                            let what = format!("{} {} (edge {})", match kind { EntryKind::Stairs => "stairs", EntryKind::SlopedEntry => "sloped entry", EntryKind::Bench => "bench" }, b.index, b.edge);
                            match r.check {
                                M::StairTread | M::StairRiser | M::StairWidth => {
                                    let (name, v) = match r.check {
                                        M::StairTread => ("its treads are", b.tread.unwrap_or(0.0)),
                                        M::StairRiser => ("its risers are", b.riser.unwrap_or(0.0)),
                                        _ => ("it is", b.width),
                                    };
                                    let wide = if r.check == M::StairWidth { " wide" } else { "" };
                                    if outside(v, r.min, r.max) {
                                        find(None, false, format!("{what}: {name} {} ({:.0} mm){wide}; {}", inches(v), v * 1000.0, bounds(r.min, r.max, &inches)));
                                    }
                                }
                                M::SlopedEntrySlope => {
                                    let v = b.slope.unwrap_or(0.0);
                                    if r.max.is_some_and(|m| v > m + 1e-6) {
                                        find(None, false, format!("{what} falls {}; at most {}", ratio(v), ratio(r.max.unwrap_or(0.0))));
                                    }
                                }
                                M::SlopedEntryDepth => {
                                    let v = b.reaches.unwrap_or(0.0);
                                    if outside(v, r.min, r.max) {
                                        find(None, false, format!("{what} reaches {} below the water; {}", inches(v), bounds(r.min, r.max, &inches)));
                                    }
                                }
                                _ => {
                                    let v = b.seat.unwrap_or(0.0);
                                    if outside(v, r.min, r.max) {
                                        find(None, false, format!("{what}: its seat is {} below the water; {}", inches(v), bounds(r.min, r.max, &inches)));
                                    }
                                    if let Some(m) = r.water_depth_max
                                        && b.water_depth > m + 1e-6
                                    {
                                        find(None, false, format!("{what} stands in {} of water; at most {}", feet(b.water_depth), feet(m)));
                                    }
                                }
                            }
                        }
                    }
                    M::BatherLoad => {
                        let Some(per) = r.min.filter(|v| *v > 0.0) else { continue };
                        report.loads.push(Load { pack: pack.id.clone(), rule: r.id.clone(), per, bathers: (g.area / per + 1e-9).floor() as u32 });
                    }
                }
            }
        }
        out.pools.push(report);
    }
    out.findings.sort_by_key(|f| f.severity != "error");
    Ok(out)
}
