// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Retail fixture runs (ARC-RETAIL-001, `INTERIORS_PLAN.md` §5.2): a run of gondola sections
//! drawn from a start to an end, filled with the manufacturers' section widths (research/15 B1:
//! 48, 36, 30 and 24 in), and closed by end caps as wide as the island is deep.
//!
//! An island run's line is its centre line; a wall run's is its back line, its front away from
//! the wall behind it (as cabinet runs, [`crate::cabinets`]). Sections are `IfcFurniture` SHELF
//! with ElementType Gondola (island) or WallGondola, end caps EndCap; each width is its own
//! generated type. The aisles between runs are checked by the rule packs' use zones on those
//! kinds ([`crate::rules`]): the ADA's 36 in route and 48 in turn at an end cap, the Group M
//! aisle, and practice cart aisles.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::bath_components::Shape;
use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::mesh_objects::Category;
use crate::store::Store;
use crate::txn::Transaction;

/// The tag on a run's fixtures: `{run, kind, index}`.
pub const RUN_KEY: &str = "rxd:retail_run";
const IN: f64 = 0.0254;
/// The upright between sections (Lozier standard, S1400).
const UPRIGHT: f64 = 2.5 * IN;
/// Lozier's allowance on an island's overall depth (S1400).
const ALLOWANCE: f64 = 1.25 * IN;
const BASE: f64 = 8.0 * IN;
const WHITE: [f32; 4] = [0.93, 0.93, 0.92, 1.0];

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Shelves both sides of a centre line.
    Island,
    /// Shelves on one side, its back to a wall.
    Wall,
}

/// The standard section widths (centre to centre of uprights), the preferred one first.
fn widths(preferred: f64) -> Vec<f64> {
    let mut all: Vec<f64> = [48.0, 36.0, 30.0, 24.0].iter().map(|w| w * IN).filter(|w| (w - preferred).abs() > 1e-9).collect();
    all.insert(0, preferred);
    all
}

