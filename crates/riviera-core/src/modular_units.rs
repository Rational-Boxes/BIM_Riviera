// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Modular units (spec Ch 3 §3.13.4, ARC-GRP-004): a unit designed once and repeated to fill a
//! building, every copy following its type, merging with the building's walls and floors and
//! connecting to the services that serve them all.
//!
//! - A **unit type** (`rxd:unit_types` on the project) is a group type (ARC-GRP-001) with a
//!   **bay**: a box `width` along its corridor face and `depth` back from it. Its frame's origin
//!   is the middle of the corridor face, its x axis along the face and its y axis into the unit.
//! - A **unit array** (`IfcGroup`, `recipe.unit_array`) places copies bay to bay along a path on
//!   each of its levels, on one or both sides of a corridor, skipping exclusions along the path,
//!   alternately mirrored, numbered by floor, with variant types every n-th copy. Each copy is a
//!   group instance, its level references moved to the copy's level, marked `rxd:unit_copy`.
//!   Changing the array remakes its copies.
//! - **Merging:** a copy's wall that lies on a wall it does not own (the building's façade or
//!   corridor wall, or a neighbouring copy's demising wall) is not built; its doors and windows
//!   are hosted on that wall instead, cutting their openings there. Walls on the corridor and
//!   façade faces that find nothing to merge with are listed. A slab beyond the façade face (a
//!   balcony) is joined to the floor it meets.
//! - **Services:** `unit.connect` runs a branch from every open duct and pipe connector of the
//!   copies into the nearest level main of the same system on the copy's level
//!   (`mep.connect_into`), joining the main's system, and sums the flows each main serves.
//!
//! Editing a copy in the group editor and finishing edits the type: every copy is remade (and
//! merged again). Not built yet: the template's own views, grids and volume fills, kept
//! GlobalIds, connection points placed by hand, sizing the mains from the sums, zones.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::element::{Element, NewElement};
use crate::error::{Error, Result};
use crate::groups::{self, GroupType};
use crate::id::{ElementId, ElementRef, GlobalId};
use crate::store::Store;
use crate::txn::Transaction;
use crate::walls::RECIPE_KEY;

pub const TYPES_KEY: &str = "rxd:unit_types";
pub const ARRAY_RECIPE: &str = "recipe.unit_array";
/// On a copy (its group instance): the array, number and how it was placed.
pub const COPY_KEY: &str = "rxd:unit_copy";
/// On a branch `unit.connect` drew: the array's GlobalId.
pub const BRANCH_KEY: &str = "rxd:unit_branch";

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

/// A unit type: its group type and bay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnitType {
    pub id: String,
    pub name: String,
    /// The group type holding its template.
    pub group_type: String,
    pub width: f64,
    pub depth: f64,
}

fn project(store: &Store) -> Option<&Element> {
    store.iter().find(|e| e.class == "IfcProject")
}

pub fn types(store: &Store) -> Vec<UnitType> {
    project(store).and_then(|p| p.param(TYPES_KEY)).and_then(|v| serde::Deserialize::deserialize(v).ok()).unwrap_or_default()
}

fn unit_type(store: &Store, r: &str) -> Result<UnitType> {
    types(store).into_iter().find(|t| t.id == r || t.name == r).ok_or_else(|| bad("unit.no_type", format!("no unit type {r}")))
}

fn write_types(txn: &mut Transaction<'_>, list: &[UnitType]) -> Result<()> {
    let p = project(txn.store()).map(Element::id).ok_or_else(|| bad("project.missing", "the model has no project"))?;
    let v = serde_json::to_value(list).expect("serialises");
    txn.modify(p, |e| {
        e.params.insert(TYPES_KEY.into(), v);
    })
}

fn find(store: &Store, guid: &str) -> Option<ElementId> {
    store.find_by_guid(&GlobalId::parse(guid)?)
}

fn resolve(store: &Store, r: &ElementRef) -> Result<ElementId> {
    store.resolve(r).ok_or_else(|| bad("element.not_found", format!("no element {r:?}")))
}

