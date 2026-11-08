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

// ---- Store checks (ARC-RETAIL-002) ----

/// On a room: what part of a store it is.
pub const AREA_KEY: &str = "rxd:retail_area";
/// IBC Table 1004.5 (research/15 G71–G72): mercantile 60 gross ft² a person, storage and stock
/// rooms 300.
const FT2: f64 = 0.092_903_04;
const SALES_FACTOR: f64 = 60.0 * FT2;
const STOCK_FACTOR: f64 = 300.0 * FT2;
/// NFPA 13: storage at least 18 in below a sprinkler's deflector, 36 in below an ESFR head's.
const CLEAR: f64 = 18.0 * IN;
const CLEAR_ESFR: f64 = 36.0 * IN;
/// Storage over 12 ft is high-piled (IFC ch. 32).
const HIGH_PILED: f64 = 12.0 * 12.0 * IN;
/// Pallet racks storing goods 8 ft up in public areas are anchored and restrained (FEMA 460).
const RACK_ANCHOR: f64 = 8.0 * 12.0 * IN;
/// Gondolas over 96 in are always anchored (Madix, S1402).
const GONDOLA_ANCHOR: f64 = 96.0 * IN;
/// A head's reach when its type gives no coverage area: half a 15 ft spacing.
const REACH: f64 = 2.3;

