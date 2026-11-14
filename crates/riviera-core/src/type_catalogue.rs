// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Type catalogues (DM-TYPE-002): the types an element could take besides those already in the
//! model, so changing a type is not limited to the few a project starts with.
//!
//! - **Starter types** Riviera defines here:
//!   - doors: single, double, glass, sliding automatic, revolving and round-top;
//!   - windows in common sizes, round and round-top;
//!   - roofs, floors, ceilings and walls by common build-up, with their layers;
//!   - ramps at 1:12, 1:15 and 1:20 (footings and wall foundations list the model's only).
//! - **Components:** the bundled component libraries (furnishings, bathroom components, MEP
//!   starters) of the element's category.
//!
//! `type.catalogue` lists them for an element (or a type), marking those already loaded;
//! `type.load` loads one by name (a type of that name and family already in the model is reused)
//! and returns it, for `element.set_type`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::element::{Element, NewElement};
use crate::error::{Error, Result};
use crate::floors::SlabKind;
use crate::id::{ElementId, ElementRef};
use crate::inserts::{self, HeadProfile, InsertType, Kind, Shape};
use crate::inserts::doors::{Operation, Panel};
use crate::mesh_objects::{self, Category};
use crate::store::Store;
use crate::txn::Transaction;
use crate::walls::RECIPE_KEY;

/// What a type is for: the families a catalogue is kept by.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Family {
    Door,
    Window,
    Roof,
    Slab(SlabKind),
    Wall,
    Ramp,
    Footing,
    WallFoundation,
    Railing,
    /// Duct, pipe, tray and conduit runs: their domain, shape and whether flexible.
    Segment(crate::mep::Domain, crate::mep::Shape, bool),
    Component(Category),
}

/// The family of an element or a type.
fn family_of(store: &Store, e: &Element) -> Option<Family> {
    if let Some(k) = inserts::kind_of(e).or_else(|| inserts::insert_type(e).and_then(|t| t.kind())) {
        return Some(match k {
            Kind::Door => Family::Door,
            Kind::Window => Family::Window,
        });
    }
    if crate::roofs::roof_recipe(e).is_some() || crate::roofs::is_roof_type(e) {
        return Some(Family::Roof);
    }
    if let Some(k) = crate::floors::slab_recipe(e).map(|(k, _)| k).or_else(|| crate::floors::type_kind(e)) {
        return Some(Family::Slab(k));
    }
    if crate::ramps::ramp_recipe(e).is_some() || crate::ramps::ramp_type(e).is_some() {
        return Some(Family::Ramp);
    }
    if let Some(t) = crate::mep_runs::segment_type(e) {
        return Some(Family::Segment(t.domain, t.shape, t.flexible));
    }
    if let Some(s) = crate::mep_runs::segment_recipe(e) {
        let t = store.find_by_guid(&crate::GlobalId::parse(&s.segment_type)?).and_then(|t| store.get(t)).and_then(crate::mep_runs::segment_type);
        return Some(Family::Segment(s.domain, s.size.shape, t.is_some_and(|t| t.flexible)));
    }
    if crate::railings::railing_recipe(e).is_some() || crate::railings::railing_type(e).is_some() {
        return Some(Family::Railing);
    }
    if crate::foundations::footing_recipe(e).is_some() || crate::foundations::footing_type(e).is_some() {
        return Some(Family::Footing);
    }
    if crate::foundations::wall_foundation_recipe(e).is_some() || crate::foundations::wall_foundation_type(e).is_some() {
        return Some(Family::WallFoundation);
    }
    if let Some(t) = mesh_objects::mesh_object_type(e).or_else(|| mesh_objects::type_of(store, e).map(|(_, t)| t)) {
        return Some(Family::Component(t.category));
    }
    // A basic wall's type (not stacked: those have rows of other types).
    let basic = |t: &Element| crate::walls::type_width(t).is_some() && crate::walls::stacked::of_type(t).is_none();
    if basic(e) {
        return Some(Family::Wall);
    }
    if crate::walls::wall_recipe(e).is_some() {
        let t = e.param("@type").and_then(Value::as_str).and_then(crate::GlobalId::parse).and_then(|g| store.find_by_guid(&g)).and_then(|t| store.get(t));
        if t.is_none_or(basic) {
            return Some(Family::Wall);
        }
    }
    None
}