// ---- Unit types ----

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateTypeParams {
    /// The unit's elements: rooms, walls, floors, furniture, terminals (doors and windows of its
    /// walls come with them).
    pub element_ids: Vec<ElementRef>,
    pub name: String,
    /// The bay's corner on the corridor face where its x axis starts.
    pub origin: [f64; 2],
    /// The x axis's direction: along the corridor face (radians or "90°"); the unit lies to its
    /// left.
    #[serde(default)]
    #[schemars(extend("x-riviera-data-type" = "angle"))]
    pub rotation: f64,
    /// Along the corridor face and back from it.
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub width: f64,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub depth: f64,
    /// Keep the elements it was made from as they are (default false: they become the type and
    /// are taken away).
    #[serde(default)]
    pub keep: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreatedType {
    pub type_id: String,
    pub group_type: String,
    /// What it took, by class.
    pub members: BTreeMap<String, usize>,
}

/// `unit.create_type`: a unit type from elements and its bay.
pub(crate) fn create_type(txn: &mut Transaction<'_>, p: CreateTypeParams) -> Result<CreatedType> {
    let name = p.name.trim().to_owned();
    let mut list = types(txn.store());
    if name.is_empty() || list.iter().any(|t| t.name == name) {
        return Err(bad("unit.name", "a unit type needs a name of its own"));
    }
    if !(p.width > 0.0 && p.depth > 0.0) {
        return Err(bad("unit.bay", "a unit's bay needs a width and a depth"));
    }
    // A group type of the elements, its frame the middle of the bay's corridor face.
    let ids: Vec<ElementRef> = p.element_ids.clone();
    let made = groups::create(txn, groups::CreateParams { element_ids: ids, name: format!("{name} (unit)") })?;
    let (s, c) = p.rotation.sin_cos();
    let origin = [p.origin[0] + c * p.width / 2.0, p.origin[1] + s * p.width / 2.0, 0.0];
    let store = txn.store().clone();
    let mut gtypes = groups::types(&store);
    let mut members = BTreeMap::new();
    for t in gtypes.iter_mut().filter(|t| t.id == made.group_type_id) {
        t.origin = origin;
        t.rotation = p.rotation;
        for m in &t.template {
            *members.entry(m.class.clone()).or_insert(0) += 1;
        }
    }
    groups::write_types(txn, &gtypes)?;
    // The instance it was made from: kept, or taken away with its members.
    let inst = made.instance_id;
    if p.keep {
        groups::ungroup(txn, groups::InstancesParams { instance_ids: vec![ElementRef::Id(inst)] })?;
    } else {
        let g = groups::recipe(txn.store().get(inst).expect("made")).expect("a group");
        groups::delete_members(txn, &g)?;
        txn.delete(inst)?;
    }
    let id = GlobalId::new().to_string();
    list.push(UnitType { id: id.clone(), name, group_type: made.group_type_id.clone(), width: p.width, depth: p.depth });
    write_types(txn, &list)?;
    Ok(CreatedType { type_id: id, group_type: made.group_type_id, members })
}

// ---- Arrays ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Sides {
    #[default]
    Left,
    Right,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Handing {
    /// Every other copy mirrored, so neighbours share their walls and services (default).
    #[default]
    Alternate,
    None,
}

/// Every `every`-th copy on each level, from the `first` (1-based), takes `unit_type` instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub unit_type: String,
    pub every: u32,
    #[serde(default = "one")]
    pub first: u32,
}

fn one() -> u32 {
    1
}

/// Numbers: `prefix`, then the floor (counting from 1 at the lowest level at or above zero, so
/// basements do not shift the numbers) times `per_level`,
/// plus `start` and the copy's place on its level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Numbering {
    #[serde(default)]
    pub prefix: String,
    #[serde(default = "hundred")]
    pub per_level: u32,
    #[serde(default = "one")]
    pub start: u32,
}

fn hundred() -> u32 {
    100
}

impl Default for Numbering {
    fn default() -> Self {
        Numbering { prefix: String::new(), per_level: 100, start: 1 }
    }
}

/// A unit array's recipe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ArrayRecipe {
    pub kind: String,
    pub unit_type: String,
    /// The corridor's centre line, start to end.
    pub path: [[f64; 2]; 2],
    /// Levels (GlobalIds).
    pub levels: Vec<String>,
    #[serde(default)]
    pub sides: Sides,
    /// The corridor's width: the bays start half of it to each side of the path.
    #[serde(default)]
    pub corridor: f64,
    /// The pitch between bays (default the bay's width).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f64>,
    /// At most this many a side on each level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Where the first bay starts along the path.
    #[serde(default)]
    pub start: f64,
    /// Stretches along the path no bay may reach into (cores, stairs, plant rooms).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclusions: Vec<[f64; 2]>,
    #[serde(default)]
    pub handing: Handing,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<Variant>,
    #[serde(default)]
    pub numbering: Numbering,
}

pub fn array_recipe(e: &Element) -> Option<ArrayRecipe> {
    let v = e.param(RECIPE_KEY)?;
    (v.get("kind")?.as_str()? == ARRAY_RECIPE).then(|| serde::Deserialize::deserialize(v).ok())?
}

/// A copy as placed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CopyInfo {
    pub array: String,
    pub number: String,
    pub level: String,
    pub unit_type: String,
    pub side: Sides,
    pub mirrored: bool,
    /// Its walls built into others (by member key), and the faces that found nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merged: Vec<u32>,
    /// Doors, windows and wall-hung fixtures moved into the wall a merged wall lay on: their key
    /// to the merged wall's key.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rehosted: BTreeMap<u32, u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unmerged: Vec<String>,
}

