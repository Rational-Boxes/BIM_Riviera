// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A pool's depth markings, NO DIVING markers and break-point line (ARC-POOL-011,
//! `POOLS_PLAN.md` §5, research/23 A.5, B.10).
//!
//! - **Where:** each edge's least and greatest depth (so both sides and each end), every slope
//!   change, the 5 ft break, and every `increment` of depth change up to 5 ft (the MAHC's 1 ft,
//!   the ISPSC's 2 ft); then more, evenly, wherever two are more than `spacing` apart round the
//!   perimeter (25 ft). Markers closer than 0.6 m merge.
//! - **What:** the water's depth 3 ft out from the wall (MAHC 4.5.19.1.3), to the nearest 3 in
//!   in feet and inches ("4 FT 6 IN": the MAHC forbids the ' and " symbols) or to 0.1 m in
//!   metres, or both. NO DIVING beside each deck marker where the water is 5 ft deep or less.
//! - **Drawn:** a deck tile 0.6 × 0.2 m centred 0.25 m out from the water's edge (within the
//!   18 in), a NO DIVING tile 0.3 m square beside it, and a wall tile 0.6 × 0.115 m under the
//!   coping (the top 4.5 in of the wall), each 3–5 mm proud. In pools deeper than 5 ft, the
//!   break-point band 4 in wide on the floor at the shallow side of the break and up the walls
//!   to the water; the float line is placed 1 ft shallower and reported.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::bodies::Mesh;
use crate::error::Result;
use crate::id::ElementRef;
use crate::pools::{self, BREAK, PoolRecipe};
use crate::store::Store;
use crate::txn::Transaction;

const FT: f64 = 0.3048;
/// How far out from the wall the depth is measured (3 ft).
pub const MEASURED: f64 = 3.0 * FT;
/// The break-point band's width (4 in: the ISPSC's least, inside the MAHC's 2–6 in).
pub const BAND: f64 = 4.0 * 0.0254;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Units {
    #[default]
    Feet,
    Metres,
    Both,
}

impl Units {
    pub fn name(self) -> &'static str {
        match self {
            Units::Feet => "feet",
            Units::Metres => "metres",
            Units::Both => "both",
        }
    }
}

/// A pool's depth markings, as set.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DepthMarkings {
    pub units: Units,
    /// Markers at every this much depth change up to 5 ft (default 1 ft).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub increment: f64,
    /// The most apart round the perimeter (default 25 ft).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub spacing: f64,
    /// NO DIVING markers where the water is 5 ft deep or less (default true).
    pub no_diving: bool,
    /// The break-point band and float line in pools deeper than 5 ft (default true).
    pub break_line: bool,
}

/// A depth marker.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Marker {
    /// The water's edge it marks, and its deck tile's centre.
    pub edge_point: [f64; 2],
    pub at: [f64; 2],
    /// Its outline edge (from 1) and its distance round the perimeter.
    pub edge: usize,
    pub along: f64,
    /// The depth 3 ft out from the wall.
    pub depth: f64,
    pub label: String,
    pub no_diving: bool,
    /// Why it is there: `least`, `greatest`, `slope_change`, `break`, `increment` or `spacing`.
    pub reason: String,
}

/// Where the break-point band and the float line are.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Lines {
    /// The band's deep edge and shallow edge, along the axis.
    pub band: [f64; 2],
    /// The float line, along the axis.
    pub float_line: f64,
}

/// The label for a depth.
pub fn label(depth: f64, units: Units) -> String {
    let quarter = (depth / 0.0254 / 3.0).round() as i64 * 3;
    let (ft, inch) = (quarter / 12, quarter % 12);
    let feet = if inch == 0 { format!("{ft} FT") } else { format!("{ft} FT {inch} IN") };
    let metres = format!("{:.1} M", (depth * 10.0).round() / 10.0);
    match units {
        Units::Feet => feet,
        Units::Metres => metres,
        Units::Both => format!("{feet} / {metres}"),
    }
}

fn inward(a: [f64; 2], b: [f64; 2]) -> ([f64; 2], f64) {
    let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
    ([-(b[1] - a[1]) / l, (b[0] - a[0]) / l], l)
}