/// Whether type `t` is of `family`.
fn is_of(store: &Store, t: &Element, family: Family) -> bool {
    crate::categories::is_type(t) && family_of(store, t) == Some(family)
}

// ---- Starter types ----

fn door(width: f64, height: f64, operation: Operation, panel: Panel, automatic: bool) -> InsertType {
    InsertType { width, height, operation, panel, automatic, ..InsertType::default_for(Kind::Door) }
}

fn window(width: f64, height: f64, sill: f64) -> InsertType {
    InsertType { width, height, default_sill: sill, ..InsertType::default_for(Kind::Window) }
}

/// The starter doors and windows, by name.
fn inserts_of(kind: Kind) -> Vec<(&'static str, InsertType)> {
    use Operation::*;
    use Panel::*;
    match kind {
        Kind::Door => vec![
            ("Single Door 800 x 2100", door(0.8, 2.1, SingleSwing, Solid, false)),
            (inserts::DEFAULT_DOOR_TYPE, door(0.9, 2.1, SingleSwing, Solid, false)),
            ("Single Door 1000 x 2100", door(1.0, 2.1, SingleSwing, Solid, false)),
            ("Single Door 900 x 2400", door(0.9, 2.4, SingleSwing, Solid, false)),
            ("Double Door 1500 x 2100", door(1.5, 2.1, DoubleSwing, Solid, false)),
            ("Double Door 1800 x 2100", door(1.8, 2.1, DoubleSwing, Solid, false)),
            ("Glass Door 900 x 2100", door(0.9, 2.1, SingleSwing, Glass, false)),
            ("Glass Double Door 1800 x 2400", door(1.8, 2.4, DoubleSwing, Glass, false)),
            ("Sliding Door 1800 x 2100, automatic", door(1.8, 2.1, Sliding, Glass, true)),
            ("Revolving Door 2100 x 2400", door(2.1, 2.4, Revolving, Glass, false)),
            ("Round-Top Door 900 x 2400", InsertType { shape: Shape::RoundTop, head_profile: HeadProfile::Semicircular, ..door(0.9, 2.4, SingleSwing, Solid, false) }),
        ],
        Kind::Window => vec![
            ("Window 600 x 600", window(0.6, 0.6, 1.5)),
            ("Window 600 x 1200", window(0.6, 1.2, 0.9)),
            ("Window 900 x 1200", window(0.9, 1.2, 0.9)),
            (inserts::DEFAULT_WINDOW_TYPE, window(1.2, 1.2, 0.9)),
            ("Window 1200 x 1500", window(1.2, 1.5, 0.75)),
            ("Window 1800 x 1200", window(1.8, 1.2, 0.9)),
            ("Window 2400 x 1500", window(2.4, 1.5, 0.75)),
            ("Window 1500 x 2400, full height", window(1.5, 2.4, 0.1)),
            ("Round Window 900", InsertType { shape: Shape::Round, ..window(0.9, 0.9, 1.2) }),
            ("Round-Top Window 900 x 1500", InsertType { shape: Shape::RoundTop, head_profile: HeadProfile::Semicircular, ..window(0.9, 1.5, 0.9) }),
        ],
    }
}

/// A layer: function, material, thickness (metres).
type L = (&'static str, &'static str, f64);

/// The starter roofs (thickness only: roof types have no layers yet).
const ROOFS: [(&str, f64); 5] = [
    ("Roof - Metal Deck 150 mm", 0.15),
    ("Roof - Concrete 200 mm", 0.2),
    ("Roof - Timber Rafters 250 mm", 0.25),
    (crate::roofs::DEFAULT_TYPE_NAME, 0.3),
    ("Roof - Insulated Concrete 350 mm", 0.35),
];