/// A section `w` wide, its decks `deck` deep each side of the upright line (one side for a wall
/// section, its back at +y), `h` high with `n` shelves a side; half an upright at each end, so
/// neighbours share one.
fn section(w: f64, deck: f64, h: f64, n: usize, island: bool) -> Shape {
    let up = UPRIGHT;
    let half = 0.5 * IN;
    let y1 = if island { deck + up / 2.0 } else { up / 2.0 };
    let mut s = Shape::default()
        .cuboid([-w / 2.0 + half, w / 2.0 - half], [-up / 2.0, up / 2.0], [BASE, h])
        .cuboid([-w / 2.0, -w / 2.0 + half], [-up / 2.0, up / 2.0], [0.0, h])
        .cuboid([w / 2.0 - half, w / 2.0], [-up / 2.0, up / 2.0], [0.0, h])
        .cuboid([-w / 2.0 + half, w / 2.0 - half], [-(deck + up / 2.0), -up / 2.0], [0.0, BASE]);
    if island {
        s = s.cuboid([-w / 2.0 + half, w / 2.0 - half], [up / 2.0, y1], [0.0, BASE]);
    }
    for k in 1..=n {
        let z = BASE + (h - BASE - 0.15) * k as f64 / n as f64;
        let shelf = deck - 2.0 * IN;
        s = s.cuboid([-w / 2.0 + half, w / 2.0 - half], [-up / 2.0 - shelf, -up / 2.0], [z - 0.02, z]);
        if island {
            s = s.cuboid([-w / 2.0 + half, w / 2.0 - half], [up / 2.0, up / 2.0 + shelf], [z - 0.02, z]);
        }
    }
    s
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunParams {
    /// The run's centre line (island) or back line (wall): from here …
    pub start: [f64; 2],
    /// … to here.
    pub end: [f64; 2],
    pub level: ElementRef,
    pub kind: Kind,
    /// The preferred section width (default 48 in; 24, 30, 36 or 48 in).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub section: Option<f64>,
    /// The base deck's depth each side (default 16 in; 13–31 in).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub deck: Option<f64>,
    /// Default 72 in (island) or 84 in (wall).
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub height: Option<f64>,
    /// End caps at both ends (default for islands; never on a wall run).
    pub end_caps: Option<bool>,
    /// A wall run: a point in front of it; default away from the wall behind it.
    pub toward: Option<[f64; 2]>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Section {
    pub element: ElementId,
    pub width: f64,
    pub end_cap: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Run {
    pub run: String,
    pub kind: Kind,
    /// Drawn length.
    pub length: f64,
    /// The fixtures' overall length and depth.
    pub overall: f64,
    pub depth: f64,
    pub sections: Vec<Section>,
    /// Of the drawn length, what no standard section filled (shared at both ends).
    pub left_over: f64,
}

/// Lays out a run of gondolas (`retail.run`).
pub(crate) fn run(txn: &mut Transaction<'_>, p: RunParams) -> Result<Run> {
    let store = txn.store().clone();
    let level = store.resolve(&p.level).and_then(|l| store.get(l)).ok_or_else(|| bad("element.not_found", "no such level"))?;
    let z0 = crate::levels::elevation(level);
    let island = p.kind == Kind::Island;
    let section_w = p.section.unwrap_or(48.0 * IN);
    if ![24.0, 30.0, 36.0, 48.0].iter().any(|w| (w * IN - section_w).abs() < 1e-4) {
        return Err(bad("retail.bad_section", "sections are 24, 30, 36 or 48 in wide"));
    }
    let deck = p.deck.unwrap_or(16.0 * IN);
    if !(12.9 * IN..=31.1 * IN).contains(&deck) {
        return Err(bad("retail.bad_deck", "base decks are 13 to 31 in deep"));
    }
    let height = p.height.unwrap_or(if island { 72.0 } else { 84.0 } * IN);
    if !(35.9 * IN..=120.1 * IN).contains(&height) {
        return Err(bad("retail.bad_height", "uprights are 36 to 120 in high"));
    }
    let shelves = ((height - BASE) / (16.0 * IN)).round().max(1.0) as usize;
    let caps = island && p.end_caps.unwrap_or(true);
    if p.end_caps == Some(true) && !island {
        return Err(bad("retail.no_caps", "end caps close island runs"));
    }
    // A wall run faces away from its wall; an island's front is to the line's left.
    let (length, d, front, rotation) = if island {
        let v = [p.end[0] - p.start[0], p.end[1] - p.start[1]];
        let l = v[0].hypot(v[1]);
        if l < 0.1 {
            return Err(bad("retail.too_short", "a run needs a start and an end at least 100 mm apart"));
        }
        let d = [v[0] / l, v[1] / l];
        let f = [-d[1], d[0]];
        (l, d, f, f[0].atan2(-f[1]))
    } else {
        crate::cabinets::line_frame(&store, p.start, p.end, z0, p.toward)?
    };
    let depth = if island { 2.0 * deck + UPRIGHT + ALLOWANCE } else { deck + UPRIGHT };
    // An end cap is as wide as the island is deep, and a deck plus an upright long.
    let cap = deck + UPRIGHT;
    let room = length - if caps { 2.0 * cap } else { 0.0 };
    let (ws, left_over) = crate::cabinets::fill(room, &widths(section_w));
    if ws.is_empty() {
        return Err(bad("retail.too_short", format!("the run is {length:.3} m: too short for a section{}", if caps { " between its end caps" } else { "" })));
    }
    let used: f64 = ws.iter().sum();
    let left_over = if left_over > 0.0 { room - used } else { 0.0 };
    let (name, kind) = if island { ("Gondola, island", "Gondola") } else { ("Gondola, wall", "WallGondola") };
    let run_id = crate::id::GlobalId::new().as_str().to_owned();
    let at_line = |s: f64, out: f64| [p.start[0] + d[0] * s + front[0] * out, p.start[1] + d[1] * s + front[1] * out];
    // A wall section's upright line is half an upright in front of the back line.
    let out = if island { 0.0 } else { UPRIGHT / 2.0 };
    let mut sections = Vec::new();
    let mut place = |txn: &mut Transaction<'_>, ty: ElementId, point: [f64; 2], rotation: f64, k: usize, w: f64, end_cap: bool| -> Result<()> {
        let id = crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(ty), point, level: p.level.clone(), offset: 0.0, rotation, mirror: false, scale: 1.0 })?;
        let tag = json!({ "run": run_id, "kind": p.kind, "index": k });
        txn.modify(id, |e| {
            e.params.insert(RUN_KEY.into(), tag);
        })?;
        sections.push(Section { element: id, width: w, end_cap });
        Ok(())
    };
    let mut s = left_over / 2.0 + if caps { cap } else { 0.0 };
    let label = |v: f64| format!("{:.0}", v / IN);
    for (k, w) in ws.iter().enumerate() {
        let ty_name = format!("{name} {} × {} × {} in", label(*w), label(deck), label(height));
        let ty = fixture_type(txn, &ty_name, kind, &section(*w, deck, height, shelves, island), [*w, depth, height])?;
        place(txn, ty, at_line(s + w / 2.0, out), rotation, k, *w, false)?;
        s += w;
    }
    if caps {
        let ty_name = format!("Gondola end cap {} × {} × {} in", label(depth), label(deck), label(height));
        let ty = fixture_type(txn, &ty_name, "EndCap", &section(depth, deck, height, shelves, false), [depth, cap, height])?;
        // Each faces out along the line, its back against the sections.
        let first = left_over / 2.0 + cap - UPRIGHT / 2.0;
        let last = s + UPRIGHT / 2.0;
        let back = (-d[0]).atan2(d[1]);
        let fwd = d[0].atan2(-d[1]);
        place(txn, ty, at_line(first, 0.0), back, ws.len(), depth, true)?;
        place(txn, ty, at_line(last, 0.0), fwd, ws.len() + 1, depth, true)?;
    }
    let overall = used + if caps { 2.0 * cap } else { 0.0 };
    Ok(Run { run: run_id, kind: p.kind, length, overall, depth, sections, left_over })
}