/// The storage fixtures the checks look at.
fn storage() -> crate::rules::Selector {
    crate::rules::Selector { classes: vec!["IfcFurniture".into()], object_types: vec!["Gondola".into(), "WallGondola".into(), "EndCap".into(), "PalletRack".into()], ..Default::default() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    /// The sales floor (mercantile).
    Sales,
    /// Stock and storage rooms.
    Stock,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetAreaParams {
    /// Riviera rooms.
    pub room_ids: Vec<ElementRef>,
    /// What they are; none to unmark them.
    pub area: Option<Area>,
}

/// Marks rooms as a store's sales floor or stock rooms (`retail.set_area`).
pub(crate) fn set_area(txn: &mut Transaction<'_>, p: SetAreaParams) -> Result<()> {
    if p.room_ids.is_empty() {
        return Err(bad("retail.no_rooms", "no rooms given"));
    }
    let store = txn.store().clone();
    for r in &p.room_ids {
        let id = store.resolve(r).filter(|id| store.get(*id).is_some_and(|e| crate::rooms::room_recipe(e).is_some())).ok_or_else(|| bad("retail.not_a_room", "not a Riviera room"))?;
        let v = p.area.map(|a| serde_json::to_value(a).expect("serialises"));
        txn.modify(id, |e| match &v {
            Some(v) => {
                e.params.insert(AREA_KEY.into(), v.clone());
            }
            None => {
                e.params.remove(AREA_KEY);
            }
        })?;
    }
    Ok(())
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckParams {
    /// Only these levels (default every level).
    #[serde(default)]
    pub level_ids: Vec<ElementRef>,
    /// The sales floor's area a person (default 60 ft², 5.57 m²; some jurisdictions use 30 at
    /// grade and in basements).
    #[schemars(extend("x-riviera-data-type" = "area"))]
    pub sales_factor: Option<f64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AreaLoad {
    pub room: ElementId,
    pub number: String,
    pub name: String,
    pub level: String,
    pub area_use: Area,
    /// m².
    pub area: f64,
    /// m² a person.
    pub factor: f64,
    pub occupants: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LevelLoad {
    pub level: String,
    pub sales_area: f64,
    pub stock_area: f64,
    pub occupants: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StoreFinding {
    pub rule: String,
    pub source: String,
    /// `error` or `note`.
    pub severity: String,
    pub element: ElementId,
    pub message: String,
    /// The measured value and the limit (metres), where there are.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StoreCheck {
    pub areas: Vec<AreaLoad>,
    pub levels: Vec<LevelLoad>,
    /// Every area's occupants.
    pub occupants: u32,
    /// Errors first.
    pub findings: Vec<StoreFinding>,
}

/// A head's deflector height, its reach in plan, and whether it is an ESFR head: named so, or
/// quick response with a K-factor of 200 L/min/bar^½ (K14) or more.
fn head(store: &Store, e: &crate::element::Element) -> Option<([f64; 2], f64, f64, bool)> {
    let it = crate::rules::item(store, e)?;
    let ty = crate::mesh_objects::type_of(store, e).and_then(|(id, _)| store.get(id))?;
    let m = crate::mep_sprinklers::mounting(ty);
    let deflector = if m == crate::mep_sprinklers::Mounting::Upright { it.z[1] } else { it.z[0] };
    let val = |k: &str| ty.param(&format!("{}.{k}", crate::mep_sprinklers::PSET)).map(|v| v.get("value").cloned().unwrap_or_else(|| v.clone()));
    let esfr = ty.name.as_deref().is_some_and(|n| n.to_ascii_uppercase().contains("ESFR"))
        || (val("Response").and_then(|v| v.as_str().map(str::to_owned)).as_deref() == Some("QUICK") && val("DischargeCoefficient").and_then(|v| v.as_f64()).is_some_and(|k| k >= 200.0));
    let reach = val("CoverageArea").and_then(|v| v.as_f64()).filter(|a| *a > 0.0).map_or(REACH, |a| a.sqrt() / 2.0);
    let c = it.frame.apply([0.0, 0.0, 0.0]);
    Some(([c[0], c[1]], deflector, reach, esfr))
}

/// Checks a store (`retail.check`): occupant loads from its marked rooms, and its storage
/// fixtures against the sprinklers over them, high-piled storage and anchoring.
pub(crate) fn check(store: &Store, p: Option<CheckParams>) -> Result<StoreCheck> {
    let p = p.unwrap_or_default();
    let sales_factor = p.sales_factor.unwrap_or(SALES_FACTOR);
    if sales_factor <= 0.0 {
        return Err(bad("retail.bad_factor", "the sales floor's area a person must be above zero"));
    }
    let wanted: Vec<String> = p
        .level_ids
        .iter()
        .map(|r| store.resolve(r).and_then(|id| store.get(id)).map(|e| e.guid().to_string()).ok_or_else(|| bad("element.not_found", "no such level")))
        .collect::<Result<_>>()?;
    let level_name = |guid: &str| crate::id::GlobalId::parse(guid).and_then(|g| store.find_by_guid(&g)).and_then(|l| store.get(l)).and_then(|l| l.name.clone()).unwrap_or_default();
    let mut areas = Vec::new();
    let mut levels: Vec<(String, LevelLoad, f64)> = Vec::new();
    // Sales rooms by level, to know which fixtures stand in public areas.
    let mut sales_regions = Vec::new();
    for r in crate::rooms::all(store) {
        let Some(e) = store.get(r.id) else { continue };
        let Some(area_use) = e.param(AREA_KEY).and_then(|v| serde_json::from_value::<Area>(v.clone()).ok()) else { continue };
        if (!wanted.is_empty() && !wanted.contains(&r.level)) || r.redundant {
            continue;
        }
        let area = r.geometry.area();
        let factor = if area_use == Area::Sales { sales_factor } else { STOCK_FACTOR };
        let occupants = (area / factor - 1e-9).ceil().max(0.0) as u32;
        let name = level_name(&r.level);
        if area_use == Area::Sales
            && let Some(reg) = r.geometry.region.clone()
        {
            sales_regions.push((r.geometry.z_base, r.geometry.z_top, reg));
        }
        let elevation = crate::id::GlobalId::parse(&r.level).and_then(|g| store.find_by_guid(&g)).and_then(|l| store.get(l)).map_or(0.0, crate::levels::elevation);
        match levels.iter_mut().find(|l| l.0 == r.level) {
            Some(l) => {
                l.1.occupants += occupants;
                if area_use == Area::Sales { l.1.sales_area += area } else { l.1.stock_area += area }
            }
            None => levels.push((
                r.level.clone(),
                LevelLoad { level: name.clone(), sales_area: if area_use == Area::Sales { area } else { 0.0 }, stock_area: if area_use == Area::Stock { area } else { 0.0 }, occupants },
                elevation,
            )),
        }
        areas.push(AreaLoad { room: r.id, number: r.number, name: r.name, level: name, area_use, area, factor, occupants });
    }
    levels.sort_by(|a, b| a.2.total_cmp(&b.2));
    let heads: Vec<([f64; 2], f64, f64, bool)> = store.iter().filter(|e| e.class == "IfcFireSuppressionTerminal").filter_map(|e| head(store, e)).collect();
    let sel = storage();
    let mut findings = Vec::new();
    for e in store.iter().filter(|e| sel.selects_element(store, e)) {
        let Some(it) = crate::rules::item(store, e) else { continue };
        let corners: Vec<[f64; 3]> = [[it.x[0], it.front], [it.x[1], it.front], [it.x[1], it.back], [it.x[0], it.back]].iter().map(|q| it.frame.apply([q[0], q[1], 0.0])).collect();
        let (lo, hi) = (
            [corners.iter().map(|c| c[0]).fold(f64::MAX, f64::min), corners.iter().map(|c| c[1]).fold(f64::MAX, f64::min)],
            [corners.iter().map(|c| c[0]).fold(f64::MIN, f64::max), corners.iter().map(|c| c[1]).fold(f64::MIN, f64::max)],
        );
        let top = it.z[1];
        let height = top - it.floor;
        let finding = |rule: &str, source: &str, error: bool, message: String, measured: Option<f64>, limit: Option<f64>| StoreFinding {
            rule: rule.into(),
            source: source.into(),
            severity: if error { "error" } else { "note" }.into(),
            element: e.id(),
            message,
            measured,
            limit,
        };
        // The lowest head over it (in reach of its footprint, above its top).
        let over = heads
            .iter()
            .filter(|(c, d, r, _)| *d > it.floor && c[0] > lo[0] - r && c[0] < hi[0] + r && c[1] > lo[1] - r && c[1] < hi[1] + r)
            .min_by(|a, b| (a.1 - if a.3 { CLEAR_ESFR } else { CLEAR }).total_cmp(&(b.1 - if b.3 { CLEAR_ESFR } else { CLEAR })));
        if let Some((_, d, _, esfr)) = over {
            let need = if *esfr { CLEAR_ESFR } else { CLEAR };
            if d - top < need - 1e-6 {
                findings.push(finding(
                    if *esfr { "NFPA 13 ESFR clearance" } else { "NFPA 13 clearance" },
                    "S1398",
                    true,
                    format!("its top is {:.0} mm under a sprinkler deflector; storage needs {:.0} in{}", (d - top) * 1000.0, need / IN, if *esfr { " under an ESFR head" } else { "" }),
                    Some(d - top),
                    Some(need),
                ));
            }
        }
        if height > HIGH_PILED + 1e-6 {
            findings.push(finding("IFC 3201 high-piled", "S1397", false, format!("{:.1} ft high: high-piled storage, with its permit, aisle and sprinkler rules (IFC ch. 32)", height / (12.0 * IN)), Some(height), Some(HIGH_PILED)));
        }
        let public = sales_regions.iter().any(|(z0, z1, reg)| it.floor >= z0 - 0.05 && it.floor < *z1 && reg.contains([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0]));
        let kind = crate::rules::Selector { object_types: vec!["PalletRack".into()], ..Default::default() };
        if kind.selects_element(store, e) {
            if public && height >= RACK_ANCHOR - 1e-6 {
                findings.push(finding("FEMA 460 anchorage", "S1399", false, format!("a {:.1} ft rack on the sales floor: anchor it, and restrain what is stored 8 ft up and above (FEMA 460)", height / (12.0 * IN)), Some(height), Some(RACK_ANCHOR)));
            }
        } else {
            // Madix's table for wall uprights: unanchored to 54, 78 and 102 in on 12, 16 and 20 in
            // base shelves (six leveller spacings, 6 × deck − 18 in); any gondola over 96 in is
            // anchored.
            let wall = crate::rules::Selector { object_types: vec!["WallGondola".into()], ..Default::default() }.selects_element(store, e);
            let deck = (it.back - it.front).abs() - UPRIGHT;
            let limit = if wall { (6.0 * deck - 18.0 * IN).min(GONDOLA_ANCHOR) } else { GONDOLA_ANCHOR };
            if height > limit + 1e-6 {
                findings.push(finding("Madix anchorage", "S1402", false, format!("{:.0} in high: anchor it to the floor or fix it at the top (unanchored to {:.0} in)", height / IN, limit / IN), Some(height), Some(limit)));
            }
        }
    }
    findings.sort_by_key(|f| f.severity != "error");
    let occupants = areas.iter().map(|a| a.occupants).sum();
    Ok(StoreCheck { areas, levels: levels.into_iter().map(|l| l.1).collect(), occupants, findings })
}