/// The starter floors and ceilings, top layer first.
fn slabs_of(kind: SlabKind) -> Vec<(&'static str, Vec<L>)> {
    match kind {
        SlabKind::Floor => vec![
            ("Floor - Concrete 150 mm", vec![("structure", "Concrete", 0.15)]),
            ("Floor - Concrete 200 mm", vec![("structure", "Concrete", 0.2)]),
            (crate::floors::DEFAULT_TYPE_NAME, vec![("structure", "Concrete", 0.3)]),
            ("Floor - Tile on Screed and Concrete 210 mm", vec![("finish1", "Ceramic Tile", 0.01), ("substrate", "Sand-Cement Screed", 0.05), ("structure", "Concrete", 0.15)]),
            ("Floor - Composite Metal Deck 150 mm", vec![("structure", "Concrete", 0.09), ("structure", "Steel Deck", 0.06)]),
            ("Floor - Timber Joists 290 mm", vec![("finish1", "Timber Flooring", 0.022), ("structure", "Timber Joists", 0.25), ("finish2", "Gypsum Board", 0.018)]),
        ],
        SlabKind::Ceiling => vec![
            ("Ceiling - Gypsum Board 13 mm", vec![("finish2", "Gypsum Board", 0.013)]),
            ("Ceiling - Acoustic Tile 600 x 600, 20 mm", vec![("finish2", "Acoustic Ceiling Tile", 0.02)]),
            (crate::floors::DEFAULT_CEILING_TYPE_NAME, vec![("finish2", "Plaster", 0.025)]),
            ("Ceiling - Timber Slats 40 mm", vec![("finish2", "Timber Slats", 0.04)]),
        ],
    }
}

/// The starter walls, exterior layer first.
fn walls() -> Vec<(&'static str, Vec<L>)> {
    vec![
        (crate::walls::DEFAULT_TYPE_NAME, vec![("structure", "", 0.2)]),
        ("Interior - Stud Partition 125 mm", vec![("finish1", "Gypsum Board", 0.0125), ("structure", "Steel Studs", 0.1), ("finish2", "Gypsum Board", 0.0125)]),
        ("Interior - Concrete Block 150 mm", vec![("structure", "Concrete Block", 0.14), ("finish2", "Plaster", 0.01)]),
        ("Concrete 200 mm", vec![("structure", "Concrete", 0.2)]),
        ("Concrete 300 mm", vec![("structure", "Concrete", 0.3)]),
        ("Exterior - Brick and Block Cavity 300 mm", vec![("finish1", "Brick", 0.1), ("thermal_air", "Insulation", 0.075), ("structure", "Concrete Block", 0.11), ("finish2", "Plaster", 0.015)]),
        ("Exterior - Insulated Stud 200 mm", vec![("finish1", "Fibre-Cement Cladding", 0.015), ("membrane", "Weather Barrier", 0.0), ("substrate", "Sheathing", 0.012), ("structure", "Timber Studs with Insulation", 0.16), ("finish2", "Gypsum Board", 0.013)]),
    ]
}

/// The starter railings: height, baluster section and spacing (a 100 mm sphere passes under
/// 120 mm centres of 20 mm balusters, the IBC 1015.4 limit).
const RAILINGS: [(&str, f64, f64, f64); 4] = [
    ("Guard 1067 mm (42 in)", 1.067, 0.02, 0.12),
    ("Guard 1100 mm", 1.1, 0.02, 0.12),
    ("Railing 900 mm", 0.9, 0.02, 0.12),
    ("Pool Fence 1219 mm (48 in)", 1.219, 0.02, 0.12),
];

const RAMPS: [(&str, f64); 3] = [(crate::ramps::DEFAULT_TYPE_NAME, 12.0), ("Ramp 1:15", 15.0), ("Ramp 1:20", 20.0)];