/// A generated gondola type of this name, made once, with `Riviera_RetailFixture` and its
/// nominal sizes.
fn fixture_type(txn: &mut Transaction<'_>, name: &str, kind: &str, sh: &Shape, size: [f64; 3]) -> Result<ElementId> {
    if let Some(e) = txn.store().iter().find(|e| e.name.as_deref() == Some(name) && crate::mesh_objects::mesh_object_type(e).is_some()) {
        return Ok(e.id());
    }
    let id = crate::mesh_objects::create_generated_type(txn, name, Category::Furniture, &sh.p, &sh.t, WHITE, Vec::new(), Some("SHELF"))?;
    let len = |v: f64| json!({ "type": "IfcPositiveLengthMeasure", "value": v });
    let kind: Value = kind.into();
    txn.modify(id, |e| {
        e.params.insert("@ElementType".into(), kind.clone());
        e.params.insert("Pset_FurnitureTypeCommon.IsBuiltIn".into(), json!({ "type": "IfcBoolean", "value": false }));
        e.params.insert("Pset_FurnitureTypeCommon.NominalLength".into(), len(size[0]));
        e.params.insert("Pset_FurnitureTypeCommon.NominalDepth".into(), len(size[1]));
        e.params.insert("Pset_FurnitureTypeCommon.NominalHeight".into(), len(size[2]));
    })?;
    Ok(id)
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunsParams {}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RunInfo {
    pub run: String,
    pub kind: Kind,
    /// Its fixtures, in order along it (end caps last).
    pub fixtures: Vec<ElementId>,
}

/// The fixture runs in the model (`retail.runs`).
pub(crate) fn runs(store: &Store, _: Option<RunsParams>) -> Result<Vec<RunInfo>> {
    let mut out: Vec<(RunInfo, Vec<u64>)> = Vec::new();
    for e in store.iter() {
        let Some(t) = e.param(RUN_KEY) else { continue };
        let (Some(run), Ok(kind)) = (t.get("run").and_then(Value::as_str), serde_json::from_value::<Kind>(t.get("kind").cloned().unwrap_or_default())) else { continue };
        let index = t.get("index").and_then(Value::as_u64).unwrap_or(0);
        match out.iter_mut().find(|r| r.0.run == run) {
            Some(r) => {
                r.0.fixtures.push(e.id());
                r.1.push(index);
            }
            None => out.push((RunInfo { run: run.to_owned(), kind, fixtures: vec![e.id()] }, vec![index])),
        }
    }
    Ok(out
        .into_iter()
        .map(|(mut r, idx)| {
            let mut pairs: Vec<(u64, ElementId)> = idx.into_iter().zip(r.fixtures).collect();
            pairs.sort_by_key(|p| p.0);
            r.fixtures = pairs.into_iter().map(|p| p.1).collect();
            r
        })
        .collect())
}