pub fn copy_info(e: &Element) -> Option<CopyInfo> {
    serde_json::from_value(e.param(COPY_KEY)?.clone()).ok()
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArrayParams {
    pub unit_type: String,
    pub path: [[f64; 2]; 2],
    pub levels: Vec<ElementRef>,
    pub sides: Option<Sides>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub corridor: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub pitch: Option<f64>,
    pub count: Option<u32>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub start: Option<f64>,
    #[serde(default)]
    pub exclusions: Vec<[f64; 2]>,
    pub handing: Option<Handing>,
    #[serde(default)]
    pub variants: Vec<Variant>,
    pub numbering: Option<Numbering>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ArrayReport {
    pub array_id: ElementId,
    pub copies: usize,
    /// Copies on each level, by level name.
    pub per_level: BTreeMap<String, usize>,
    /// Copies of each unit type.
    pub per_type: BTreeMap<String, usize>,
    /// Faces of copies that found nothing to merge with: "201: façade".
    pub unmerged: Vec<String>,
}

fn level_guid(store: &Store, r: &ElementRef) -> Result<String> {
    let id = resolve(store, r)?;
    let e = store.get(id).expect("resolved");
    if e.class != "IfcBuildingStorey" {
        return Err(bad("unit.level", format!("element {id} is not a level")));
    }
    Ok(e.guid().to_string())
}

/// `unit.array`: copies of a unit type along a corridor on each level.
pub(crate) fn array(txn: &mut Transaction<'_>, p: ArrayParams) -> Result<ArrayReport> {
    let store = txn.store().clone();
    let t = unit_type(&store, &p.unit_type)?;
    let levels = p.levels.iter().map(|l| level_guid(&store, l)).collect::<Result<Vec<_>>>()?;
    let rec = ArrayRecipe {
        kind: ARRAY_RECIPE.into(),
        unit_type: t.id.clone(),
        path: p.path,
        levels,
        sides: p.sides.unwrap_or_default(),
        corridor: p.corridor.unwrap_or(0.0),
        pitch: p.pitch,
        count: p.count,
        start: p.start.unwrap_or(0.0),
        exclusions: p.exclusions,
        handing: p.handing.unwrap_or_default(),
        variants: p.variants,
        numbering: p.numbering.unwrap_or_default(),
    };
    validate(&store, &rec)?;
    let name = p.name.unwrap_or_else(|| format!("{} array", t.name));
    let id = txn.create(NewElement::new("IfcGroup").name(name.as_str()).param("@ObjectType", "UnitArray").param(RECIPE_KEY, serde_json::to_value(&rec).expect("serialises")))?;
    place_copies(txn, id, &rec)
}

fn validate(store: &Store, rec: &ArrayRecipe) -> Result<()> {
    let t = unit_type(store, &rec.unit_type)?;
    let [a, b] = rec.path;
    if (b[0] - a[0]).hypot(b[1] - a[1]) < t.width {
        return Err(bad("unit.path", format!("the path is shorter than one {:.2} m bay", t.width)));
    }
    if rec.levels.is_empty() {
        return Err(bad("unit.levels", "choose the levels to fill"));
    }
    if rec.pitch.is_some_and(|p| p < t.width - 1e-9) || rec.corridor < 0.0 {
        return Err(bad("unit.pitch", format!("the pitch must be at least the bay's {:.2} m, and the corridor zero or more", t.width)));
    }
    for v in &rec.variants {
        let vt = unit_type(store, &v.unit_type)?;
        if v.every == 0 || (vt.width - t.width).abs() > 1e-6 {
            return Err(bad("unit.variant", format!("a variant needs `every` of 1 or more and the same bay width as {}", t.name)));
        }
    }
    Ok(())
}

/// The bays along the path: their starts.
fn bays(rec: &ArrayRecipe, width: f64) -> Vec<f64> {
    let [a, b] = rec.path;
    let length = (b[0] - a[0]).hypot(b[1] - a[1]);
    let pitch = rec.pitch.unwrap_or(width);
    let mut out = Vec::new();
    let mut t = rec.start;
    while t + width <= length + 1e-9 {
        if let Some(x) = rec.exclusions.iter().find(|[lo, hi]| t < *hi - 1e-9 && t + width > *lo + 1e-9) {
            // Past the exclusion, on the pitch.
            let skip = ((x[1] - t) / pitch).ceil().max(1.0);
            t += skip * pitch;
            continue;
        }
        if rec.count.is_some_and(|c| out.len() >= c as usize) {
            break;
        }
        out.push(t);
        t += pitch;
    }
    out
}

/// Places an array's copies (its old ones and their branches gone first).
fn place_copies(txn: &mut Transaction<'_>, array_id: ElementId, rec: &ArrayRecipe) -> Result<ArrayReport> {
    let store = txn.store().clone();
    let guid = store.get(array_id).expect("exists").guid().to_string();
    remove_copies(txn, &guid)?;
    let t = unit_type(&store, &rec.unit_type)?;
    let all_levels = crate::levels::list(&store);
    let [a, b] = rec.path;
    let l = (b[0] - a[0]).hypot(b[1] - a[1]);
    let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
    let left = [-u[1], u[0]];
    let angle = u[1].atan2(u[0]);
    let starts = bays(rec, t.width);
    let sides: &[Sides] = match rec.sides {
        Sides::Both => &[Sides::Left, Sides::Right],
        Sides::Left => &[Sides::Left],
        Sides::Right => &[Sides::Right],
    };
    let mut report = ArrayReport { array_id, copies: 0, per_level: BTreeMap::new(), per_type: BTreeMap::new(), unmerged: Vec::new() };
    for level in &rec.levels {
        let Some(at) = all_levels.iter().position(|x| &x.guid == level) else { continue };
        let level_name = all_levels[at].name.clone();
        // Floors count up from the lowest level at or above zero (the ground floor is 1);
        // anything below it is 0.
        let ground = all_levels.iter().position(|x| x.elevation >= -1e-6).unwrap_or(0) as i64;
        let floor = (at as i64 - ground + 1).max(0) as u32;
        let mut seq = 0u32;
        for side in sides {
            for (k, s) in starts.iter().enumerate() {
                seq += 1;
                // Variants by the copy's place on its level.
                let ty = rec.variants.iter().find(|v| seq >= v.first && (seq - v.first) % v.every == 0).map(|v| unit_type(&store, &v.unit_type)).transpose()?.unwrap_or_else(|| t.clone());
                let gt = groups::group_type(&store, &ty.group_type)?;
                let mid = s + t.width / 2.0;
                let (origin, rotation) = match side {
                    Sides::Left => ([a[0] + u[0] * mid + left[0] * rec.corridor / 2.0, a[1] + u[1] * mid + left[1] * rec.corridor / 2.0], angle),
                    _ => ([a[0] + u[0] * mid - left[0] * rec.corridor / 2.0, a[1] + u[1] * mid - left[1] * rec.corridor / 2.0], angle + std::f64::consts::PI),
                };
                let mirrored = rec.handing == Handing::Alternate && k % 2 == 1;
                let number = format!("{}{}", rec.numbering.prefix, floor * rec.numbering.per_level + rec.numbering.start + seq - 1);
                // The template's levels to this one.
                let template_levels = template_levels(&gt);
                let levels: BTreeMap<String, String> = template_levels.into_iter().filter(|g| g != level).map(|g| (g, level.clone())).collect();
                let info = CopyInfo { array: guid.clone(), number: number.clone(), level: level.clone(), unit_type: ty.id.clone(), side: *side, mirrored, merged: Vec::new(), rehosted: BTreeMap::new(), unmerged: Vec::new() };
                let copy = groups::place_with(txn, &gt, [origin[0], origin[1], gt.origin[2]], rotation, mirrored, levels)?;
                txn.modify(copy, |e| {
                    e.name = Some(format!("{} {number}", ty.name));
                    e.params.insert(COPY_KEY.into(), serde_json::to_value(&info).expect("serialises"));
                })?;
                number_rooms(txn, copy, &number)?;
                let info = merge(txn, copy)?;
                report.unmerged.extend(info.unmerged.iter().map(|f| format!("{number}: {f}")));
                report.copies += 1;
                *report.per_level.entry(level_name.clone()).or_insert(0) += 1;
                *report.per_type.entry(ty.name.clone()).or_insert(0) += 1;
            }
        }
    }
    Ok(report)
}

/// The levels a group type's members refer to.
fn template_levels(t: &GroupType) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for m in &t.template {
        for key in ["base_level", "level", "level_id", "reference_level", "top_level"] {
            let v = m.params.get(RECIPE_KEY).and_then(|r| r.get(key)).and_then(Value::as_str);
            if let Some(g) = v
                && GlobalId::parse(g).is_some()
                && !out.iter().any(|x| x == g)
            {
                out.push(g.to_owned());
            }
        }
        if let Some(g) = m.params.get("@level").and_then(Value::as_str)
            && !out.iter().any(|x| x == g)
        {
            out.push(g.to_owned());
        }
    }
    out
}

/// A copy's rooms take its number (a second room gets "A", "B", …).
fn number_rooms(txn: &mut Transaction<'_>, copy: ElementId, number: &str) -> Result<()> {
    let store = txn.store().clone();
    let rooms: Vec<ElementId> = members(&store, copy).into_iter().filter(|id| store.get(*id).is_some_and(|e| crate::rooms::room_recipe(e).is_some())).collect();
    groups::internally(txn, |txn| {
        for (k, r) in rooms.iter().enumerate() {
            let n = if k == 0 { number.to_owned() } else { format!("{number}{}", (b'A' + (k as u8 - 1).min(25)) as char) };
            txn.modify(*r, |e| e.name = Some(n))?;
        }
        Ok(())
    })
}

/// A copy's members as they are now.
pub fn members(store: &Store, copy: ElementId) -> Vec<ElementId> {
    store.get(copy).and_then(groups::recipe).map(|g| g.members.iter().filter_map(|m| find(store, &m.guid)).collect()).unwrap_or_default()
}

/// Takes an array's copies away, with their members and branches.
fn remove_copies(txn: &mut Transaction<'_>, array: &str) -> Result<()> {
    let store = txn.store().clone();
    for e in store.iter() {
        if copy_info(e).is_some_and(|c| c.array == array) {
            if let Some(g) = groups::recipe(e) {
                groups::delete_members(txn, &g)?;
            }
            txn.delete(e.id())?;
        } else if e.param(BRANCH_KEY).and_then(Value::as_str) == Some(array) && txn.store().get(e.id()).is_some() {
            txn.delete(e.id())?;
        }
    }
    Ok(())
}

// ---- Merging ----

/// A wall's centre line, base and width.
fn wall(store: &Store, id: ElementId) -> Option<crate::walls::WallGeometry> {
    let e = store.get(id)?;
    crate::walls::wall_recipe(e)?;
    crate::walls::regenerate(store, e).ok()
}

/// Whether wall `a` lies on wall `b`: parallel, overlapping in plan and height, along most of
/// `a`'s length.
fn lies_on(a: &crate::walls::WallGeometry, b: &crate::walls::WallGeometry) -> bool {
    let (da, db) = (a.direction(), b.direction());
    if (da[0] * db[1] - da[1] * db[0]).abs() > 0.02 {
        return false;
    }
    if (a.base_z - b.base_z).abs() > 0.05 && (a.base_z >= b.base_z + b.height || b.base_z >= a.base_z + a.height) {
        return false;
    }
    let n = b.normal();
    let off = (a.start[0] - b.start[0]) * n[0] + (a.start[1] - b.start[1]) * n[1];
    if off.abs() > (a.width + b.width) / 2.0 + 0.01 {
        return false;
    }
    let along = |p: [f64; 2]| (p[0] - b.start[0]) * db[0] + (p[1] - b.start[1]) * db[1];
    let (s, e) = (along(a.start), along(a.end));
    let (lo, hi) = (s.min(e).max(0.0), s.max(e).min(b.length()));
    hi - lo >= 0.5 * a.length() - 1e-6
}

/// The face of the bay a point of a copy is on, in the copy's frame.
fn face(origin: [f64; 2], rotation: f64, depth: f64, p: [f64; 2], slack: f64) -> Option<&'static str> {
    let (s, c) = rotation.sin_cos();
    let ly = -(p[0] - origin[0]) * s + (p[1] - origin[1]) * c;
    if ly.abs() <= slack {
        Some("corridor")
    } else if (ly - depth).abs() <= slack {
        Some("façade")
    } else {
        None
    }
}

