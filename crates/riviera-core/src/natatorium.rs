// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A pool hall's air and a pool's water heat balance (ARC-POOL-008, `POOLS_PLAN.md` §6, research/23
//! C.1–C.2).
//!
//! - **Evaporation** by ASHRAE's equation (Applications, natatoriums, as EnergyPlus and ENERGY
//!   STAR publish it): ṁ = 4.0 × 10⁻⁵ · A · AF · (p_w − p_a) kg/s, A in m², the pressures in kPa:
//!   p_w the saturation pressure at the water's temperature, p_a the air's vapour pressure. The
//!   activity factor AF comes from the pool's class (public 1.0, competition 0.65, residential
//!   0.5); an unoccupied pool's is 0.5, and a cover keeps a fraction of that.
//! - **The latent load** is ṁ · h_fg (2.43 MJ/kg).
//! - **The water's losses:** evaporation; convection to the air, EnergyPlus's
//!   h = 0.22 · |ΔT|^⅓ W/m²K (a gain when the air is warmer); and heating the make-up water for
//!   what evaporates.
//! - **Outdoor air:** ASHRAE 62.1's 0.48 cfm/ft² (2.44 L/s·m²) of water and wet deck (1.5 m round
//!   the edge), plus 7.5 cfm (3.8 L/s) a spectator, as the MAHC adopts them (4.6.2.7.4); a purge
//!   of twice that (4.6.2.7.12); the hall kept negative by exhausting 5 % more (4.6.2.7.8).
//! - **Dew point** of the hall air: surfaces below it condense; the MAHC asks for a vapour
//!   retarder of 0.2 perm or less (4.2.2.2).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::pools::{self, PoolClass, PoolRecipe};
use crate::store::Store;

/// Latent heat of evaporation near pool temperatures (J/kg).
const H_FG: f64 = 2.43e6;
/// Water's specific heat (J/kg·K).
const C_W: f64 = 4186.0;
/// ASHRAE's 0.48 cfm/ft² of water and wet deck (L/s·m²).
const OA_AREA: f64 = 2.44;
/// 7.5 cfm a spectator (L/s).
const OA_PERSON: f64 = 3.8;
/// The wet deck: this far round the water's edge (m).
const WET_DECK: f64 = 1.5;

/// A pool hall's design conditions, on the pool (`pool.set(hall)`); each omitted value is the
/// pool's class's default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Hall {
    /// °C.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub water: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub air: Option<f64>,
    /// Relative humidity, 0–1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rh: Option<f64>,
    /// ASHRAE's activity factor while in use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<f64>,
    /// The fraction of an unoccupied pool's evaporation a cover lets through (a sealed cover
    /// about 0–0.1, a slatted movable floor about 0.3); none when there is no cover.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectators: Option<u32>,
    /// The make-up water's temperature (default 10 °C).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_water: Option<f64>,
}

/// A hall's conditions with every value given.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
pub struct Conditions {
    pub water: f64,
    pub air: f64,
    pub rh: f64,
    pub activity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<f64>,
    pub spectators: u32,
    pub fill_water: f64,
}

impl Hall {
    /// Filled in from the class: water at PWTAG's and ASHRAE's usual temperatures (public 29 °C,
    /// competition 27 °C, residential 28 °C, wading 31 °C), air a degree warmer but not over
    /// 30 °C unless the water is, 55 % RH (the middle of ASHRAE's 50–60 %), and ASHRAE's activity
    /// factors (public and wading 1.0, competition 0.65, residential 0.5).
    pub fn resolve(&self, class: PoolClass) -> Conditions {
        let water = self.water.unwrap_or(match class {
            PoolClass::Public => 29.0,
            PoolClass::Competition => 27.0,
            PoolClass::Residential => 28.0,
            PoolClass::Wading => 31.0,
        });
        Conditions {
            water,
            air: self.air.unwrap_or((water + 1.0).min(30.0).max(water)),
            rh: self.rh.unwrap_or(0.55),
            activity: self.activity.unwrap_or(match class {
                PoolClass::Public | PoolClass::Wading => 1.0,
                PoolClass::Competition => 0.65,
                PoolClass::Residential => 0.5,
            }),
            cover: self.cover,
            spectators: self.spectators.unwrap_or(0),
            fill_water: self.fill_water.unwrap_or(10.0),
        }
    }
}

