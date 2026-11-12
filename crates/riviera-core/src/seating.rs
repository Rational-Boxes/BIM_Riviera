// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Event and dining seating (spec Ch 3, ARC-INT-006; research/20 C4, C5, D3, E2–E3): a room set
//! out for a banquet, theatre, classroom, boardroom, dining or reception, with its seats counted
//! against the IBC's occupant load, exits and egress width.
//!
//! - **The set-up** fills the room's region, kept `perimeter` clear of its walls (default 44 in,
//!   an egress aisle), in the room's frame: x along its longest edge, the front at the side of
//!   `front` (a point; default the region's lower side in that frame).
//!   - banquet: rounds (72 in, ten stack chairs; 60 in, eight) on a square grid `pitch` apart
//!     (default the round plus two chairs and 24 in);
//!   - theatre: stack chairs 20 in apart in rows 36 in back to back, `front` clear for a stage
//!     (default 3 m), in blocks of at most 14 between 48 in aisles;
//!   - classroom: 18 × 72 in seminar tables for three in rows 54 in apart, in blocks of at most
//!     three tables between 48 in aisles, facing the front;
//!   - boardroom: a 96 × 42 in conference table for eight in the middle;
//!   - dining: 36 in four-tops with four chairs 7 ft apart;
//!   - reception: 30 in cocktail rounds 3 m apart, standing.
//!   Every piece is a library item (*Riviera Interiors*), marked with the room; setting out a room
//!   again replaces its set-up.
//! - **The occupant load** (IBC Table 1004.5): the room's net area over 15 ft² for tables and
//!   chairs, 7 ft² for chairs only (theatre) and 5 ft² standing (reception). A set-up with more
//!   seats than that is a note: it takes the increased-load route (1004.5.1), with egress for the
//!   larger number.
//! - **Exits** (Table 1006.3.2): one for up to 49, two to 500, three to 1,000, four above, counted
//!   as the doors in the room's walls; their widths against 0.2 in an occupant (0.15 in where
//!   sprinklered, 1005.3.2).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::store::Store;
use crate::txn::Transaction;

/// On each piece: the room's GlobalId.
pub const KEY: &str = "rxd:seating";
const IN: f64 = 0.0254;
const FT2: f64 = 0.3048 * 0.3048;

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Banquet,
    Theatre,
    Classroom,
    Boardroom,
    Dining,
    Reception,
}