/// Merges a copy with what it meets; its copy info back.
pub(crate) fn merge(txn: &mut Transaction<'_>, copy: ElementId) -> Result<CopyInfo> {
    let store = txn.store().clone();
    let e = store.get(copy).ok_or_else(|| bad("unit.no_copy", "no such copy"))?;
    let mut info = copy_info(e).ok_or_else(|| bad("unit.no_copy", format!("element {copy} is not a unit copy")))?;
    let g = groups::recipe(e).expect("a group");
    let depth = unit_type(&store, &info.unit_type).map_or(0.0, |t| t.depth);
    let mine: Vec<(u32, ElementId)> = g.members.iter().filter_map(|m| Some((m.key, find(&store, &m.guid)?))).collect();
    let mine_ids: Vec<ElementId> = mine.iter().map(|(_, id)| *id).collect();
    // Walls on the copy's level only (a merge needs the same level).
    let others: Vec<(ElementId, crate::walls::WallGeometry)> = store
        .iter()
        .filter(|o| !mine_ids.contains(&o.id()) && crate::walls::wall_recipe(o).is_some_and(|r| r.base_level == info.level))
        .filter_map(|o| Some((o.id(), wall(&store, o.id())?)))
        .collect();
    let (origin, rotation) = ([g.origin[0], g.origin[1]], g.rotation);
    info.merged.clear();
    info.rehosted.clear();
    info.unmerged.clear();
    groups::internally(txn, |txn| {
        for (key, id) in &mine {
            let Some(a) = wall(&store, *id) else { continue };
            let mid = [(a.start[0] + a.end[0]) / 2.0, (a.start[1] + a.end[1]) / 2.0];
            let Some((host, b)) = others.iter().find(|(o, b)| txn.store().get(*o).is_some() && lies_on(&a, b)) else {
                if let Some(f) = face(origin, rotation, depth, mid, a.width / 2.0 + 0.05) {
                    info.unmerged.push(format!("{f} wall"));
                }
                continue;
            };
            // Its doors and windows into the wall it lies on, then it goes.
            let host_guid = store.get(*host).expect("exists").guid().to_string();
            let own_guid = store.get(*id).expect("exists").guid().to_string();
            for i in crate::inserts::hosted_by(txn.store(), &own_guid) {
                rehost(txn, i, &a, &host_guid, b)?;
                if let Some((k, _)) = mine.iter().find(|(_, x)| *x == i) {
                    info.rehosted.insert(*k, *key);
                }
            }
            // Fixtures on its faces (a bathroom's on its side wall) onto the same face of the other.
            for i in crate::mesh_objects::on_face_of(txn.store(), &own_guid) {
                crate::mesh_objects::rehost_face(txn, i, &host_guid)?;
                if let Some((k, _)) = mine.iter().find(|(_, x)| *x == i) {
                    info.rehosted.insert(*k, *key);
                }
            }
            txn.delete(*id)?;
            info.merged.push(*key);
        }
        // Balconies: slabs beyond the façade face, joined to the floor they meet.
        for (_, id) in &mine {
            let Some(s) = txn.store().get(*id) else { continue };
            if s.class != "IfcSlab" {
                continue;
            }
            let Some(outline) = slab_outline(txn.store(), *id) else { continue };
            let (sn, cs) = rotation.sin_cos();
            let ly = |p: &[f64; 2]| -(p[0] - origin[0]) * sn + (p[1] - origin[1]) * cs;
            if outline.iter().map(ly).fold(f64::MAX, f64::min) < depth - 0.05 {
                continue;
            }
            let level = s.param(RECIPE_KEY).and_then(|r| r.get("level")).and_then(Value::as_str).map(str::to_owned);
            let floor = txn.store().iter().filter(|o| o.class == "IfcSlab" && !mine_ids.contains(&o.id())).filter(|o| level.is_none() || o.param(RECIPE_KEY).and_then(|r| r.get("level")).and_then(Value::as_str) == level.as_deref()).find_map(|o| {
                let other = slab_outline(txn.store(), o.id())?;
                touches(&outline, &other).then(|| o.guid().to_string())
            });
            match floor {
                Some(f) => txn.modify(*id, |e| {
                    e.params.insert("rxd:joined_to".into(), Value::from(f));
                })?,
                None => info.unmerged.push("balcony".into()),
            }
        }
        // Neighbours' fixtures merged onto a wall of this copy that was made again.
        crate::mesh_objects::reattach_orphans(txn)
    })?;
    let v = serde_json::to_value(&info).expect("serialises");
    txn.modify(copy, |e| {
        e.params.insert(COPY_KEY.into(), v);
    })?;
    Ok(info)
}

