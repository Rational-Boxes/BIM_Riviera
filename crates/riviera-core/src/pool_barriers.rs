// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! The barrier round a pool (ARC-POOL-012, `POOLS_PLAN.md` §5, research/23 A.8).
//!
//! Railings (their openings left open, their gates shut) and walls at the pool's level are drawn
//! onto a 0.1 m grid, and the ground the pool's water stands in is filled outward. If the fill
//! reaches the edge of the grid, nothing encloses the pool. Otherwise the railings, gates and
//! walls the enclosed ground touches are its barrier, each with its height, its gap under the
//! bottom rail, its balusters' clear gap and the distance between its rails, and each gate with
//! the side it swings to; the doors and windows in its walls are listed too (ISPSC 305.4). The
//! rule packs judge them (`barrier` and `gate_swing` pool rules).

use schemars::JsonSchema;
use serde::Serialize;

use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::pools::{self, PoolRecipe};
use crate::railings::{self, FeatureKind};
use crate::store::Store;

const CELL: f64 = 0.1;

/// What a stretch of barrier is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BarrierKind {
    Railing,
    Wall,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Barrier {
    pub element: ElementId,
    pub name: String,
    pub kind: BarrierKind,
    /// Its top over the ground it stands on (m).
    pub height: f64,
    /// Railings: the gap under the bottom rail, the balusters' clear gap and the distance
    /// between the tops of the bottom and top rails (m).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clearance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opening: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rails_apart: Option<f64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Gate {
    pub element: ElementId,
    pub railing: ElementId,
    pub at: [f64; 2],
    pub width: f64,
    /// It swings away from the pool, out of the enclosure.
    pub swings_out: bool,
    /// Its railing's height, and its hardware where it is set (ARC-POOL-016).
    pub height: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardware: Option<crate::railings::GateHardware>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WallOpening {
    pub element: ElementId,
    pub wall: ElementId,
    /// `door` or `window`, and its bottom over the ground.
    pub kind: String,
    pub sill: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Enclosure {
    pub pool: ElementId,
    pub name: String,
    pub enclosed: bool,
    /// m² of ground inside the barrier, the pool's water included (enclosed pools).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<f64>,
    pub barriers: Vec<Barrier>,
    pub gates: Vec<Gate>,
    /// Railing openings the fill passed through.
    pub openings: Vec<[f64; 2]>,
    pub wall_openings: Vec<WallOpening>,
}

/// A stretch of barrier drawn on the grid.
struct Run {
    element: ElementId,
    a: [f64; 2],
    b: [f64; 2],
    half: f64,
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = (dx * dx + dy * dy).max(1e-12);
    let f = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0);
    (p[0] - a[0] - f * dx).hypot(p[1] - a[1] - f * dy)
}

/// The point `s` along a path, and the path's direction there.
fn along(path: &[[f64; 2]], closed: bool, s: f64) -> ([f64; 2], [f64; 2]) {
    let n = path.len();
    let segs = if closed { n } else { n - 1 };
    let mut left = s;
    for i in 0..segs {
        let (a, b) = (path[i], path[(i + 1) % n]);
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        if left <= l || i == segs - 1 {
            let f = (left / l.max(1e-12)).clamp(0.0, 1.0);
            return ([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f], [(b[0] - a[0]) / l.max(1e-12), (b[1] - a[1]) / l.max(1e-12)]);
        }
        left -= l;
    }
    (path[0], [1.0, 0.0])
}

/// The pool's barrier, found by filling the ground round it.
pub fn enclosure(store: &Store, id: ElementId, rec: &PoolRecipe, deck_z: f64) -> Enclosure {
    let e = store.get(id).expect("a pool");
    let mut runs: Vec<Run> = Vec::new();
    let mut barriers: Vec<Barrier> = Vec::new();
    let mut gate_spots: Vec<(ElementId, ElementId, [f64; 2], [f64; 2], f64, bool)> = Vec::new();
    let mut open_spots: Vec<[f64; 2]> = Vec::new();
    // Railings standing at about the deck's height.
    for r in store.iter() {
        let Some(rr) = railings::railing_recipe(r) else { continue };
        if matches!(rr.path, railings::RailPath::Stair { .. }) {
            continue;
        }
        let Ok((path, closed, z)) = railings::resolve_path(store, &rr) else { continue };
        if path.len() < 2 || (z - deck_z).abs() > 1.0 {
            continue;
        }
        let t = railings::type_of(store, r);
        // The path less its openings and gates (gates shut afterwards).
        let n = path.len();
        let segs = if closed { n } else { n - 1 };
        let mut cuts: Vec<(f64, f64)> = Vec::new();
        for f in &rr.features {
            let (lo, hi) = (f.at - f.width / 2.0, f.at + f.width / 2.0);
            cuts.push((lo, hi));
            let (p, dir) = along(&path, closed, f.at);
            match &f.kind {
                FeatureKind::Opening => open_spots.push(p),
                FeatureKind::Gate { swing_out, gate, .. } => {
                    let g = crate::id::GlobalId::parse(gate).and_then(|g| store.find_by_guid(&g)).unwrap_or(r.id());
                    gate_spots.push((g, r.id(), p, dir, f.width, *swing_out));
                    let (a, _) = along(&path, closed, lo);
                    let (b, _) = along(&path, closed, hi);
                    runs.push(Run { element: g, a, b, half: 0.06 });
                }
            }
        }
        let mut start = 0.0;
        for i in 0..segs {
            let (a, b) = (path[i], path[(i + 1) % n]);
            let l = (b[0] - a[0]).hypot(b[1] - a[1]);
            let mut pieces = vec![(0.0, l)];
            for (lo, hi) in &cuts {
                let (lo, hi) = (lo - start, hi - start);
                pieces = pieces.into_iter().flat_map(|(p0, p1)| {
                    if hi <= p0 || lo >= p1 { vec![(p0, p1)] } else { [(p0, lo.max(p0)), (hi.min(p1), p1)].into_iter().filter(|(x, y)| y - x > 1e-6).collect() }
                }).collect();
            }
            for (p0, p1) in pieces {
                let at = |s: f64| [a[0] + (b[0] - a[0]) * s / l.max(1e-12), a[1] + (b[1] - a[1]) * s / l.max(1e-12)];
                runs.push(Run { element: r.id(), a: at(p0), b: at(p1), half: (t.post.max(t.top_rail[0]) / 2.0).max(0.06) });
            }
            start += l;
        }
        barriers.push(Barrier {
            element: r.id(),
            name: r.name.clone().unwrap_or_else(|| "Railing".into()),
            kind: BarrierKind::Railing,
            height: t.height,
            clearance: Some((t.bottom_rail_height).max(0.0)),
            opening: Some((t.baluster_spacing - t.baluster).max(0.0)),
            rails_apart: Some(t.height - (t.bottom_rail_height + t.bottom_rail[1])),
        });
    }
    // Walls standing on the deck's level and rising above it.
    for w in store.iter() {
        if crate::walls::wall_recipe(w).is_none() {
            continue;
        }
        let Ok(g) = crate::walls::regenerate(store, w) else { continue };
        if g.base_z > deck_z + 0.5 || g.base_z + g.height < deck_z + 0.3 {
            continue;
        }
        runs.push(Run { element: w.id(), a: g.start, b: g.end, half: (g.width / 2.0).max(0.06) });
        barriers.push(Barrier { element: w.id(), name: w.name.clone().unwrap_or_else(|| "Wall".into()), kind: BarrierKind::Wall, height: g.base_z + g.height - deck_z, clearance: None, opening: None, rails_apart: None });
    }
    // The grid: the pool and the barriers, 1 m round.
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut grow = |p: [f64; 2]| bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
    rec.outline.iter().for_each(|p| grow(*p));
    for r in &runs {
        grow(r.a);
        grow(r.b);
    }
    let (x0, y0) = (bb[0] - 1.0, bb[1] - 1.0);
    let cell = CELL.max(((bb[2] - bb[0] + 2.0) * (bb[3] - bb[1] + 2.0) / 4.0e6).sqrt());
    let (nx, ny) = (((bb[2] + 1.0 - x0) / cell).ceil() as usize + 1, ((bb[3] + 1.0 - y0) / cell).ceil() as usize + 1);
    let centre = |i: usize, j: usize| [x0 + (i as f64 + 0.5) * cell, y0 + (j as f64 + 0.5) * cell];
    // Which run blocks each cell (usize::MAX: none).
    let mut owner = vec![usize::MAX; nx * ny];
    for (k, r) in runs.iter().enumerate() {
        let reach = r.half.max(cell * 0.75);
        let lo = [r.a[0].min(r.b[0]) - reach, r.a[1].min(r.b[1]) - reach];
        let hi = [r.a[0].max(r.b[0]) + reach, r.a[1].max(r.b[1]) + reach];
        let (i0, i1) = (((lo[0] - x0) / cell).floor().max(0.0) as usize, (((hi[0] - x0) / cell).ceil() as usize).min(nx - 1));
        let (j0, j1) = (((lo[1] - y0) / cell).floor().max(0.0) as usize, (((hi[1] - y0) / cell).ceil() as usize).min(ny - 1));
        for j in j0..=j1 {
            for i in i0..=i1 {
                if seg_dist(centre(i, j), r.a, r.b) <= reach {
                    owner[j * nx + i] = k;
                }
            }
        }
    }
    // Fill out from the water.
    let mut seen = vec![false; nx * ny];
    let mut queue = std::collections::VecDeque::new();
    for j in 0..ny {
        for i in 0..nx {
            if owner[j * nx + i] == usize::MAX && crate::sco::inside(centre(i, j), &rec.outline) {
                seen[j * nx + i] = true;
                queue.push_back((i, j));
            }
        }
    }
    let mut escaped = false;
    let mut touched = std::collections::BTreeSet::new();
    let mut cells = 0usize;
    while let Some((i, j)) = queue.pop_front() {
        cells += 1;
        if i == 0 || j == 0 || i == nx - 1 || j == ny - 1 {
            escaped = true;
        }
        for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
            let (a, b) = (i as i64 + di, j as i64 + dj);
            if a < 0 || b < 0 || a >= nx as i64 || b >= ny as i64 {
                continue;
            }
            let k = b as usize * nx + a as usize;
            if owner[k] != usize::MAX {
                touched.insert(runs[owner[k]].element);
                continue;
            }
            if !seen[k] {
                seen[k] = true;
                queue.push_back((a as usize, b as usize));
            }
        }
    }
    let inside = |p: [f64; 2]| {
        let (i, j) = (((p[0] - x0) / cell).floor(), ((p[1] - y0) / cell).floor());
        i >= 0.0 && j >= 0.0 && (i as usize) < nx && (j as usize) < ny && seen[j as usize * nx + i as usize]
    };
    let gates = gate_spots
        .into_iter()
        .filter(|g| touched.contains(&g.0))
        .map(|(gate, railing, at, dir, width, swing_out)| {
            // The swing side: right of the path when it swings out, else left.
            let right = [dir[1], -dir[0]];
            let s = if swing_out { 1.0 } else { -1.0 };
            let side = [at[0] + right[0] * s * 0.4, at[1] + right[1] * s * 0.4];
            let height = barriers.iter().find(|b| b.element == railing).map_or(0.0, |b| b.height);
            let hardware = store.get(gate).and_then(crate::railings::hardware);
            Gate { element: gate, railing, at, width, swings_out: !inside(side), height, hardware }
        })
        .collect();
    let barriers: Vec<Barrier> = barriers.into_iter().filter(|b| touched.contains(&b.element)).collect();
    let mut wall_openings = Vec::new();
    for b in barriers.iter().filter(|b| b.kind == BarrierKind::Wall) {
        let w = store.get(b.element).expect("listed");
        for o in crate::inserts::hosted_by(store, &w.guid().to_string()) {
            let Some(oe) = store.get(o) else { continue };
            let Some(ir) = crate::inserts::insert_recipe(oe) else { continue };
            let kind = match crate::inserts::kind_of(oe) {
                Some(crate::inserts::Kind::Door) => "door",
                Some(_) => "window",
                None => continue,
            };
            let base = crate::walls::regenerate(store, w).map(|g| g.base_z).unwrap_or(deck_z);
            wall_openings.push(WallOpening { element: o, wall: b.element, kind: kind.into(), sill: base + ir.sill - deck_z });
        }
    }
    Enclosure {
        pool: id,
        name: e.name.clone().unwrap_or_default(),
        enclosed: !escaped,
        area: (!escaped).then_some(cells as f64 * cell * cell),
        barriers,
        gates,
        openings: open_spots.into_iter().filter(|p| inside(*p)).collect(),
        wall_openings,
    }
}

#[derive(Debug, Default, serde::Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BarriersParams {
    /// One pool; every pool when omitted.
    pub pool_id: Option<ElementRef>,
}

/// Each pool's barrier (`pool.barriers`).
pub(crate) fn barriers(store: &Store, p: Option<BarriersParams>) -> Result<Vec<Enclosure>> {
    let only = match p.and_then(|p| p.pool_id) {
        Some(r) => Some(store.resolve(&r).ok_or_else(|| Error::new("element.not_found", "no such pool"))?),
        None => None,
    };
    Ok(pools::all(store)
        .into_iter()
        .filter(|(id, _)| only.is_none_or(|o| o == *id))
        .map(|(id, g)| enclosure(store, id, &pools::recipe(store.get(id).expect("listed")).expect("a pool"), g.deck_z))
        .collect())
}