/// The break-point band, in pools deeper than 5 ft.
pub fn lines(rec: &PoolRecipe) -> Option<Lines> {
    if !rec.depth_markings.is_some_and(|m| m.break_line) {
        return None;
    }
    let ts: Vec<f64> = rec.outline.iter().map(|q| rec.t(*q)).collect();
    let (t0, t1) = ts.iter().fold((f64::MAX, f64::MIN), |(a, b), t| (a.min(*t), b.max(*t)));
    if rec.depth_at(t0) >= BREAK || rec.depth_at(t1) <= BREAK + 1e-6 {
        return None;
    }
    // The first place along the axis the floor reaches 5 ft.
    let (mut lo, mut hi) = (t0, t1);
    for _ in 0..60 {
        let m = (lo + hi) / 2.0;
        if rec.depth_at(m) >= BREAK { hi = m } else { lo = m }
    }
    Some(Lines { band: [hi, hi - BAND], float_line: hi - BAND - FT })
}

/// The plan outlines of the placed objects by a pool's edge, which its markers keep clear of.
fn footprints(store: &Store, rec: &PoolRecipe) -> Vec<Vec<[f64; 2]>> {
    store
        .iter()
        // Cheaply first: by the insertion point, then by the shape.
        .filter(|o| crate::mesh_objects::mesh_object_recipe(o).is_some_and(|m| m.level == rec.level && (crate::sco::inside(m.point, &rec.outline) || pools::edge_distance(m.point, &rec.outline) < 4.0)))
        .filter_map(|o| crate::rules::item(store, o))
        .map(|it| [[it.x[0], it.front], [it.x[1], it.front], [it.x[1], it.back], [it.x[0], it.back]].iter().map(|q| {
            let p = it.frame.apply([q[0], q[1], 0.0]);
            [p[0], p[1]]
        }).collect::<Vec<_>>())
        .filter(|r: &Vec<[f64; 2]>| r.iter().any(|q| pools::edge_distance(*q, &rec.outline) < 1.5) || r.iter().any(|q| crate::sco::inside(*q, &rec.outline)))
        .collect()
}

/// Whether a marker at `p` on an edge running `u` (inward `nin`) is clear of the outlines: its
/// wall tile, deck tile and NO DIVING tile.
fn clear(p: [f64; 2], u: [f64; 2], nin: [f64; 2], blocks: &[Vec<[f64; 2]>]) -> bool {
    let at = |a: f64, b: f64| [p[0] + u[0] * a - nin[0] * b, p[1] + u[1] * a - nin[1] * b];
    let mut zone = vec![at(-0.35, -0.1), at(0.75, -0.1), at(0.75, 0.45), at(-0.35, 0.45)];
    if crate::polygons::area(&zone) < 0.0 {
        zone.reverse();
    }
    let bbox = |r: &[[f64; 2]]| r.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])]);
    let zb = bbox(&zone);
    let z = [crate::polygons::Shape { outer: zone, holes: Vec::new() }];
    blocks.iter().all(|b| {
        let bb = bbox(b);
        if bb[0] >= zb[2] || bb[2] <= zb[0] || bb[1] >= zb[3] || bb[3] <= zb[1] {
            return true;
        }
        let mut b = b.clone();
        if crate::polygons::area(&b) < 0.0 {
            b.reverse();
        }
        crate::polygons::intersection(&z, &[crate::polygons::Shape { outer: b, holes: Vec::new() }]).iter().map(|s| crate::polygons::area(&s.outer).abs()).sum::<f64>() < 1e-6
    })
}