/// Moves a door or window from wall `a` into wall `b` (GlobalId `host`), where it is.
fn rehost(txn: &mut Transaction<'_>, insert: ElementId, a: &crate::walls::WallGeometry, host: &str, b: &crate::walls::WallGeometry) -> Result<()> {
    let Some(mut r) = txn.store().get(insert).and_then(crate::inserts::insert_recipe) else { return Ok(()) };
    let (da, db) = (a.direction(), b.direction());
    let p = [a.start[0] + da[0] * r.along, a.start[1] + da[1] * r.along];
    r.host = host.to_owned();
    r.along = (p[0] - b.start[0]) * db[0] + (p[1] - b.start[1]) * db[1];
    r.sill += a.base_z - b.base_z;
    let (na, nb) = (a.normal(), b.normal());
    if na[0] * nb[0] + na[1] * nb[1] < 0.0 {
        r.flip_facing = !r.flip_facing;
        r.flip_hand = !r.flip_hand;
    }
    let v = serde_json::to_value(&r).expect("serialises");
    txn.modify(insert, |e| {
        e.params.insert(RECIPE_KEY.into(), v);
    })
}

/// Undoes a copy's merges before its members become the template (`group.finish`): its merged
/// walls are made again from the template, and its doors and windows go back into them.
pub(crate) fn unmerge(txn: &mut Transaction<'_>, copy: ElementId) -> Result<()> {
    let store = txn.store().clone();
    let Some(e) = store.get(copy) else { return Ok(()) };
    let (Some(mut info), Some(mut g)) = (copy_info(e), groups::recipe(e)) else { return Ok(()) };
    if info.merged.is_empty() {
        return Ok(());
    }
    let t = groups::group_type(&store, &g.group_type)?;
    let guid = e.guid().to_string();
    let made = groups::make_members(txn, &t, &info.merged, g.origin, g.rotation, &guid, &g.members, g.mirrored, &g.levels)?;
    groups::internally(txn, |txn| {
        for m in &made {
            let Some(wid) = find(txn.store(), &m.guid) else { continue };
            let Some(wa) = wall(txn.store(), wid) else { continue };
            for (ins, _) in info.rehosted.iter().filter(|(_, w)| **w == m.key) {
                let Some(iid) = g.members.iter().find(|x| x.key == *ins).and_then(|x| find(txn.store(), &x.guid)) else { continue };
                if txn.store().get(iid).and_then(crate::mesh_objects::mesh_object_recipe).is_some() {
                    crate::mesh_objects::rehost_face(txn, iid, &m.guid)?;
                    continue;
                }
                let Some(host) = txn.store().get(iid).and_then(crate::inserts::insert_recipe).and_then(|r| find(txn.store(), &r.host)) else { continue };
                let Some(wb) = wall(txn.store(), host) else { continue };
                rehost(txn, iid, &wb, &m.guid, &wa)?;
            }
        }
        Ok(())
    })?;
    for m in made {
        match g.members.iter_mut().find(|x| x.key == m.key) {
            Some(x) => x.guid = m.guid,
            None => g.members.push(m),
        }
    }
    info.merged.clear();
    info.rehosted.clear();
    let (gv, iv) = (serde_json::to_value(&g).expect("serialises"), serde_json::to_value(&info).expect("serialises"));
    txn.modify(copy, |e| {
        e.params.insert(RECIPE_KEY.into(), gv);
        e.params.insert(COPY_KEY.into(), iv);
    })
}

