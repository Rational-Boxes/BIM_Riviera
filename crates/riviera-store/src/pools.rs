// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Swimming pools in IFC (ARC-POOL-001, the owner's answer to `POOLS_PLAN.md` Question 1): the
//! pool is an `IfcElementAssembly` (SWIMMINGPOOL) contained in its level, aggregating its shell's
//! `IfcWall` (SOLIDWALL) and `IfcSlab` (BASESLAB) and its coping's `IfcCovering` (COPING), each a
//! faceted B-rep. Its water is an `IfcSpace` (USERDEFINED, WATER) under the level, with its area
//! and volume in `Qto_SpaceBaseQuantities`.
//!
//! The parts are made from the pool: they are not elements of their own. The aggregations are
//! marked `rxd:pool:<signature>` (the water's `rxd:pool:water:<pool GlobalId>`) and each part
//! `rxd:pool`, so an unchanged pool is left as it is, a changed one gets new parts, and opening
//! the file does not load them as elements.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use riviera_core::pools::{self, PoolGeometry};
use riviera_core::{Document, ElementId, GlobalId};
use riviera_ifc::{AttrValue, IfcModel, StepId};

use crate::{Result, geometry};

pub(crate) const PART_TAG: &str = "rxd:pool";

fn r(id: StepId) -> AttrValue {
    AttrValue::Ref(id.0)
}

fn signature(g: &PoolGeometry, scale: f64) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for set in [&g.walls, &g.base, &g.coping, &g.water] {
        set.len().hash(&mut h);
        for (p, i) in set {
            for q in p {
                for v in q {
                    v.to_bits().hash(&mut h);
                }
            }
            i.hash(&mut h);
        }
    }
    scale.to_bits().hash(&mut h);
    format!("{:016x}", h.finish())
}

fn solids(meshes: &[riviera_core::bodies::Mesh]) -> Vec<geometry::Polyhedron> {
    meshes
        .iter()
        .map(|(points, idx)| geometry::Polyhedron { points: points.clone(), faces: idx.chunks(3).map(|t| t.iter().map(|i| *i as usize).collect()).collect() })
        .collect()
}

/// Our aggregations of a pool: its parts' (on the assembly) and its water's (on the level).
fn our_rels(ifc: &IfcModel, whole: StepId, guid: &str) -> Result<Vec<(StepId, String)>> {
    let water = format!("{PART_TAG}:water:{guid}");
    let mut out = Vec::new();
    for rel in ifc.entities_of_type("IfcRelAggregates")?.into_iter().map(|e| e.id) {
        let AttrValue::String(d) = ifc.attribute(rel, "Description")? else { continue };
        let on_whole = ifc.attribute(rel, "RelatingObject")? == AttrValue::Ref(whole.0);
        if (on_whole && d.starts_with(PART_TAG) && !d.starts_with(&format!("{PART_TAG}:water:"))) || d == water {
            out.push((rel, d));
        }
    }
    Ok(out)
}

/// Removes a pool's parts, its water and their aggregations.
pub(crate) fn remove_parts(ifc: &mut IfcModel, whole: StepId, guid: &str) -> Result<()> {
    for (rel, _) in our_rels(ifc, whole, guid)? {
        if let AttrValue::List(parts) = ifc.attribute(rel, "RelatedObjects")? {
            for p in parts {
                let AttrValue::Ref(p) = p else { continue };
                let p = StepId(p);
                for q in ifc.referenced_by(p, "IfcRelDefinesByProperties")? {
                    if let AttrValue::Ref(set) = ifc.attribute(q, "RelatingPropertyDefinition")? {
                        if let AttrValue::List(qs) = ifc.attribute(StepId(set), "Quantities")? {
                            for x in qs {
                                if let AttrValue::Ref(x) = x {
                                    ifc.remove(StepId(x))?;
                                }
                            }
                        }
                        ifc.remove(StepId(set))?;
                    }
                    ifc.remove(q)?;
                }
                geometry::remove_breps(ifc, p)?;
                ifc.remove(p)?;
            }
        }
        ifc.remove(rel)?;
    }
    Ok(())
}

fn part(ifc: &mut IfcModel, class: &str, name: String, predefined: &str, object_type: Option<&str>, owner_history: Option<StepId>) -> Result<StepId> {
    let e = ifc.create_rooted(class, &GlobalId::new())?;
    ifc.set_attribute(e, "Name", &AttrValue::String(name))?;
    ifc.set_attribute(e, "Description", &AttrValue::String(PART_TAG.into()))?;
    ifc.set_attribute(e, "PredefinedType", &AttrValue::Enum(predefined.into()))?;
    if let Some(t) = object_type {
        ifc.set_attribute(e, "ObjectType", &AttrValue::String(t.into()))?;
    }
    if let Some(oh) = owner_history {
        ifc.set_attribute(e, "OwnerHistory", &r(oh))?;
    }
    Ok(e)
}

fn aggregate(ifc: &mut IfcModel, whole: StepId, parts: &[StepId], tag: String, owner_history: Option<StepId>) -> Result<()> {
    let rel = ifc.create_rooted("IfcRelAggregates", &GlobalId::new())?;
    ifc.set_attribute(rel, "Description", &AttrValue::String(tag))?;
    if let Some(oh) = owner_history {
        ifc.set_attribute(rel, "OwnerHistory", &r(oh))?;
    }
    ifc.set_attribute(rel, "RelatingObject", &r(whole))?;
    ifc.set_attribute(rel, "RelatedObjects", &AttrValue::List(parts.iter().map(|p| r(*p)).collect()))?;
    Ok(())
}