/// One entry: its name and where it comes from.
fn entries_of(family: Family) -> Vec<(String, String)> {
    let starters = |names: Vec<&str>| names.into_iter().map(|n| (n.to_owned(), "Riviera starter types".to_owned())).collect::<Vec<_>>();
    match family {
        Family::Door => starters(inserts_of(Kind::Door).into_iter().map(|x| x.0).collect()),
        Family::Window => starters(inserts_of(Kind::Window).into_iter().map(|x| x.0).collect()),
        Family::Roof => starters(ROOFS.iter().map(|x| x.0).collect()),
        Family::Slab(k) => starters(slabs_of(k).into_iter().map(|x| x.0).collect()),
        Family::Wall => starters(walls().into_iter().map(|x| x.0).collect()),
        Family::Ramp => starters(RAMPS.iter().map(|x| x.0).collect()),
        // Sized to suit: Duplicate Type and its values (no starters).
        Family::Footing | Family::WallFoundation | Family::Segment(..) => Vec::new(),
        Family::Railing => starters(RAILINGS.iter().map(|x| x.0).collect()),
        Family::Component(c) => {
            let mut out: Vec<(String, String)> = crate::furnishings::catalogue().into_iter().filter(|i| i.category == c).map(|i| (i.name.to_owned(), i.library.title().to_owned())).collect();
            out.extend(crate::bath_components::catalogue().into_iter().filter(|i| i.category == c).map(|i| (i.name.to_owned(), "Riviera bathroom components".to_owned())));
            out.extend(crate::mep::starter_categories().into_iter().filter(|(_, k)| *k == c).map(|(n, _)| (n.to_owned(), "Riviera MEP starter types".to_owned())));
            out
        }
    }
}