impl Style {
    /// IBC Table 1004.5's net area an occupant (ft²) and what it is for.
    fn factor(self) -> (f64, &'static str) {
        match self {
            Style::Theatre => (7.0, "concentrated, chairs only"),
            Style::Reception => (5.0, "standing space"),
            _ => (15.0, "unconcentrated, tables and chairs"),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutParams {
    pub room_id: ElementRef,
    pub style: Style,
    /// Banquet rounds: 72 (default) or 60 in.
    pub round: Option<u32>,
    /// Between table centres (banquet, dining, reception).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub pitch: Option<f64>,
    /// Kept clear of the walls (default 44 in).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub perimeter: Option<f64>,
    /// A point toward the front (theatre, classroom), and the depth kept clear there (default 3 m).
    pub front: Option<[f64; 2]>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub stage: Option<f64>,
    /// Whether the building is sprinklered with voice alarm (default true: 0.15 in an occupant).
    pub sprinklered: Option<bool>,
    /// Take the room's set-up away.
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Finding {
    pub rule: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Layout {
    pub style: Style,
    pub tables: usize,
    pub chairs: usize,
    /// Seats (chairs), or standing places for a reception.
    pub seats: usize,
    /// The room's net area, m², and per seat, ft².
    pub area: f64,
    pub per_seat: f64,
    /// IBC 1004.5's occupant load, and the load egress is designed for (the larger of it and the
    /// seats).
    pub code_load: u32,
    pub design_load: u32,
    pub exits_needed: u32,
    pub doors: usize,
    /// Inches of egress width needed and the doors' total.
    pub width_needed: f64,
    pub width: f64,
    pub elements: Vec<ElementId>,
    pub findings: Vec<Finding>,
}

/// The room's region (outer ring), its level and GlobalId.
fn room(store: &Store, id: ElementId) -> Result<(Vec<[f64; 2]>, String, String)> {
    let e = store.get(id).ok_or_else(|| bad("element.not_found", format!("no element {id}")))?;
    let rec = crate::rooms::room_recipe(e).ok_or_else(|| bad("seating.room", format!("element {id} is not a room")))?;
    let g = crate::rooms::regenerate(store, e)?;
    let region = g.region.ok_or_else(|| bad("seating.room", format!("room {id} is not enclosed")))?;
    Ok((region.outer.iter().map(|x| x.line.start).collect(), rec.level, e.guid().to_string()))
}

/// A frame on the region: its longest edge's direction, and the front at the lower side unless
/// `front` is nearer the upper.
fn frame(r: &[[f64; 2]], front: Option<[f64; 2]>) -> ([f64; 2], [f64; 2]) {
    let n = r.len();
    let k = (0..n).max_by(|a, b| {
        let l = |i: usize| (r[(i + 1) % n][0] - r[i][0]).hypot(r[(i + 1) % n][1] - r[i][1]);
        l(*a).total_cmp(&l(*b))
    });
    let (a, b) = (r[k.unwrap_or(0)], r[(k.unwrap_or(0) + 1) % n]);
    let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
    let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
    let mut v = [-u[1], u[0]];
    if let Some(f) = front {
        let vs: Vec<f64> = r.iter().map(|p| p[0] * v[0] + p[1] * v[1]).collect();
        let (lo, hi) = (vs.iter().copied().fold(f64::MAX, f64::min), vs.iter().copied().fold(f64::MIN, f64::max));
        let fv = f[0] * v[0] + f[1] * v[1];
        if fv - lo > hi - fv {
            v = [-v[0], -v[1]];
        }
    }
    (u, v)
}

/// What to place: an item's name, its centre and its rotation.
type Piece = (&'static str, [f64; 2], f64);

/// Even centres from `lo` to `hi` (inclusive of a piece `size` wide) at `pitch`, centred.
fn centres(lo: f64, hi: f64, size: f64, pitch: f64) -> Vec<f64> {
    if hi - lo < size {
        return Vec::new();
    }
    let n = ((hi - lo - size) / pitch).floor() as usize + 1;
    let start = (lo + hi) / 2.0 - (n as f64 - 1.0) * pitch / 2.0;
    (0..n).map(|k| start + k as f64 * pitch).collect()
}

/// Splits `n` seats or tables into blocks of at most `max`, as evenly as can be.
fn blocks(n: usize, max: usize) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    let b = n.div_ceil(max);
    (0..b).map(|k| n / b + usize::from(k < n % b)).collect()
}

/// A chair at `p` turned to face `toward` (a chair's back is at its +y).
fn facing(p: [f64; 2], toward: [f64; 2]) -> f64 {
    (toward[1] - p[1]).atan2(toward[0] - p[0]) + std::f64::consts::FRAC_PI_2
}

/// `seating.layout`.
pub(crate) fn layout(txn: &mut Transaction<'_>, p: LayoutParams) -> Result<Layout> {
    let store = txn.store().clone();
    let id = store.resolve(&p.room_id).ok_or_else(|| bad("element.not_found", format!("no element {:?}", p.room_id)))?;
    let (ring, level, room_guid) = room(&store, id)?;
    // The room's old set-up goes.
    for e in store.iter().filter(|e| e.param(KEY).and_then(Value::as_str) == Some(room_guid.as_str())) {
        txn.delete(e.id())?;
    }
    let area = crate::polygons::area(&ring).abs();
    let (u, v) = frame(&ring, p.front);
    let to_local = |q: [f64; 2]| [q[0] * u[0] + q[1] * u[1], q[0] * v[0] + q[1] * v[1]];
    let to_world = |l: [f64; 2]| [u[0] * l[0] + v[0] * l[1], u[1] * l[0] + v[1] * l[1]];
    let local: Vec<[f64; 2]> = ring.iter().map(|q| to_local(*q)).collect();
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for q in &local {
        lo = [lo[0].min(q[0]), lo[1].min(q[1])];
        hi = [hi[0].max(q[0]), hi[1].max(q[1])];
    }
    let clear = p.perimeter.unwrap_or(44.0 * IN);
    let (u0, u1, v0, v1) = (lo[0] + clear, hi[0] - clear, lo[1] + clear, hi[1] - clear);
    // A footprint fits where its corners are inside the region, `clear` from its edges.
    let fits = |c: [f64; 2], half: [f64; 2]| {
        [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]].iter().all(|s| {
            let q = [c[0] + s[0] * half[0], c[1] + s[1] * half[1]];
            crate::sco::inside(q, &local) && crate::pools::edge_distance(q, &local) >= clear - 1e-6
        })
    };
    // The front's facing: toward −v in the local frame.
    let front_rot = facing([0.0, 0.0], [0.0, -1.0]);
    let chair_d = 22.0 * IN;
    let mut pieces: Vec<Piece> = Vec::new();
    let mut tables = 0;
    let mut standing = 0;
    match p.style {
        Style::Banquet => {
            let (dia, seats, name) = if p.round == Some(60) { (60.0 * IN, 8, "Banquet round 60 in") } else { (72.0 * IN, 10, "Banquet round 72 in") };
            let reach = dia / 2.0 + chair_d;
            let pitch = p.pitch.unwrap_or(2.0 * reach + 24.0 * IN);
            for x in centres(u0, u1, 2.0 * reach, pitch) {
                for y in centres(v0, v1, 2.0 * reach, pitch) {
                    if !fits([x, y], [reach, reach]) {
                        continue;
                    }
                    tables += 1;
                    pieces.push((name, [x, y], 0.0));
                    for k in 0..seats {
                        let a = std::f64::consts::TAU * k as f64 / seats as f64;
                        let c = [x + (dia / 2.0 + chair_d / 2.0) * a.cos(), y + (dia / 2.0 + chair_d / 2.0) * a.sin()];
                        pieces.push(("Stack chair 18 × 22 in", c, facing(c, [x, y])));
                    }
                }
            }
        }
        Style::Theatre => {
            let (seat, row, aisle) = (20.0 * IN, 36.0 * IN, 48.0 * IN);
            let front = v0 + p.stage.unwrap_or(3.0);
            let width = u1 - u0;
            let mut n = (width / seat).floor() as usize;
            let mut bl = blocks(n, 14);
            while !bl.is_empty() && bl.len() > 1 && (n as f64 * seat + (bl.len() - 1) as f64 * aisle) > width + 1e-9 {
                n -= 1;
                bl = blocks(n, 14);
            }
            let total = n as f64 * seat + bl.len().saturating_sub(1) as f64 * aisle;
            let mut y = front + chair_d / 2.0;
            while y + chair_d / 2.0 <= v1 + 1e-9 {
                let mut x = (u0 + u1) / 2.0 - total / 2.0;
                for b in &bl {
                    for _ in 0..*b {
                        let c = [x + seat / 2.0, y];
                        if fits(c, [9.0 * IN, chair_d / 2.0]) {
                            pieces.push(("Stack chair 18 × 22 in", c, front_rot));
                        }
                        x += seat;
                    }
                    x += aisle;
                }
                y += row;
            }
        }
        Style::Classroom => {
            let (tw, td, row, aisle) = (72.0 * IN, 18.0 * IN, 54.0 * IN, 48.0 * IN);
            let front = v0 + p.stage.unwrap_or(3.0);
            let width = u1 - u0;
            let mut n = (width / tw).floor() as usize;
            let mut bl = blocks(n, 3);
            while bl.len() > 1 && (n as f64 * tw + (bl.len() - 1) as f64 * aisle) > width + 1e-9 {
                n -= 1;
                bl = blocks(n, 3);
            }
            let total = n as f64 * tw + bl.len().saturating_sub(1) as f64 * aisle;
            let mut y = front + td / 2.0;
            while y + td / 2.0 + chair_d <= v1 + 1e-9 {
                let mut x = (u0 + u1) / 2.0 - total / 2.0;
                for b in &bl {
                    for _ in 0..*b {
                        let c = [x + tw / 2.0, y];
                        if fits(c, [tw / 2.0, td / 2.0 + chair_d]) {
                            tables += 1;
                            pieces.push(("Seminar table 18 × 72 in", c, 0.0));
                            for k in [-1.0, 0.0, 1.0] {
                                pieces.push(("Stack chair 18 × 22 in", [c[0] + k * 24.0 * IN, c[1] + td / 2.0 + chair_d / 2.0], front_rot));
                            }
                        }
                        x += tw;
                    }
                    x += aisle;
                }
                y += row;
            }
        }
        Style::Boardroom => {
            let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
            let (w, d) = (96.0 * IN, 42.0 * IN);
            if fits(c, [w / 2.0 + chair_d, d / 2.0 + chair_d]) {
                tables = 1;
                pieces.push(("Conference table, 8 seats 96 × 42 in", c, 0.0));
                for k in [-1.0, 0.0, 1.0] {
                    for s in [-1.0, 1.0] {
                        let q = [c[0] + k * 30.0 * IN, c[1] + s * (d / 2.0 + chair_d / 2.0)];
                        pieces.push(("Dining chair", q, facing(q, [q[0], c[1]])));
                    }
                }
                for s in [-1.0, 1.0] {
                    let q = [c[0] + s * (w / 2.0 + chair_d / 2.0), c[1]];
                    pieces.push(("Dining chair", q, facing(q, c)));
                }
            }
        }
        Style::Dining => {
            let half = 18.0 * IN;
            let reach = half + 20.0 * IN;
            let pitch = p.pitch.unwrap_or(7.0 * 12.0 * IN);
            for x in centres(u0, u1, 2.0 * reach, pitch) {
                for y in centres(v0, v1, 2.0 * reach, pitch) {
                    if !fits([x, y], [reach, reach]) {
                        continue;
                    }
                    tables += 1;
                    pieces.push(("Restaurant table, 4 seats 36 × 36 in", [x, y], 0.0));
                    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                        let q = [x + dx * (half + 10.0 * IN), y + dy * (half + 10.0 * IN)];
                        pieces.push(("Dining chair", q, facing(q, [x, y])));
                    }
                }
            }
        }
        Style::Reception => {
            let pitch = p.pitch.unwrap_or(3.0);
            for x in centres(u0, u1, 30.0 * IN, pitch) {
                for y in centres(v0, v1, 30.0 * IN, pitch) {
                    if fits([x, y], [15.0 * IN, 15.0 * IN]) {
                        tables += 1;
                        pieces.push(("Cocktail table 30 in", [x, y], 0.0));
                    }
                }
            }
            standing = (area / (7.0 * FT2)).floor() as usize;
        }
    }
    let mut elements = Vec::new();
    if !p.clear && !pieces.is_empty() {
        let mut names: Vec<String> = pieces.iter().map(|(n, ..)| (*n).to_owned()).collect();
        names.sort();
        names.dedup();
        let types: BTreeMap<String, ElementId> = crate::furnishings::load(txn, crate::furnishings::LoadParams { names })?;
        let turn = v[1].atan2(v[0]) - std::f64::consts::FRAC_PI_2;
        for (name, at, rot) in &pieces {
            let w = to_world(*at);
            let place: crate::mesh_objects::PlaceParams = serde_json::from_value(json!({ "type_id": types[*name], "point": w, "level": level, "rotation": rot + turn })).expect("valid");
            let e = crate::mesh_objects::place(txn, place)?;
            txn.modify(e, |x| {
                x.params.insert(KEY.into(), Value::from(room_guid.as_str()));
            })?;
            elements.push(e);
        }
    }
    let chairs = if p.clear { 0 } else { pieces.iter().filter(|(n, ..)| n.contains("chair")).count() };
    let tables = if p.clear { 0 } else { tables };
    let seats = if p.style == Style::Reception { standing } else { chairs };
    // The occupant load, exits and egress width.
    let (factor, what) = p.style.factor();
    let code_load = (area / (factor * FT2)).floor() as u32;
    let design_load = code_load.max(seats as u32);
    let exits_needed = match design_load {
        0..=49 => 1,
        50..=500 => 2,
        501..=1000 => 3,
        _ => 4,
    };
    let s = txn.store().clone();
    let doors: Vec<f64> = crate::inserts::placed(&s)
        .into_iter()
        .filter(|d| d.kind == crate::inserts::Kind::Door)
        .filter(|d| {
            let a = d.recipe.along;
            let (dir, st) = (d.wall.direction(), d.wall.start);
            let c = [st[0] + dir[0] * a, st[1] + dir[1] * a];
            crate::pools::edge_distance(c, &ring) <= d.wall.width / 2.0 + 0.2 && d.z0 <= crate::levels::elevation_of(&s, &level).unwrap_or(0.0) + 0.5
        })
        .map(|d| d.recipe.width.unwrap_or(d.ty.width) / IN)
        .collect();
    let per = if p.sprinklered.unwrap_or(true) { 0.15 } else { 0.2 };
    let width_needed = f64::from(design_load) * per;
    let width: f64 = doors.iter().sum();
    let mut findings = Vec::new();
    let mut find = |rule: &str, error: bool, message: String| findings.push(Finding { rule: rule.into(), severity: if error { "error" } else { "note" }.into(), message });
    if seats as u32 > code_load {
        find("IBC 1004.5.1", false, format!("{seats} seats is more than the {code_load} of IBC Table 1004.5 ({factor:.0} ft² net each, {what}): an increased occupant load needs an approved plan and egress for {seats}"));
    }
    if (doors.len() as u32) < exits_needed {
        find("IBC 1006.3.2", true, format!("{} door{} for {design_load} occupants; {exits_needed} exits needed", doors.len(), if doors.len() == 1 { "" } else { "s" }));
    }
    if width + 1e-6 < width_needed {
        find("IBC 1005.3.2", true, format!("the doors give {width:.0} in of egress width; {design_load} occupants need {width_needed:.0} in ({per} in each)"));
    }
    Ok(Layout {
        style: p.style,
        tables,
        chairs,
        seats,
        area,
        per_seat: if seats > 0 { area / FT2 / seats as f64 } else { 0.0 },
        code_load,
        design_load,
        exits_needed,
        doors: doors.len(),
        width_needed,
        width,
        elements,
        findings,
    })
}