/// The water's area and volume (`Qto_SpaceBaseQuantities`), in file units.
fn quantities(ifc: &mut IfcModel, space: StepId, area: f64, volume: f64, scale: f64, owner_history: Option<StepId>) -> Result<()> {
    let a = ifc.create("IfcQuantityArea")?;
    ifc.set_attribute(a, "Name", &AttrValue::String("NetFloorArea".into()))?;
    ifc.set_attribute(a, "AreaValue", &AttrValue::Real(area / (scale * scale)))?;
    let v = ifc.create("IfcQuantityVolume")?;
    ifc.set_attribute(v, "Name", &AttrValue::String("NetVolume".into()))?;
    ifc.set_attribute(v, "VolumeValue", &AttrValue::Real(volume / (scale * scale * scale)))?;
    let set = ifc.create_rooted("IfcElementQuantity", &GlobalId::new())?;
    ifc.set_attribute(set, "Name", &AttrValue::String("Qto_SpaceBaseQuantities".into()))?;
    ifc.set_attribute(set, "Quantities", &AttrValue::List(vec![r(a), r(v)]))?;
    let rel = ifc.create_rooted("IfcRelDefinesByProperties", &GlobalId::new())?;
    if let Some(oh) = owner_history {
        ifc.set_attribute(set, "OwnerHistory", &r(oh))?;
        ifc.set_attribute(rel, "OwnerHistory", &r(oh))?;
    }
    ifc.set_attribute(rel, "RelatedObjects", &AttrValue::List(vec![r(space)]))?;
    ifc.set_attribute(rel, "RelatingPropertyDefinition", &r(set))?;
    Ok(())
}

/// Writes every pool's parts and water, leaving unchanged pools as they are, and contains each
/// pool in its level.
pub(crate) fn write_pools(ifc: &mut IfcModel, doc: &Document, steps: &HashMap<ElementId, StepId>, owner_history: Option<StepId>, scale: f64) -> Result<()> {
    let store = doc.store();
    let all = pools::all(store);
    if all.is_empty() {
        return Ok(());
    }
    let contexts = geometry::Contexts::find(ifc)?;
    for (id, g) in all {
        let Some(&step) = steps.get(&id) else { continue };
        let e = store.get(id).expect("listed");
        let guid = e.guid().as_str().to_owned();
        let Some(storey) = pools::recipe(e).and_then(|rec| GlobalId::parse(&rec.level)).and_then(|l| store.find_by_guid(&l)).and_then(|l| steps.get(&l).copied()) else { continue };
        geometry::contain(ifc, step, storey, owner_history)?;
        let tag = format!("{PART_TAG}:{}", signature(&g, scale));
        if our_rels(ifc, step, &guid)?.iter().any(|(_, d)| *d == tag) {
            continue;
        }
        remove_parts(ifc, step, &guid)?;
        // The assembly sits at the origin; its parts are placed absolutely.
        if matches!(ifc.attribute(step, "ObjectPlacement")?, AttrValue::Null) {
            let origin = ifc.create("IfcCartesianPoint")?;
            ifc.set_attribute(origin, "Coordinates", &AttrValue::List(vec![AttrValue::Real(0.0); 3]))?;
            let axes = ifc.create("IfcAxis2Placement3D")?;
            ifc.set_attribute(axes, "Location", &r(origin))?;
            let placement = ifc.create("IfcLocalPlacement")?;
            ifc.set_attribute(placement, "RelativePlacement", &r(axes))?;
            ifc.set_attribute(step, "ObjectPlacement", &r(placement))?;
        }
        let name = e.name.clone().unwrap_or_else(|| "Swimming pool".into());
        let mut parts = Vec::new();
        for (class, label, predefined, object_type, meshes) in [
            ("IfcWall", "shell walls", "SOLIDWALL", None, &g.walls),
            ("IfcSlab", "base slab", "BASESLAB", None, &g.base),
            ("IfcCovering", "coping", "USERDEFINED", Some("COPING"), &g.coping),
        ] {
            if meshes.is_empty() {
                continue;
            }
            let p = part(ifc, class, format!("{name} {label}"), predefined, object_type, owner_history)?;
            geometry::write_breps(ifc, p, geometry::BrepFrame::WORLD, &solids(meshes), scale, contexts)?;
            parts.push(p);
        }
        if !parts.is_empty() {
            aggregate(ifc, step, &parts, tag, owner_history)?;
        }
        if !g.water.is_empty() {
            let w = part(ifc, "IfcSpace", format!("{name} water"), "USERDEFINED", Some("WATER"), owner_history)?;
            ifc.set_attribute(w, "CompositionType", &AttrValue::Enum("ELEMENT".into()))?;
            geometry::write_breps(ifc, w, geometry::BrepFrame::WORLD, &solids(&g.water), scale, contexts)?;
            quantities(ifc, w, g.area, g.volume, scale, owner_history)?;
            aggregate(ifc, storey, &[w], format!("{PART_TAG}:water:{guid}"), owner_history)?;
        }
    }
    Ok(())
}