/// The markers a pool's depth markings put round it, each moved along its edge clear of the
/// objects standing there.
pub fn markers(store: &Store, rec: &PoolRecipe) -> Vec<Marker> {
    let Some(m) = rec.depth_markings else { return Vec::new() };
    let r = &rec.outline;
    let n = r.len();
    let depth = |p: [f64; 2], nin: [f64; 2]| rec.depth_at(rec.t([p[0] + nin[0] * MEASURED, p[1] + nin[1] * MEASURED])).max(0.0);
    // Samples round the perimeter, kept 0.4 m from the corners.
    struct S {
        along: f64,
        edge: usize,
        p: [f64; 2],
        nin: [f64; 2],
        d: f64,
    }
    let mut samples: Vec<Vec<S>> = Vec::new();
    let mut start = 0.0;
    for i in 0..n {
        let (a, b) = (r[i], r[(i + 1) % n]);
        let (nin, l) = inward(a, b);
        let k = ((l - 0.8) / 0.05).floor().max(0.0) as usize;
        let mut v = Vec::new();
        for j in 0..=k {
            let f = if l > 0.8 { (0.4 + j as f64 * 0.05).min(l - 0.4) } else { l / 2.0 };
            let p = [a[0] + (b[0] - a[0]) * f / l, a[1] + (b[1] - a[1]) * f / l];
            v.push(S { along: start + f, edge: i, p, nin, d: depth(p, nin) });
        }
        samples.push(v);
        start += l;
    }
    let perimeter = start;
    // Required places, most important first.
    let mut want: Vec<(&S, &'static str)> = Vec::new();
    for v in &samples {
        let Some(lo) = v.iter().min_by(|a, b| a.d.total_cmp(&b.d)) else { continue };
        let hi = v.iter().max_by(|a, b| a.d.total_cmp(&b.d)).expect("not empty");
        if hi.d - lo.d < 0.01 {
            want.push((&v[v.len() / 2], "least"));
        } else {
            want.push((lo, "least"));
            want.push((hi, "greatest"));
        }
    }
    let crossing = |level: f64| -> Vec<&S> {
        samples.iter().flat_map(|v| v.windows(2).filter(move |w| (w[0].d - level) * (w[1].d - level) < 0.0 || (w[1].d - level).abs() < 1e-9).map(|w| &w[1])).collect()
    };
    for st in &rec.profile {
        // Where the floor's slope changes, measured as the marker would be.
        for s in samples.iter().flat_map(|v| v.windows(2).filter(|w| (rec.t([w[0].p[0] + w[0].nin[0] * MEASURED, w[0].p[1] + w[0].nin[1] * MEASURED]) - st.at) * (rec.t([w[1].p[0] + w[1].nin[0] * MEASURED, w[1].p[1] + w[1].nin[1] * MEASURED]) - st.at) < 0.0).map(|w| &w[1])) {
            want.push((s, "slope_change"));
        }
    }
    for s in crossing(BREAK) {
        want.push((s, "break"));
    }
    let mut level = m.increment;
    while m.increment > 1e-3 && level < BREAK - 1e-6 {
        for s in crossing(level) {
            want.push((s, "increment"));
        }
        level += m.increment;
    }
    // Each in a clear spot: the nearest sample on its edge, within 3 m, clear of what stands there.
    let blocks = footprints(store, rec);
    let free = |s: &S| {
        let (a, b) = (r[s.edge], r[(s.edge + 1) % n]);
        let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
        blocks.is_empty() || clear(s.p, [(b[0] - a[0]) / l, (b[1] - a[1]) / l], s.nin, &blocks)
    };
    let settle = |s: &S| -> (f64, usize, [f64; 2], [f64; 2], f64) {
        let mut near: Vec<&S> = samples[s.edge].iter().filter(|x| (x.along - s.along).abs() <= 3.0).collect();
        near.sort_by(|x, y| (x.along - s.along).abs().total_cmp(&(y.along - s.along).abs()));
        let x = if free(s) { s } else { near.into_iter().find(|x| free(x)).unwrap_or(s) };
        (x.along, x.edge, x.p, x.nin, x.d)
    };
    let mut kept: Vec<(f64, usize, [f64; 2], [f64; 2], f64, &'static str)> = Vec::new();
    for (s, why) in want {
        let (along, edge, p, nin, d) = settle(s);
        let s = S { along, edge, p, nin, d };
        let gap = |a: f64| (a - s.along).abs().min(perimeter - (a - s.along).abs());
        if kept.iter().all(|k| gap(k.0) >= 0.6) {
            kept.push((s.along, s.edge, s.p, s.nin, s.d, why));
        }
    }
    kept.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Fill the long gaps, round the ring.
    let all: Vec<&S> = samples.iter().flatten().collect();
    // Again while a marker moved clear of an object leaves a gap too long.
    for _ in 0..4 {
        if m.spacing <= 0.5 || kept.is_empty() {
            break;
        }
        let mut extra = Vec::new();
        for k in 0..kept.len() {
            let a = kept[k].0;
            let b = if k + 1 < kept.len() { kept[k + 1].0 } else { kept[0].0 + perimeter };
            let parts = ((b - a) / m.spacing - 1e-9).ceil() as usize;
            for j in 1..parts {
                let want_at = (a + (b - a) * j as f64 / parts as f64) % perimeter;
                let cyc = |x: f64| (x - want_at).abs().min(perimeter - (x - want_at).abs());
                let mut near: Vec<&&S> = all.iter().filter(|x| cyc(x.along) <= 3.0).collect();
                near.sort_by(|x, y| cyc(x.along).total_cmp(&cyc(y.along)));
                if let Some(s) = near.iter().find(|x| free(x)).or(near.first()) {
                    extra.push((s.along, s.edge, s.p, s.nin, s.d, "spacing"));
                }
            }
        }
        if extra.is_empty() {
            break;
        }
        kept.extend(extra);
        kept.sort_by(|a, b| a.0.total_cmp(&b.0));
        kept.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9);
    }
    kept.into_iter()
        .map(|(along, edge, p, nin, d, why)| Marker {
            edge_point: p,
            at: [p[0] - nin[0] * 0.25, p[1] - nin[1] * 0.25],
            edge: edge + 1,
            along,
            depth: d,
            label: label(d, m.units),
            no_diving: m.no_diving && d <= BREAK + 1e-6,
            reason: why.into(),
        })
        .collect()
}

/// A box from plan corners and two heights.
fn tile(c: [f64; 2], u: [f64; 2], v: [f64; 2], half: [f64; 2], z: [f64; 2]) -> Mesh {
    let at = |a: f64, b: f64| [c[0] + u[0] * a + v[0] * b, c[1] + u[1] * a + v[1] * b];
    prism(&[at(-half[0], -half[1]), at(half[0], -half[1]), at(half[0], half[1]), at(-half[0], half[1])], z)
}

fn prism(ring: &[[f64; 2]], z: [f64; 2]) -> Mesh {
    let mut r = ring.to_vec();
    if crate::polygons::area(&r) < 0.0 {
        r.reverse();
    }
    crate::bodies::extrude_between(&r, &[], &|_| z[0], &|_| z[1])
}

/// The markings' solids: deck, NO DIVING and wall tiles, and the break-point band's plan
/// rings on the floor (drawn by the pool, which follows its floor) and its wall strips.
pub fn solids(store: &Store, rec: &PoolRecipe, deck_z: f64, water_z: f64, wall_top: f64) -> (Vec<Mesh>, Vec<crate::polygons::Ring>) {
    let mut meshes = Vec::new();
    for mk in markers(store, rec) {
        let (a, b) = (rec.outline[mk.edge - 1], rec.outline[mk.edge % rec.outline.len()]);
        let (nin, l) = inward(a, b);
        let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
        meshes.push(tile(mk.at, u, nin, [0.3, 0.1], [deck_z, deck_z + 0.003]));
        if mk.no_diving {
            let c = [mk.at[0] + u[0] * 0.55, mk.at[1] + u[1] * 0.55];
            meshes.push(tile(c, u, nin, [0.15, 0.15], [deck_z, deck_z + 0.003]));
        }
        let w = [mk.edge_point[0] + nin[0] * 0.0025, mk.edge_point[1] + nin[1] * 0.0025];
        meshes.push(tile(w, u, nin, [0.3, 0.0025], [wall_top - 0.115, wall_top]));
    }
    let mut floor = Vec::new();
    let mut walls = Vec::new();
    if let Some(ln) = lines(rec) {
        let [a, u] = {
            let [a, b] = rec.axis;
            let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
            [a, [(b[0] - a[0]) / l, (b[1] - a[1]) / l]]
        };
        let v = [-u[1], u[0]];
        let big = rec.outline.iter().map(|q| (q[0] - a[0]).hypot(q[1] - a[1])).fold(0.0, f64::max) + 1.0;
        let at = |t: f64, s: f64| [a[0] + u[0] * t + v[0] * s, a[1] + u[1] * t + v[1] * s];
        let (t0, t1) = (ln.band[1], ln.band[0]);
        floor.push(vec![at(t0, -big), at(t1, -big), at(t1, big), at(t0, big)]);
        // Up each wall the band crosses: its stretch of the edge, 5 mm proud, floor to water.
        let n = rec.outline.len();
        for i in 0..n {
            let (p, q) = (rec.outline[i], rec.outline[(i + 1) % n]);
            let (tp, tq) = (rec.t(p), rec.t(q));
            if (tq - tp).abs() < 1e-9 {
                continue;
            }
            let fa = ((t0 - tp) / (tq - tp)).clamp(0.0, 1.0);
            let fb = ((t1 - tp) / (tq - tp)).clamp(0.0, 1.0);
            if (fb - fa).abs() < 1e-9 {
                continue;
            }
            let (nin, _) = inward(p, q);
            let pt = |f: f64, d: f64| [p[0] + (q[0] - p[0]) * f + nin[0] * d, p[1] + (q[1] - p[1]) * f + nin[1] * d];
            let ring = vec![pt(fa.min(fb), 0.0), pt(fa.max(fb), 0.0), pt(fa.max(fb), 0.005), pt(fa.min(fb), 0.005)];
            let bottom = water_z - rec.depth_at((t0 + t1) / 2.0);
            walls.push(prism(&ring, [bottom, water_z]));
        }
    }
    meshes.extend(walls);
    (meshes, floor)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkingsParams {
    pub pool_id: ElementRef,
    /// Default: metres under the UK code setting, else feet.
    pub units: Option<Units>,
    /// Default 1 ft (0.3048 m; the MAHC's), or 0.5 m in metres.
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub increment: Option<f64>,
    /// Default 25 ft (7.62 m).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub spacing: Option<f64>,
    pub no_diving: Option<bool>,
    pub break_line: Option<bool>,
    /// Takes the markings away.
    #[serde(default)]
    pub remove: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Markings {
    pub markers: Vec<Marker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<Lines>,
}

/// Sets a pool's depth markings (`pool.depth_markings`); the markers back.
pub(crate) fn set(txn: &mut Transaction<'_>, p: MarkingsParams) -> Result<Markings> {
    let store = txn.store().clone();
    let id = store.resolve(&p.pool_id).ok_or_else(|| crate::error::Error::new("element.not_found", "no such pool"))?;
    let mut rec = store.get(id).and_then(pools::recipe).ok_or_else(|| crate::error::Error::new("pool.not_a_pool", "not a pool"))?;
    let old = rec.depth_markings;
    rec.depth_markings = if p.remove {
        None
    } else {
        let units = p.units.or(old.map(|m| m.units)).unwrap_or(if crate::codes::settings(&store).code == crate::codes::Code::UkAdk { Units::Metres } else { Units::Feet });
        Some(DepthMarkings {
            units,
            increment: p.increment.or(old.map(|m| m.increment)).unwrap_or(if units == Units::Metres { 0.5 } else { FT }),
            spacing: p.spacing.or(old.map(|m| m.spacing)).unwrap_or(25.0 * FT),
            no_diving: p.no_diving.or(old.map(|m| m.no_diving)).unwrap_or(true),
            break_line: p.break_line.or(old.map(|m| m.break_line)).unwrap_or(true),
        })
    };
    let v = serde_json::to_value(&rec).expect("serialises");
    txn.modify(id, |e| {
        e.params.insert(crate::walls::RECIPE_KEY.into(), v);
    })?;
    Ok(Markings { markers: markers(&store, &rec), lines: lines(&rec) })
}
