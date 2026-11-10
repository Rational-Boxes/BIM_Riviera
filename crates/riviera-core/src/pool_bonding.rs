// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A pool's electrical zones and bonding (ARC-POOL-010, `POOLS_PLAN.md` §6, research/23 C.5 and
//! A.16). Both bases come from public summaries of the codes (secondary): check each finding
//! against the edition that applies.
//!
//! - **NEC Article 680** (US): receptacles 6 ft (1.83 m) from the inside wall, GFCI within 20 ft
//!   (6.1 m); luminaires over the pool or within 5 ft (1.52 m) at least 12 ft (3.66 m) over the
//!   water (7 ft 6 in, 2.29 m, indoors with GFCI); 5–10 ft away, 5 ft up or GFCI; switches and
//!   disconnects 5 ft away; underwater lenses 18 in (457 mm) below the water, GFCI over the
//!   15 V low-voltage contact limit. The equipotential bonding of 680.26: an 8 AWG copper grid
//!   on a 12 in square under the perimeter surface 3 ft (0.91 m) wide, or a ring 18–24 in out,
//!   tied to the shell's steel at four points at least, and 9 in² (5800 mm²) touching the water.
//! - **IEC 60364-7-702 / BS 7671 Section 702**: zone 0 the water; zone 1 2.0 m from the rim and
//!   2.5 m up; zone 2 1.5 m beyond; no sockets, switches or switchgear in zones 0 and 1, none
//!   but those an RCD or SELV protects in zone 2, SELV at 12 V a.c. in zone 0 its source outside
//!   the zones, IPX8, IPX4 and IPX2 (IPX4 outdoors) by zone, and the extraneous-conductive parts
//!   in all three bonded.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::mesh_objects::Category;
use crate::pools::{self, PoolFinding};
use crate::rules::Confidence;
use crate::store::Store;

const FT: f64 = 0.3048;

/// Whose rules apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// NFPA 70 Article 680.
    Nec,
    /// IEC 60364-7-702 (BS 7671 Section 702).
    Iec,
}

