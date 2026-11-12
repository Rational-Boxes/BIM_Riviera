// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Disciplines (spec Ch 5, DOC-GFX-021): which discipline each element belongs to, for showing a
//! model from one discipline's point of view with the others ghosted or hidden (the View tab's
//! discipline selector, `view.discipline`).
//!
//! An element's discipline is its category's (architecture, structure, mechanical, plumbing,
//! electrical or site), else its IFC class's; walls, floors, roofs and other building elements
//! marked structural belong to structure. Datums, annotation, views and the like belong to none
//! and are left as they are.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::element::Element;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Discipline {
    Architecture,
    Structure,
    Mechanical,
    Plumbing,
    Electrical,
    Site,
}

impl Discipline {
    pub const ALL: [Discipline; 6] = [Discipline::Architecture, Discipline::Structure, Discipline::Mechanical, Discipline::Plumbing, Discipline::Electrical, Discipline::Site];

    pub fn label(self) -> &'static str {
        match self {
            Discipline::Architecture => "Architecture",
            Discipline::Structure => "Structure",
            Discipline::Mechanical => "Mechanical",
            Discipline::Plumbing => "Plumbing",
            Discipline::Electrical => "Electrical",
            Discipline::Site => "Site",
        }
    }
}

/// A category key's discipline.
fn of_category(key: &str) -> Option<Discipline> {
    use Discipline::*;
    Some(match key {
        k if k.starts_with("cat.structural_") => Structure,
        "cat.walls" | "cat.curtain_walls" | "cat.curtain_panels" | "cat.curtain_mullions" | "cat.floors" | "cat.roofs" | "cat.ceilings" | "cat.doors" | "cat.windows" | "cat.stairs" | "cat.railings" | "cat.ramps" | "cat.columns" | "cat.furniture" | "cat.casework" | "cat.transport" | "cat.generic_models" | "cat.specialty_equipment" | "cat.mass" | "cat.rooms" | "cat.areas" | "cat.parts" | "cat.assemblies" | "cat.growing_systems" | "cat.glazed_enclosures" => Architecture,
        "cat.mechanical_equipment" | "cat.air_terminals" | "cat.air_terminal_boxes" | "cat.ducts" | "cat.duct_fittings" | "cat.fans" | "cat.boilers" | "cat.chillers" | "cat.space_heaters" | "cat.mechanical_control_devices" | "cat.sensors" | "cat.controllers" | "cat.spaces" | "cat.system_zones" => Mechanical,
        "cat.pipes" | "cat.pipe_fittings" | "cat.pipe_accessories" | "cat.plumbing_fixtures" | "cat.pumps" | "cat.waste_terminals" | "cat.sprinklers" | "cat.flow_meters" | "cat.filters" => Plumbing,
        "cat.electrical_equipment" | "cat.electrical_fixtures" | "cat.lighting_fixtures" | "cat.switching_devices" | "cat.alarms" | "cat.transformers" | "cat.cable_trays" | "cat.conduits" | "cat.junction_boxes" => Electrical,
        "cat.toposolid" => Site,
        _ => return None,
    })
}

/// An IFC class's discipline, for elements without a category.
fn of_class(class: &str) -> Option<Discipline> {
    use Discipline::*;
    Some(match class {
        "IfcWall" | "IfcWallStandardCase" | "IfcSlab" | "IfcRoof" | "IfcCovering" | "IfcDoor" | "IfcWindow" | "IfcStair" | "IfcRamp" | "IfcRailing" | "IfcCurtainWall" | "IfcPlate" | "IfcFurniture" | "IfcFurnishingElement" | "IfcBuildingElementProxy" | "IfcElementAssembly" | "IfcShadingDevice" | "IfcTransportElement" | "IfcSpace" | "IfcGroup" => Architecture,
        "IfcColumn" | "IfcBeam" | "IfcMember" | "IfcFooting" | "IfcPile" | "IfcReinforcingBar" | "IfcReinforcingMesh" | "IfcTendon" | "IfcMechanicalFastener" | "IfcFastener" | "IfcDiscreteAccessory" => Structure,
        "IfcDuctSegment" | "IfcDuctFitting" | "IfcAirTerminal" | "IfcAirTerminalBox" | "IfcFan" | "IfcUnitaryEquipment" | "IfcBoiler" | "IfcChiller" | "IfcCoil" | "IfcDamper" | "IfcSpaceHeater" | "IfcHumidifier" | "IfcEvaporativeCooler" | "IfcCooledBeam" => Mechanical,
        "IfcPipeSegment" | "IfcPipeFitting" | "IfcPump" | "IfcSanitaryTerminal" | "IfcWasteTerminal" | "IfcFireSuppressionTerminal" | "IfcValve" | "IfcTank" | "IfcFilter" | "IfcDistributionChamberElement" | "IfcFlowMeter" | "IfcInterceptor" => Plumbing,
        "IfcCableCarrierSegment" | "IfcCableCarrierFitting" | "IfcCableSegment" | "IfcElectricDistributionBoard" | "IfcLightFixture" | "IfcOutlet" | "IfcSwitchingDevice" | "IfcElectricAppliance" | "IfcTransformer" | "IfcAlarm" | "IfcSensor" | "IfcSolarDevice" | "IfcJunctionBox" | "IfcElectricGenerator" => Electrical,
        "IfcGeographicElement" | "IfcAlignment" => Site,
        _ => return None,
    })
}

/// The discipline an element belongs to, if any.
pub fn discipline_of(e: &Element) -> Option<Discipline> {
    if crate::views::is_view(e) || crate::categories::is_type(e) || matches!(e.class.as_str(), "IfcBuildingStorey" | "IfcProject" | "IfcSite" | "IfcBuilding") {
        return None;
    }
    // Building elements marked structural are the structure's.
    if e.param("@LoadBearing").and_then(serde_json::Value::as_bool) == Some(true)
        || e.param("rxd:recipe").and_then(|r| r.get("structural")).and_then(serde_json::Value::as_bool) == Some(true)
    {
        return Some(Discipline::Structure);
    }
    match crate::categories::category_of(e) {
        Some(c) => of_category(c.key).or_else(|| of_class(&e.class)),
        None => of_class(&e.class),
    }
}