/// Saturation vapour pressure over water (Pa), the Magnus form.
pub fn saturation(t: f64) -> f64 {
    610.94 * (17.625 * t / (t + 243.04)).exp()
}

/// The dew point (°C) of air at `t` °C and `rh`.
pub fn dew_point(t: f64, rh: f64) -> f64 {
    let g = (rh.max(1e-6) * saturation(t) / 610.94).ln();
    243.04 * g / (17.625 - g)
}

/// Evaporation (kg/s) from `area` m² of water at `c` with activity factor `af`.
pub fn evaporation(area: f64, c: &Conditions, af: f64) -> f64 {
    let dp = (saturation(c.water) - c.rh * saturation(c.air)) / 1000.0;
    (4.0e-5 * area * af * dp).max(0.0)
}

/// A pool's evaporation's latent load (W), in use and unoccupied (covered if it has a cover), for
/// the design loads (ANA-HCL-001).
pub fn latent_loads(rec: &PoolRecipe, area: f64) -> (f64, f64) {
    let c = rec.hall.unwrap_or_default().resolve(rec.class());
    let idle = 0.5_f64.min(c.activity) * c.cover.unwrap_or(1.0);
    (evaporation(area, &c, c.activity) * H_FG, evaporation(area, &c, idle) * H_FG)
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NatatoriumParams {
    /// One pool; every pool when omitted.
    pub pool_id: Option<ElementRef>,
    /// Conditions to try instead of the pools' own.
    pub hall: Option<Hall>,
    /// The outdoor design dew point (°C): the outdoor air that evaporation alone needs is given.
    pub outdoor_dew_point: Option<f64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WaterHeat {
    /// W, losses positive.
    pub evaporation: f64,
    pub convection: f64,
    pub make_up: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Natatorium {
    pub pool: ElementId,
    pub name: String,
    pub conditions: Conditions,
    pub water_area: f64,
    pub wet_deck_area: f64,
    /// The hall air's dew point (°C).
    pub dew_point: f64,
    /// kg/h in use, unoccupied, and covered (when it has a cover).
    pub evaporation: f64,
    pub evaporation_unoccupied: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaporation_covered: Option<f64>,
    /// The latent load in use (W).
    pub latent_load: f64,
    pub water_heat: WaterHeat,
    /// Litres a day lost to evaporation, in use all day.
    pub water_per_day: f64,
    /// L/s: ASHRAE 62.1 / MAHC outdoor air, the purge, and the exhaust that keeps the hall negative.
    pub outdoor_air: f64,
    pub purge: f64,
    pub exhaust: f64,
    /// The outdoor air (L/s) that would remove the evaporation alone at the outdoor dew point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outdoor_air_for_moisture: Option<f64>,
    /// The space (or room) holding the pool, its volume, and its outdoor air changes an hour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hall: Option<ElementId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hall_volume: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub air_changes: Option<f64>,
    pub notes: Vec<String>,
}

/// Humidity ratio (kg/kg) of air with vapour pressure `p` Pa at sea level.
fn ratio(p: f64) -> f64 {
    0.621_945 * p / (101_325.0 - p)
}

/// The MEP space a pool stands in (the one the design loads see), else the room: one on its
/// level whose region holds the pool's middle, with its geometry.
pub fn hall_of(store: &Store, rec: &PoolRecipe) -> Option<(ElementId, crate::rooms::RoomGeometry)> {
    let n = rec.outline.len().max(1) as f64;
    let c = rec.outline.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
    let level = crate::id::GlobalId::parse(&rec.level).and_then(|g| store.find_by_guid(&g))?;
    if let Some(s) = crate::mep_spaces::space_at(store, level, c)
        && let Some(g) = store.get(s).and_then(|e| crate::mep_spaces::geometry(store, e))
    {
        return Some((s, g));
    }
    crate::rooms::all(store).into_iter().filter(|r| !r.redundant && r.level == rec.level).find(|r| r.geometry.region.as_ref().is_some_and(|g| g.contains(c))).map(|r| (r.id, r.geometry))
}

/// A pool hall's air and its pool's water heat balance (`pool.natatorium`).
pub(crate) fn natatorium(store: &Store, p: Option<NatatoriumParams>) -> Result<Vec<Natatorium>> {
    let p = p.unwrap_or_default();
    let only = match &p.pool_id {
        Some(r) => Some(store.resolve(r).ok_or_else(|| Error::new("element.not_found", "no such pool"))?),
        None => None,
    };
    let mut out = Vec::new();
    for (id, g) in pools::all(store) {
        if only.is_some_and(|o| o != id) {
            continue;
        }
        let e = store.get(id).expect("listed");
        let rec = pools::recipe(e).expect("a pool");
        let c = p.hall.or(rec.hall).unwrap_or_default().resolve(rec.class());
        let a = g.area;
        let deck = crate::polygons::area(&pools::offset(&rec.outline, WET_DECK)).abs() - a;
        let use_ = evaporation(a, &c, c.activity);
        let idle = evaporation(a, &c, 0.5_f64.min(c.activity));
        let covered = c.cover.map(|f| idle * f);
        let dt = c.water - c.air;
        let convection = 0.22 * dt.abs().cbrt() * dt * a;
        let make_up = use_ * C_W * (c.water - c.fill_water);
        let evap_w = use_ * H_FG;
        let oa = OA_AREA * (a + deck) + OA_PERSON * f64::from(c.spectators);
        let dew = dew_point(c.air, c.rh);
        let for_moisture = p.outdoor_dew_point.map(|odp| {
            let dw = ratio(c.rh * saturation(c.air)) - ratio(saturation(odp));
            // 1.2 kg/m³ of air; L/s.
            if dw > 1e-6 { use_ / (1.2 * dw) * 1000.0 } else { f64::INFINITY }
        });
        let room = hall_of(store, &rec);
        let volume = room.as_ref().map(|(_, g)| g.area() * (g.z_top - g.z_base));
        let mut notes = vec![
            format!("Surfaces colder than the hall air's dew point, {dew:.1} °C, condense: insulate the envelope and keep a vapour retarder of 0.2 perm or less inside it (MAHC 4.2.2.2)."),
            "Keep the air over the water to 0.15 m/s (30 fpm) or less, or work the evaporation out again (MAHC 4.6.2.7.9.1.3).".to_owned(),
            "Exhaust low near the water to take off chloramines, and keep chemical-store, toilet and changing exhausts out of the hall (MAHC 4.6.2.7.9, 4.6.2.7.10.1).".to_owned(),
        ];
        if c.air < c.water {
            notes.push(format!("The air ({:.1} °C) is cooler than the water ({:.1} °C): evaporation and the water's heat loss rise; ASHRAE and Sport England keep the air 1–2 K warmer.", c.air, c.water));
        }
        if c.rh > 0.6 + 1e-9 {
            notes.push(format!("{:.0} % RH is above ASHRAE's 50–60 % (Sport England allows 60 % ± 10 %).", c.rh * 100.0));
        }
        if let Some(f) = for_moisture
            && f > oa
        {
            notes.push(if f.is_finite() {
                format!("At the outdoor dew point, removing the evaporation with outdoor air alone takes {f:.0} L/s, more than the {oa:.0} L/s minimum: dehumidify mechanically or bring in more air.")
            } else {
                "The outdoor air is as humid as the hall's: outdoor air cannot remove the evaporation; dehumidify mechanically.".to_owned()
            });
        }
        out.push(Natatorium {
            pool: id,
            name: e.name.clone().unwrap_or_default(),
            conditions: c,
            water_area: a,
            wet_deck_area: deck,
            dew_point: dew,
            evaporation: use_ * 3600.0,
            evaporation_unoccupied: idle * 3600.0,
            evaporation_covered: covered.map(|v| v * 3600.0),
            latent_load: evap_w,
            water_heat: WaterHeat { evaporation: evap_w, convection, make_up, total: evap_w + convection + make_up },
            water_per_day: use_ * 86_400.0,
            outdoor_air: oa,
            purge: 2.0 * oa,
            exhaust: 1.05 * oa,
            outdoor_air_for_moisture: for_moisture.filter(|f| f.is_finite()),
            hall: room.as_ref().map(|r| r.0),
            hall_volume: volume,
            air_changes: volume.filter(|v| *v > 0.0).map(|v| oa / 1000.0 * 3600.0 / v),
            notes,
        });
    }
    Ok(out)
}