impl Basis {
    fn pack(self) -> &'static str {
        match self {
            Basis::Nec => "nec_680",
            Basis::Iec => "iec_60364_7_702",
        }
    }
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BondingParams {
    /// One pool; every pool when omitted.
    pub pool_id: Option<ElementRef>,
    /// Default: IEC under the UK, German or Australian code setting, else the NEC.
    pub basis: Option<Basis>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ZoneRing {
    pub name: String,
    /// How far out from the water's edge (m), and how high over the deck (m) it reaches.
    pub distance: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// Its outer edge in plan.
    pub ring: Vec<[f64; 2]>,
}

/// What an electrical element near the pool is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Receptacle,
    Switch,
    Luminaire,
    UnderwaterLuminaire,
    Switchgear,
    Motor,
    Other,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Device {
    pub element: ElementId,
    pub name: String,
    pub kind: DeviceKind,
    /// Plan distance from the water's edge (0 over the water), m.
    pub distance: f64,
    /// Its lowest point over the water and over the deck, m.
    pub above_water: f64,
    pub above_deck: f64,
    /// Its supply's highest voltage, if it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voltage: Option<f64>,
    /// IEC: 0, 1 or 2; none outside them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<u8>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Bonded {
    pub element: ElementId,
    pub name: String,
    pub what: String,
    pub distance: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Grid {
    /// m², the perimeter surface 3 ft wide that the grid lies under.
    pub perimeter_surface: f64,
    /// m of 8 AWG copper for a 12 in grid under it (both ways).
    pub grid_conductor: f64,
    /// m, the unpaved option's ring 18–24 in out (at 21 in).
    pub ring_length: f64,
    /// Where to tie the grid to the shell's steel: four points evenly round the pool.
    pub ties: Vec<[f64; 2]>,
    /// mm² of conductive surface in contact with the water.
    pub water_contact: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Bonding {
    pub pool: ElementId,
    pub name: String,
    pub basis: Basis,
    pub indoor: bool,
    pub zones: Vec<ZoneRing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid: Option<Grid>,
    /// Metal parts and equipment to bond.
    pub bond: Vec<Bonded>,
    pub devices: Vec<Device>,
    pub findings: Vec<PoolFinding>,
    pub notes: Vec<String>,
}

/// Points evenly round a ring by its length.
fn round(r: &[[f64; 2]], n: usize) -> Vec<[f64; 2]> {
    let k = r.len();
    let lens: Vec<f64> = (0..k).map(|i| (r[(i + 1) % k][0] - r[i][0]).hypot(r[(i + 1) % k][1] - r[i][1])).collect();
    let total: f64 = lens.iter().sum();
    (0..n)
        .map(|j| {
            let mut s = total * j as f64 / n as f64;
            for i in 0..k {
                if s <= lens[i] || i == k - 1 {
                    let f = if lens[i] > 1e-12 { (s / lens[i]).min(1.0) } else { 0.0 };
                    let (a, b) = (r[i], r[(i + 1) % k]);
                    return [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f];
                }
                s -= lens[i];
            }
            r[0]
        })
        .collect()
}

fn perimeter(r: &[[f64; 2]]) -> f64 {
    (0..r.len()).map(|i| (r[(i + 1) % r.len()][0] - r[i][0]).hypot(r[(i + 1) % r.len()][1] - r[i][1])).sum()
}

fn kind_of(store: &Store, e: &crate::Element, cat: Category, below_water: bool) -> DeviceKind {
    let ty = crate::mesh_objects::type_of(store, e).map(|(_, t)| t);
    let predefined = ty.as_ref().and_then(|t| t.predefined.clone()).unwrap_or_default();
    let underwater = crate::rules::Selector { object_types: vec!["Underwater Light".into()], ..Default::default() }.selects_element(store, e);
    match cat {
        Category::LightingFixture if underwater || below_water => DeviceKind::UnderwaterLuminaire,
        Category::LightingFixture => DeviceKind::Luminaire,
        Category::ElectricalFixture if predefined == "POWEROUTLET" || (predefined.is_empty() && ty.is_some_and(|t| t.connectors.iter().any(|c| c.electrical.is_some()))) => DeviceKind::Receptacle,
        Category::SwitchingDevice => DeviceKind::Switch,
        Category::ElectricalEquipment | Category::Transformer => DeviceKind::Switchgear,
        Category::Pump | Category::Fan | Category::ElectricAppliance => DeviceKind::Motor,
        _ => DeviceKind::Other,
    }
}

fn m_ft(m: f64) -> String {
    format!("{:.2} m ({:.1} ft)", m, m / FT)
}

/// A pool's electrical zones, the devices in them and its bonding (`pool.bonding`).
pub(crate) fn bonding(store: &Store, p: Option<BondingParams>) -> Result<Vec<Bonding>> {
    use crate::codes::Code;
    let p = p.unwrap_or_default();
    let only = match &p.pool_id {
        Some(r) => Some(store.resolve(r).ok_or_else(|| Error::new("element.not_found", "no such pool"))?),
        None => None,
    };
    let basis = p.basis.unwrap_or(match crate::codes::settings(store).code {
        Code::UkAdk | Code::Din18065 | Code::NccAustralia => Basis::Iec,
        _ => Basis::Nec,
    });
    let mut out = Vec::new();
    for (id, g) in pools::all(store) {
        if only.is_some_and(|o| o != id) {
            continue;
        }
        let e = store.get(id).expect("listed");
        let rec = pools::recipe(e).expect("a pool");
        let indoor = crate::natatorium::hall_of(store, &rec).is_some();
        let mut findings = Vec::new();
        let mut find = |rule: &str, source: &str, error: bool, element: ElementId, message: String| {
            findings.push(PoolFinding {
                pack: basis.pack().into(),
                rule: rule.into(),
                source: source.into(),
                severity: if error { "error" } else { "note" }.into(),
                confidence: Confidence::Secondary,
                pool: id,
                element: Some(element),
                message,
            });
        };
        // Everything electrical within reach: 20 ft for the NEC, zone 2 for the IEC (and its
        // underwater lights).
        let reach = match basis {
            Basis::Nec => 20.0 * FT,
            Basis::Iec => 3.5,
        };
        let mut devices = Vec::new();
        let mut bond = Vec::new();
        for o in store.iter() {
            if o.id() == id {
                continue;
            }
            let Some((type_id, ty)) = crate::mesh_objects::type_of(store, o) else { continue };
            let Some(it) = crate::rules::item(store, o) else { continue };
            let c = it.frame.apply([(it.x[0] + it.x[1]) / 2.0, (it.front + it.back) / 2.0, 0.0]);
            let at = [c[0], c[1]];
            let over = crate::sco::inside(at, &rec.outline);
            let distance = if over { 0.0 } else { pools::edge_distance(at, &rec.outline) };
            let equipment = pools::EQUIPMENT.iter().find(|n| **n != "Main Drain" && crate::rules::Selector { object_types: vec![(**n).into()], ..Default::default() }.selects_element(store, o));
            let cat = ty.category;
            let electrical = cat == Category::LightingFixture || cat.is_electrical_device() || cat.is_electrical_equipment() || matches!(cat, Category::Pump | Category::Fan | Category::ElectricAppliance);
            if let Some(what) = equipment
                && (it.z[0] < g.deck_z + 3.66)
                && match basis {
                    Basis::Nec => distance <= 5.0 * FT,
                    Basis::Iec => distance <= 3.5 && it.z[0] < g.deck_z + 2.5,
                }
            {
                bond.push(Bonded { element: o.id(), name: o.name.clone().unwrap_or_default(), what: (*what).into(), distance });
            }
            if !electrical || distance > reach || it.z[1] < g.water_z - 6.0 || it.z[0] > g.deck_z + 6.0 {
                continue;
            }
            let kind = kind_of(store, o, cat, over && it.z[1] < g.water_z);
            let voltage = crate::mep::ports(store, o).iter().filter_map(|q| q.connector.electrical.as_ref().map(|el| el.voltage)).fold(None, |a: Option<f64>, v| Some(a.map_or(v, |a| a.max(v))));
            let above_water = it.z[0] - g.water_z;
            let above_deck = it.z[0] - g.deck_z;
            let zone = (basis == Basis::Iec).then(|| {
                if (over || distance < 0.05) && it.z[0] < g.water_z {
                    Some(0)
                } else if above_deck < 2.5 && distance <= 2.0 {
                    Some(1)
                } else if above_deck < 2.5 && distance <= 3.5 {
                    Some(2)
                } else {
                    None
                }
            });
            let zone = zone.flatten();
            let name = o.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("{} {}", store.get(type_id).and_then(|t| t.name.clone()).unwrap_or_else(|| cat.label().to_string()), o.id()));
            if matches!(kind, DeviceKind::Switchgear | DeviceKind::Motor) && distance <= 5.0 * FT && basis == Basis::Nec {
                bond.push(Bonded { element: o.id(), name: name.clone(), what: if kind == DeviceKind::Motor { "Pump or motor" } else { "Electrical equipment" }.into(), distance });
            }
            match basis {
                Basis::Nec => match kind {
                    DeviceKind::Receptacle if distance < 6.0 * FT - 1e-6 => find("receptacle_distance", "NEC 680.22(A)(3)", true, o.id(), format!("{name} is {} from the inside wall: receptacles at least 6 ft (1.83 m) from it.", m_ft(distance))),
                    DeviceKind::Receptacle => find("receptacle_gfci", "NEC 680.22(A)(4)", false, o.id(), format!("{name}, {} from the inside wall, needs GFCI protection (within 20 ft, 6.1 m).", m_ft(distance))),
                    DeviceKind::Switch if distance < 5.0 * FT - 1e-6 => find("switch_distance", "NEC 680.22(C)", true, o.id(), format!("{name} is {} from the inside wall: switching devices at least 5 ft (1.52 m) from it unless a permanent barrier separates them.", m_ft(distance))),
                    DeviceKind::Switchgear if distance < 5.0 * FT - 1e-6 => find("disconnect_distance", "NEC 680.13", true, o.id(), format!("{name} is {} from the water: switchgear and disconnects at least 5 ft (1.52 m) from it unless a barrier intervenes.", m_ft(distance))),
                    DeviceKind::Luminaire if distance < 5.0 * FT - 1e-6 => {
                        let (need, how) = if indoor { (7.5 * FT, "indoors with GFCI protection, 7 ft 6 in (2.29 m), 680.22(B)(2)") } else { (12.0 * FT, "12 ft (3.66 m)") };
                        if above_water < need - 1e-6 {
                            find("luminaire_height", "NEC 680.22(B)", true, o.id(), format!("{name}, {} from the inside wall, is {} over the water: over the pool or within 5 ft of it, at least {how}.", m_ft(distance), m_ft(above_water)));
                        } else if indoor && above_water < 12.0 * FT - 1e-6 {
                            find("luminaire_gfci", "NEC 680.22(B)(2)", false, o.id(), format!("{name} is {} over the water: under 12 ft indoors it needs GFCI protection and a totally enclosed fitting.", m_ft(above_water)));
                        }
                    }
                    DeviceKind::Luminaire if distance < 10.0 * FT - 1e-6 && above_water < 5.0 * FT - 1e-6 => find("luminaire_gfci", "NEC 680.22(B)(4)", false, o.id(), format!("{name}, {} from the inside wall and {} up, needs GFCI protection (5–10 ft away and under 5 ft above the water).", m_ft(distance), m_ft(above_water))),
                    DeviceKind::UnderwaterLuminaire => {
                        let lens = g.water_z - it.z[1];
                        if lens < 18.0 * 0.0254 - 1e-6 {
                            find("underwater_lens_depth", "NEC 680.23(A)(5)", true, o.id(), format!("{name}'s lens top is {:.0} mm ({:.1} in) below the water: at least 18 in (457 mm) unless listed for less.", lens * 1000.0, lens / 0.0254));
                        }
                        if voltage.is_some_and(|v| v > 15.0) {
                            find("underwater_gfci", "NEC 680.23(A)(3)", false, o.id(), format!("{name} runs at {:.0} V, over the 15 V low-voltage contact limit: GFCI protection on its branch circuit.", voltage.unwrap_or_default()));
                        }
                    }
                    DeviceKind::Motor => find("motor_gfci", "NEC 680.21", false, o.id(), format!("{name}: a pool pump motor is GFCI protected, with an insulated copper equipment grounding conductor of 12 AWG or more.")),
                    _ => {}
                },
                Basis::Iec => match (kind, zone) {
                    (_, None) => {}
                    (DeviceKind::Receptacle | DeviceKind::Switch, Some(z @ (0 | 1))) => find("zone_switches", "IEC 60364-7-702.53", true, o.id(), format!("{name} is in zone {z} ({} from the rim, {} up): no socket outlets or switches in zones 0 and 1 (in zone 1 only where there is no zone 2, 1.25 m from zone 0 and 0.3 m up, protected).", m_ft(distance), m_ft(above_deck))),
                    (DeviceKind::Receptacle | DeviceKind::Switch, Some(_)) => find("zone2_protection", "IEC 60364-7-702.53", false, o.id(), format!("{name} is in zone 2: protect it with a 30 mA RCD, SELV or electrical separation; IPX2 indoors, IPX4 outdoors.")),
                    (DeviceKind::Switchgear, Some(z)) => find("zone_switchgear", "IEC 60364-7-702.53", true, o.id(), format!("{name} is in zone {z}: no switchgear, controlgear or junction boxes in zones 0–2 but SELV's.")),
                    (DeviceKind::Luminaire | DeviceKind::UnderwaterLuminaire | DeviceKind::Other | DeviceKind::Motor, Some(0)) => {
                        if voltage.is_some_and(|v| v > 12.0) {
                            find("zone0_selv", "IEC 60364-7-702.414", true, o.id(), format!("{name} runs at {:.0} V in zone 0: SELV at 12 V a.c. (30 V ripple-free d.c.) at most, its source outside zones 0–2.", voltage.unwrap_or_default()));
                        } else {
                            find("zone0_ip", "IEC 60364-7-702.512.2", false, o.id(), format!("{name} is in zone 0: IPX8, SELV at 12 V a.c., its transformer outside zones 0–2."));
                        }
                    }
                    (_, Some(1)) => find("zone1_protection", "IEC 60364-7-702.410.3", false, o.id(), format!("{name} is in zone 1 ({} from the rim, {} up): SELV at 12 V a.c. (30 V d.c.), its source outside zones 0–2; IPX4 (IPX5 where jets clean).", m_ft(distance), m_ft(above_deck))),
                    (_, Some(_)) => find("zone2_protection", "IEC 60364-7-702.410.3", false, o.id(), format!("{name} is in zone 2: SELV, a 30 mA RCD or electrical separation; IPX2 indoors, IPX4 outdoors.")),
                },
            }
            devices.push(Device { element: o.id(), name, kind, distance, above_water, above_deck, voltage, zone });
        }
        let mut notes = Vec::new();
        let zones = match basis {
            Basis::Nec => vec![
                ZoneRing { name: "Perimeter surface (680.26(B)(2))".into(), distance: 3.0 * FT, height: None, ring: pools::offset(&rec.outline, 3.0 * FT) },
                ZoneRing { name: "Switches, disconnects, luminaire heights (680.22(B)(1), (C))".into(), distance: 5.0 * FT, height: Some(12.0 * FT + rec.water_level), ring: pools::offset(&rec.outline, 5.0 * FT) },
                ZoneRing { name: "Receptacles kept out (680.22(A)(3))".into(), distance: 6.0 * FT, height: None, ring: pools::offset(&rec.outline, 6.0 * FT) },
                ZoneRing { name: "GFCI on receptacles (680.22(A)(4))".into(), distance: 20.0 * FT, height: None, ring: pools::offset(&rec.outline, 20.0 * FT) },
            ],
            Basis::Iec => vec![
                ZoneRing { name: "Zone 1".into(), distance: 2.0, height: Some(2.5), ring: pools::offset(&rec.outline, 2.0) },
                ZoneRing { name: "Zone 2".into(), distance: 3.5, height: Some(2.5), ring: pools::offset(&rec.outline, 3.5) },
            ],
        };
        let grid = (basis == Basis::Nec).then(|| {
            let surface = crate::polygons::area(&pools::offset(&rec.outline, 3.0 * FT)).abs() - g.area;
            Grid {
                perimeter_surface: surface,
                grid_conductor: 2.0 * surface / FT,
                ring_length: perimeter(&pools::offset(&rec.outline, 21.0 * 0.0254)),
                ties: round(&rec.outline, 4),
                water_contact: 5800.0,
            }
        });
        match basis {
            Basis::Nec => {
                notes.push("Bond the shell's reinforcing steel, the perimeter surface, metal fittings 4 in (100 mm) or larger, pump motors and luminaire shells with solid copper 8 AWG or larger (680.26(B)); metal within 5 ft horizontally and 12 ft vertically of the water too (680.26(B)(7)).".into());
                if rec.class() == pools::PoolClass::Residential && !devices.iter().any(|d| d.kind == DeviceKind::Receptacle && (6.0 * FT - 1e-6..=20.0 * FT).contains(&d.distance)) {
                    notes.push("A dwelling's pool needs a 125 V 15 or 20 A receptacle 6–20 ft (1.83–6.1 m) from the inside wall and no more than 6 ft 6 in (1.98 m) up (680.22(A)(1)); none is modelled.".into());
                }
                notes.push("Overhead conductors 22.5 ft (6.9 m) over the water and 10 ft (3.05 m) round it; no underground wiring within 5 ft of the inside wall but by the approved methods (680.9, 680.11).".into());
            }
            Basis::Iec => {
                notes.push("Bond every extraneous-conductive part in zones 0, 1 and 2 to the protective conductors of the exposed-conductive-parts there (702.415.2).".into());
                notes.push("No zone 2 is drawn for fountains; where a fixed partition 2.5 m high bounds zone 1, the zones stop at it.".into());
            }
        }
        notes.push("From public summaries of the code (secondary): check each distance against the edition the project is under.".into());
        out.push(Bonding { pool: id, name: e.name.clone().unwrap_or_default(), basis, indoor, zones, grid, bond, devices, findings, notes });
    }
    Ok(out)
}