/// A floor's outer outlines in plan, as one list of points (enough for a box).
fn slab_outline(store: &Store, id: ElementId) -> Option<Vec<[f64; 2]>> {
    let g = crate::floors::regenerate(store, store.get(id)?).ok()?;
    let pts: Vec<[f64; 2]> = g.profiles.iter().flat_map(|p| p.outer.iter().copied()).collect();
    (!pts.is_empty()).then_some(pts)
}

/// Whether two plan boxes touch or overlap, or are no further apart than a façade wall is thick
/// (0.35 m): a balcony starts at the wall's outer face, the floor at its line.
fn touches(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    let bx = |r: &[[f64; 2]]| r.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |m, p| [m[0].min(p[0]), m[1].min(p[1]), m[2].max(p[0]), m[3].max(p[1])]);
    let (p, q) = (bx(a), bx(b));
    const GAP: f64 = 0.35;
    p[0] <= q[2] + GAP && q[0] <= p[2] + GAP && p[1] <= q[3] + GAP && q[1] <= p[3] + GAP
}

// ---- Changing and listing ----

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetArrayParams {
    pub array_id: ElementRef,
    pub path: Option<[[f64; 2]; 2]>,
    pub levels: Option<Vec<ElementRef>>,
    pub sides: Option<Sides>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub corridor: Option<f64>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub pitch: Option<f64>,
    pub count: Option<u32>,
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub start: Option<f64>,
    pub exclusions: Option<Vec<[f64; 2]>>,
    pub handing: Option<Handing>,
    pub variants: Option<Vec<Variant>>,
    pub numbering: Option<Numbering>,
    pub unit_type: Option<String>,
}