// ---- Commands ----

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CatalogueParams {
    /// The element (or a type) whose kind of type to list.
    pub element_id: ElementRef,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Entry {
    pub name: String,
    /// Where it comes from: "In the model", "Riviera starter types" or a component library.
    pub source: String,
    /// The type, when it is in the model already.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<ElementId>,
}

fn family(store: &Store, r: &ElementRef) -> Result<Family> {
    let id = store.resolve(r).ok_or_else(|| Error::new("element.not_found", format!("no element {r:?}")))?;
    family_of(store, store.get(id).expect("resolved")).ok_or_else(|| Error::new("type.no_catalogue", format!("element {id} has no type catalogue")))
}

/// `type.catalogue`: the types of an element's kind in the model, then those that can be loaded.
pub fn catalogue(store: &Store, p: CatalogueParams) -> Result<Vec<Entry>> {
    let f = family(store, &p.element_id)?;
    let mut out: Vec<Entry> = store.iter().filter(|t| is_of(store, t, f)).map(|t| Entry { name: t.name.clone().unwrap_or_default(), source: "In the model".into(), type_id: Some(t.id()) }).collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    for (name, source) in entries_of(f) {
        if !out.iter().any(|e| e.name == name) {
            out.push(Entry { name, source, type_id: None });
        }
    }
    Ok(out)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoadParams {
    /// The element (or a type) whose kind of type to load.
    pub element_id: ElementRef,
    /// The catalogue entry's name.
    pub name: String,
}

/// `type.load`: loads a catalogue entry (a type of that name already in the model is reused).
pub(crate) fn load(txn: &mut Transaction<'_>, p: LoadParams) -> Result<ElementId> {
    let store = txn.store().clone();
    let f = family(&store, &p.element_id)?;
    if let Some(t) = store.iter().find(|t| is_of(&store, t, f) && t.name.as_deref() == Some(p.name.as_str())) {
        return Ok(t.id());
    }
    let missing = || Error::new("type.not_in_catalogue", format!("{:?} is not in the catalogue of this kind of type", p.name));
    let layered = |txn: &mut Transaction<'_>, id: ElementId, layers: &[L]| {
        let layers = layers.iter().map(|(f, m, t)| serde_json::from_value(json!({ "function": f, "material": m, "thickness": t, "structural_material": *f == "structure" })).expect("a layer")).collect();
        crate::compound::set_layers(txn, crate::compound::SetLayersParams { type_id: ElementRef::Id(id), layers, core_start: None, core_end: None })?;
        Ok::<_, Error>(id)
    };
    match f {
        Family::Door | Family::Window => {
            let kind = if f == Family::Door { Kind::Door } else { Kind::Window };
            let (name, t) = inserts_of(kind).into_iter().find(|x| x.0 == p.name).ok_or_else(missing)?;
            txn.create(
                NewElement::new(kind.type_class())
                    .name(name)
                    .param("@PredefinedType", if kind == Kind::Door { "DOOR" } else { "WINDOW" })
                    .param(RECIPE_KEY, serde_json::to_value(t).expect("serialises")),
            )
        }
        Family::Roof => {
            let (name, thickness) = ROOFS.iter().find(|x| x.0 == p.name).ok_or_else(missing)?;
            txn.create(NewElement::new(crate::roofs::TYPE_CLASS).name(*name).param("@PredefinedType", "NOTDEFINED").param(RECIPE_KEY, json!({ "kind": crate::roofs::ROOF_TYPE_RECIPE, "thickness": thickness })))
        }
        Family::Slab(k) => {
            let (name, layers) = slabs_of(k).into_iter().find(|x| x.0 == p.name).ok_or_else(missing)?;
            let thickness: f64 = layers.iter().map(|l| l.2).sum();
            let id = txn.create(NewElement::new(k.type_class()).name(name).param("@PredefinedType", k.predefined()).param(RECIPE_KEY, json!({ "kind": k.type_recipe(), "thickness": thickness })))?;
            if layers.len() > 1 || !layers[0].1.is_empty() { layered(txn, id, &layers) } else { Ok(id) }
        }
        Family::Wall => {
            let (name, layers) = walls().into_iter().find(|x| x.0 == p.name).ok_or_else(missing)?;
            let width: f64 = layers.iter().map(|l| l.2).sum();
            let id = txn.create(NewElement::new("IfcWallType").name(name).param("@PredefinedType", "STANDARD").param(RECIPE_KEY, json!({ "kind": crate::walls::WALL_TYPE_RECIPE, "width": width })))?;
            if layers.len() > 1 || !layers[0].1.is_empty() { layered(txn, id, &layers) } else { Ok(id) }
        }
        Family::Ramp => {
            let (name, slope) = RAMPS.iter().find(|x| x.0 == p.name).ok_or_else(missing)?;
            let t = crate::ramps::RampType { max_slope: *slope, ..Default::default() };
            txn.create(NewElement::new("IfcRampType").name(*name).param("@PredefinedType", "STRAIGHT_RUN_RAMP").param(RECIPE_KEY, serde_json::to_value(t).expect("serialises")))
        }
        Family::Footing | Family::WallFoundation | Family::Segment(..) => Err(missing()),
        Family::Railing => {
            let (name, height, baluster, spacing) = RAILINGS.iter().find(|x| x.0 == p.name).ok_or_else(missing)?;
            let t = crate::railings::RailingType { height: *height, baluster: *baluster, baluster_spacing: *spacing, ..Default::default() };
            txn.create(NewElement::new("IfcRailingType").name(*name).param("@PredefinedType", "GUARDRAIL").param(RECIPE_KEY, serde_json::to_value(t).expect("serialises")))
        }
        Family::Component(_) => {
            let names = vec![p.name.clone()];
            let entries = entries_of(f);
            let (_, source) = entries.iter().find(|x| x.0 == p.name).ok_or_else(missing)?;
            let loaded = match source.as_str() {
                "Riviera bathroom components" => crate::bath_components::load(txn, crate::bath_components::LoadParams { names })?,
                "Riviera MEP starter types" => crate::mep::load_starters(txn, crate::mep::LoadStartersParams { names })?,
                _ => crate::furnishings::load(txn, crate::furnishings::LoadParams { names })?,
            };
            loaded.get(&p.name).copied().ok_or_else(missing)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_names_are_unique_per_family() {
        let fams = [Family::Door, Family::Window, Family::Roof, Family::Slab(SlabKind::Floor), Family::Slab(SlabKind::Ceiling), Family::Wall, Family::Ramp, Family::Railing];
        for f in fams {
            let mut n: Vec<String> = entries_of(f).into_iter().map(|e| e.0).collect();
            let all = n.len();
            n.sort();
            n.dedup();
            assert_eq!(n.len(), all, "{f:?}");
        }
        // Layered starters add up to the thickness in their names.
        for (name, layers) in walls().into_iter().chain(slabs_of(SlabKind::Floor)).chain(slabs_of(SlabKind::Ceiling)) {
            let total: f64 = layers.iter().map(|l| l.2).sum();
            let mm = name.rsplit(' ').nth(1).and_then(|s| s.parse::<f64>().ok());
            if let Some(mm) = mm.filter(|_| name.ends_with(" mm")) {
                assert!((total * 1000.0 - mm).abs() < 1.0, "{name}: layers {total}");
            }
        }
    }
}