/// `unit.set_array`: changes an array; its copies are made again.
pub(crate) fn set_array(txn: &mut Transaction<'_>, p: SetArrayParams) -> Result<ArrayReport> {
    let store = txn.store().clone();
    let id = resolve(&store, &p.array_id)?;
    let mut rec = store.get(id).and_then(array_recipe).ok_or_else(|| bad("unit.not_array", format!("element {id} is not a unit array")))?;
    if let Some(v) = p.path {
        rec.path = v;
    }
    if let Some(v) = p.levels {
        rec.levels = v.iter().map(|l| level_guid(&store, l)).collect::<Result<_>>()?;
    }
    if let Some(t) = p.unit_type {
        rec.unit_type = unit_type(&store, &t)?.id;
    }
    rec.sides = p.sides.unwrap_or(rec.sides);
    rec.corridor = p.corridor.unwrap_or(rec.corridor);
    rec.pitch = p.pitch.or(rec.pitch);
    rec.count = p.count.or(rec.count);
    rec.start = p.start.unwrap_or(rec.start);
    rec.exclusions = p.exclusions.unwrap_or(rec.exclusions);
    rec.handing = p.handing.unwrap_or(rec.handing);
    rec.variants = p.variants.unwrap_or(rec.variants);
    rec.numbering = p.numbering.unwrap_or(rec.numbering);
    validate(&store, &rec)?;
    let v = serde_json::to_value(&rec).expect("serialises");
    txn.modify(id, |e| {
        e.params.insert(RECIPE_KEY.into(), v);
    })?;
    place_copies(txn, id, &rec)
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListParams {
    pub array_id: Option<ElementRef>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Listed {
    pub copy: ElementId,
    pub number: String,
    pub level: String,
    pub unit_type: String,
    pub side: Sides,
    pub mirrored: bool,
    /// Its open duct and pipe connectors left (none once connected).
    pub open_connectors: usize,
    pub merged: usize,
    pub unmerged: Vec<String>,
}

/// `unit.list`: copies, by array.
pub(crate) fn list(store: &Store, p: Option<ListParams>) -> Result<Vec<Listed>> {
    let only = match p.and_then(|p| p.array_id) {
        Some(r) => Some(store.get(resolve(store, &r)?).expect("resolved").guid().to_string()),
        None => None,
    };
    let names: BTreeMap<String, String> = types(store).into_iter().map(|t| (t.id, t.name)).collect();
    let levels: BTreeMap<String, String> = crate::levels::list(store).into_iter().map(|l| (l.guid, l.name)).collect();
    let open = crate::mep::open_ports(store, &[crate::mep::Domain::Duct, crate::mep::Domain::Pipe]);
    let mut out: Vec<Listed> = store
        .iter()
        .filter_map(|e| Some((e, copy_info(e)?)))
        .filter(|(_, c)| only.as_ref().is_none_or(|a| &c.array == a))
        .map(|(e, c)| {
            let m = members(store, e.id());
            Listed {
                copy: e.id(),
                number: c.number.clone(),
                level: levels.get(&c.level).cloned().unwrap_or_default(),
                unit_type: names.get(&c.unit_type).cloned().unwrap_or_default(),
                side: c.side,
                mirrored: c.mirrored,
                open_connectors: open.iter().filter(|p| m.contains(&p.element)).count(),
                merged: c.merged.len(),
                unmerged: c.unmerged.clone(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.number.cmp(&b.number));
    Ok(out)
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TypeListed {
    pub id: String,
    pub name: String,
    pub width: f64,
    pub depth: f64,
    pub members: usize,
    pub copies: usize,
}

/// `unit.types`.
pub(crate) fn list_types(store: &Store, _: Option<ListParams>) -> Result<Vec<TypeListed>> {
    let g = groups::types(store);
    Ok(types(store)
        .into_iter()
        .map(|t| TypeListed {
            members: g.iter().find(|x| x.id == t.group_type).map_or(0, |x| x.template.len()),
            copies: store.iter().filter(|e| copy_info(e).is_some_and(|c| c.unit_type == t.id)).count(),
            id: t.id,
            name: t.name,
            width: t.width,
            depth: t.depth,
        })
        .collect())
}

// ---- Editing in isolation ----

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditParams {
    /// A copy, one of its members, or a unit array (its first copy) …
    pub element_id: Option<ElementRef>,
    /// … or a unit type, by key or name (its first copy).
    pub unit_type: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Editing {
    /// The copy opened in the group editor.
    pub copy: ElementId,
    pub number: String,
    pub unit_type: String,
    /// The copies that follow it when the edit finishes, this one included.
    pub copies: usize,
    /// Its members as they are now, the walls it had merged into the building's made again.
    pub members: Vec<ElementId>,
}

/// The copy `unit.edit` opens: the one given, a member's, or the first (not mirrored, then
/// lowest number) of an array or a type.
fn copy_to_edit(store: &Store, p: &EditParams) -> Result<ElementId> {
    let first = |pick: &dyn Fn(&CopyInfo) -> bool| {
        let mut c: Vec<(bool, String, ElementId)> = store.iter().filter_map(|e| Some((e, copy_info(e)?))).filter(|(_, c)| pick(c)).map(|(e, c)| (c.mirrored, c.number, e.id())).collect();
        c.sort();
        c.first().map(|x| x.2).ok_or_else(|| bad("unit.no_copy", "it has no copies to edit"))
    };
    if let Some(r) = &p.element_id {
        let id = resolve(store, r)?;
        let e = store.get(id).expect("resolved");
        if copy_info(e).is_some() {
            return Ok(id);
        }
        if array_recipe(e).is_some() {
            let guid = e.guid().to_string();
            return first(&|c| c.array == guid);
        }
        if let Some(g) = groups::instance_of(e).and_then(|g| find(store, &g)).filter(|g| store.get(*g).and_then(copy_info).is_some()) {
            return Ok(g);
        }
        return Err(bad("unit.no_copy", format!("element {id} is not a unit copy, one of its members or a unit array")));
    }
    let Some(t) = &p.unit_type else {
        return Err(bad("unit.no_copy", "choose a copy, a unit array or a unit type to edit"));
    };
    let t = unit_type(store, t)?;
    first(&|c| c.unit_type == t.id)
}

/// `unit.edit`: opens a copy in the group editor to edit its type in isolation; the walls it
/// merged into the building's are made again so the whole unit is there. Finish (`group.finish`)
/// remakes every copy from it, merged again; Cancel puts it back as it was.
pub(crate) fn edit(txn: &mut Transaction<'_>, p: EditParams) -> Result<Editing> {
    let copy = copy_to_edit(txn.store(), &p)?;
    groups::edit(txn, groups::InstanceParams { instance_id: ElementRef::Id(copy) })?;
    unmerge(txn, copy)?;
    let store = txn.store();
    let info = copy_info(store.get(copy).expect("found")).expect("a copy");
    Ok(Editing {
        copy,
        number: info.number,
        unit_type: unit_type(store, &info.unit_type).map(|t| t.name).unwrap_or_default(),
        copies: store.iter().filter(|e| copy_info(e).is_some_and(|c| c.unit_type == info.unit_type)).count(),
        members: members(store, copy),
    })
}

// ---- Services ----

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConnectParams {
    /// An array, or copies.
    pub array_id: Option<ElementRef>,
    #[serde(default)]
    pub copy_ids: Vec<ElementRef>,
    /// The mains to connect to: straight level ducts and pipes.
    pub spines: Vec<ElementRef>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Unconnected {
    pub number: String,
    pub element: ElementId,
    pub connector: String,
    pub reason: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SpineLoad {
    pub spine: ElementId,
    /// Branches into it and the flow they carry (m³/s for air and water, as the connectors).
    pub branches: usize,
    pub flow: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Connected {
    pub branches: usize,
    pub unconnected: Vec<Unconnected>,
    pub spines: Vec<SpineLoad>,
}

/// `unit.connect`: a branch from every open duct and pipe connector of the copies into the
/// nearest main of its system on its level.
pub(crate) fn connect(txn: &mut Transaction<'_>, p: ConnectParams) -> Result<Connected> {
    let store = txn.store().clone();
    let mut copies: Vec<ElementId> = p.copy_ids.iter().map(|r| resolve(&store, r)).collect::<Result<_>>()?;
    let mut array_guid = None;
    if let Some(a) = &p.array_id {
        let id = resolve(&store, a)?;
        let e = store.get(id).expect("resolved");
        array_recipe(e).ok_or_else(|| bad("unit.not_array", format!("element {id} is not a unit array")))?;
        let g = e.guid().to_string();
        copies.extend(store.iter().filter(|c| copy_info(c).is_some_and(|i| i.array == g)).map(Element::id));
        array_guid = Some(g);
    }
    let spines: Vec<(ElementId, crate::mep_runs::SegmentRecipe)> = p
        .spines
        .iter()
        .map(|r| {
            let id = resolve(&store, r)?;
            let s = store.get(id).and_then(crate::mep_runs::segment_recipe).ok_or_else(|| bad("unit.spine", format!("element {id} is not a duct or pipe")))?;
            Ok((id, s))
        })
        .collect::<Result<_>>()?;
    if spines.is_empty() {
        return Err(bad("unit.spine", "choose the mains to connect to"));
    }
    let mut out = Connected { branches: 0, unconnected: Vec::new(), spines: spines.iter().map(|(id, _)| SpineLoad { spine: *id, branches: 0, flow: 0.0 }).collect() };
    // Each branch splits its main at the tee: the main's pieces, by spine.
    let mut pieces: Vec<(ElementId, usize)> = spines.iter().enumerate().map(|(k, (id, _))| (*id, k)).collect();
    for copy in copies {
        let Some(info) = txn.store().get(copy).and_then(copy_info) else { continue };
        let tag = array_guid.clone().unwrap_or_else(|| info.array.clone());
        for m in members(txn.store(), copy) {
            // Segments of the unit itself are not terminals.
            if txn.store().get(m).and_then(crate::mep_runs::segment_recipe).is_some() {
                continue;
            }
            let ports: Vec<crate::mep::Port> = {
                let s = txn.store();
                let partners = crate::mep::partners(s);
                s.get(m).map(|e| crate::mep::ports(s, e)).unwrap_or_default().into_iter().filter(|p| matches!(p.connector.domain, crate::mep::Domain::Duct | crate::mep::Domain::Pipe) && !partners.contains_key(&(m, p.index))).collect()
            };
            for port in ports {
                let class = port.connector.classification;
                // The pieces of mains of its system on its level, nearest first.
                let mut near: Vec<(f64, ElementId, usize)> = pieces
                    .iter()
                    .filter_map(|(id, k)| {
                        let s = txn.store().get(*id).and_then(crate::mep_runs::segment_recipe)?;
                        if s.domain != port.connector.domain || s.level != info.level {
                            return None;
                        }
                        if let (Some(a), Some(b)) = (class, s.classification)
                            && a != b
                            && !matches!(b, crate::mep::Classification::Global)
                        {
                            return None;
                        }
                        Some((distance_to(port.position, s.start, s.end), *id, *k))
                    })
                    .collect();
                near.sort_by(|a, b| a.0.total_cmp(&b.0));
                if near.is_empty() {
                    out.unconnected.push(Unconnected { number: info.number.clone(), element: m, connector: port.connector.name.clone(), reason: "no main of its system on its level".into() });
                    continue;
                }
                let mut why = String::new();
                let mut done = false;
                for (_, target, k) in near {
                    match crate::mep_equipment::connect_into(txn, crate::mep_equipment::ConnectIntoParams { element_id: ElementRef::Id(m), connector_index: port.index, target_id: ElementRef::Id(target) }) {
                        Ok(run) => {
                            let (a, b) = (spines[k].1.start, spines[k].1.end);
                            for id in &run.segments {
                                let Some(s) = txn.store().get(*id).and_then(crate::mep_runs::segment_recipe) else { continue };
                                if distance_to_line(s.start, a, b) < 1e-3 && distance_to_line(s.end, a, b) < 1e-3 {
                                    pieces.push((*id, k));
                                } else {
                                    txn.modify(*id, |e| {
                                        e.params.insert(BRANCH_KEY.into(), Value::from(tag.as_str()));
                                    })?;
                                }
                            }
                            out.branches += 1;
                            out.spines[k].branches += 1;
                            out.spines[k].flow += port.connector.flow;
                            done = true;
                            break;
                        }
                        Err(e) => why = e.message.clone(),
                    }
                }
                if !done {
                    out.unconnected.push(Unconnected { number: info.number.clone(), element: m, connector: port.connector.name.clone(), reason: why });
                }
            }
        }
    }
    Ok(out)
}

/// The distance from a point to the infinite line through `a` and `b`.
fn distance_to_line(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l2 = ab.iter().map(|v| v * v).sum::<f64>().max(1e-12);
    let t = ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1] + (p[2] - a[2]) * ab[2]) / l2;
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}

fn distance_to(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l2 = ab.iter().map(|v| v * v).sum::<f64>().max(1e-12);
    let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1] + (p[2] - a[2]) * ab[2]) / l2).clamp(0.0, 1.0);
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}
