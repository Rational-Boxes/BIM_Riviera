// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A pool's water treatment, sized (ARC-POOL-009, `POOLS_PLAN.md` §6, research/23 C.3).
//!
//! - **Turnover**: the US basis takes the MAHC's maximum (Table 4.7.1.10: wading pools 1 h, diving
//!   pools 8 h, all other pools 6 h) over the pool and its surge volume; the UK basis Sport
//!   England's (after PWTAG): swimming pools 2.5–4.0 h, learner and teaching pools 0.5–1.5 h,
//!   diving pools 4.0–8.0 h, the upper bound used unless one is given.
//! - **Filters**: the filtered flow over the rate (MAHC high-rate media 15 gpm/ft², 36.7 m/h; UK
//!   medium-rate sand 25 m/h); backwash at 37 m/h (MAHC) or 30 m/h for 7 minutes (UK practice).
//! - **Surge or balance tank**: the MAHC's 1 gal/ft² (40.7 L/m²) of water surface; the UK's half
//!   the bathing load (3 m² a bather, HSG179) at 80 L each, plus the largest filter's backwash.
//! - **Hydraulics**: skimmers one per 500 ft² (46 m², MAHC 4.7.1.5) of a skimmer pool; gutters at
//!   125 % of the flow (MAHC 4.7.1.4); pipes at most 1.8 m/s suction and 2.4 m/s discharge (MAHC
//!   4.7.1.7.2; UK 1.5 and 2.0 m/s); two main drains each for the whole flow, three or more for
//!   the flow over one fewer (MAHC 4.7.1.6).
//! - **Plant room**: 15–30 % of the water area it serves (Sport England); bathers an hour the
//!   flow over 1.7 m³ (PWTAG); dilution 15 L a bather a day (MAHC) or 30 L (PWTAG).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::pools::{self, Edge, PoolClass};
use crate::store::Store;

/// Whose rules size the plant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// The MAHC.
    Us,
    /// Sport England and PWTAG.
    Uk,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TreatmentParams {
    /// One pool; every pool when omitted.
    pub pool_id: Option<ElementRef>,
    /// Default: UK under the UK code setting, else US.
    pub basis: Option<Basis>,
    /// Hours (default the basis's for the pool's type).
    pub turnover: Option<f64>,
    /// Filtration rate, m/h (default 36.7 US, 25 UK).
    pub filter_rate: Option<f64>,
    /// How many filters share the flow (default 2).
    pub filters: Option<u32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Pipe {
    /// m³/h.
    pub flow: f64,
    /// m/s, the limit used.
    pub velocity: f64,
    /// The least inside diameter (mm).
    pub diameter: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Treatment {
    pub pool: ElementId,
    pub name: String,
    pub basis: Basis,
    /// What the turnover was taken for.
    pub kind: String,
    pub water_volume: f64,
    pub surge_volume: f64,
    /// Hours, and the basis's range for the pool's type.
    pub turnover: f64,
    pub turnover_range: [f64; 2],
    /// m³/h of filtered water.
    pub flow: f64,
    pub filter_rate: f64,
    /// m² in all, and each.
    pub filter_area: f64,
    pub filters: u32,
    pub filter_area_each: f64,
    /// m³/h for one filter, and m³ a backwash.
    pub backwash_flow: f64,
    pub backwash_volume: f64,
    /// Skimmers needed (skimmer pools), or the gutter's capacity (m³/h, deck-level pools).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skimmers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_capacity: Option<f64>,
    pub suction: Pipe,
    pub discharge: Pipe,
    /// The main drains counted at the pool, and the flow each must carry (m³/h).
    pub main_drains: u32,
    pub flow_per_drain: f64,
    /// m², the range.
    pub plant_room: [f64; 2],
    /// Bathers an hour the flow treats (PWTAG's 1.7 m³ a bather).
    pub bathers_per_hour: f64,
    /// Litres of fresh water a day for dilution, at the maximum bathing load (3 m² a bather) once.
    pub dilution_per_day: f64,
    pub notes: Vec<String>,
}

fn pipe(flow: f64, v: f64) -> Pipe {
    let q = flow / 3600.0;
    Pipe { flow, velocity: v, diameter: (4.0 * q / (std::f64::consts::PI * v)).sqrt() * 1000.0 }
}

/// A pool's water treatment, sized (`pool.treatment`).
pub(crate) fn treatment(store: &Store, p: Option<TreatmentParams>) -> Result<Vec<Treatment>> {
    let p = p.unwrap_or_default();
    let only = match &p.pool_id {
        Some(r) => Some(store.resolve(r).ok_or_else(|| Error::new("element.not_found", "no such pool"))?),
        None => None,
    };
    let basis = p.basis.unwrap_or(if crate::codes::settings(store).code == crate::codes::Code::UkAdk { Basis::Uk } else { Basis::Us });
    let boards = crate::rules::Selector { object_types: vec!["Diving Board".into()], ..Default::default() };
    let drains = crate::rules::Selector { object_types: vec!["Main Drain".into()], ..Default::default() };
    let mut out = Vec::new();
    for (id, g) in pools::all(store) {
        if only.is_some_and(|o| o != id) {
            continue;
        }
        let e = store.get(id).expect("listed");
        let rec = pools::recipe(e).expect("a pool");
        let class = rec.class();
        let area = g.area;
        let deepest = rec.profile.iter().map(|s| s.depth).fold(0.0, f64::max);
        let near = |sel: &crate::rules::Selector, inside_only: bool| {
            store
                .iter()
                .filter(|o| o.id() != id && sel.selects_element(store, o))
                .filter_map(|o| crate::rules::item(store, o))
                .filter(|it| {
                    let c = it.frame.apply([(it.x[0] + it.x[1]) / 2.0, (it.front + it.back) / 2.0, 0.0]);
                    crate::sco::inside([c[0], c[1]], &rec.outline) || (!inside_only && pools::edge_distance([c[0], c[1]], &rec.outline) <= 1.0)
                })
                .count() as u32
        };
        let diving = near(&boards, false) > 0;
        let (kind, range) = match (basis, class, diving) {
            (_, _, true) if basis == Basis::Us => ("diving pool (MAHC Table 4.7.1.10)", [0.0, 8.0]),
            (Basis::Us, PoolClass::Wading, _) => ("wading pool (MAHC Table 4.7.1.10)", [0.0, 1.0]),
            (Basis::Us, ..) => ("other pools (MAHC Table 4.7.1.10)", [0.0, 6.0]),
            (Basis::Uk, _, true) => ("diving pool (Sport England Table 3)", [4.0, 8.0]),
            (Basis::Uk, PoolClass::Wading, _) => ("leisure water under 0.5 m (Sport England Table 3)", [0.17, 0.75]),
            (Basis::Uk, ..) if deepest <= 1.5 => ("learner or teaching pool (Sport England Table 3)", [0.5, 1.5]),
            (Basis::Uk, ..) => ("swimming pool (Sport England Table 3)", [2.5, 4.0]),
        };
        let deck_level = rec.edges.iter().all(|e| matches!(e, Edge::DeckLevel { .. }));
        let bathers = area / 3.0;
        let rate = p.filter_rate.unwrap_or(match basis {
            Basis::Us => 36.7,
            Basis::Uk => 25.0,
        });
        let filters = p.filters.unwrap_or(2).max(1);
        let (bw_rate, bw_minutes) = match basis {
            Basis::Us => (36.7, 7.0),
            Basis::Uk => (30.0, 7.0),
        };
        // The surge first (the US counts it in the turnover volume); the UK's needs the
        // backwash, which needs the flow: the bather term is fixed, so take it in two passes.
        let turnover = p.turnover.unwrap_or(range[1]);
        let mut surge = match basis {
            Basis::Us => 0.0407 * area,
            Basis::Uk => 0.5 * bathers * 0.08,
        };
        let mut flow = (g.volume + if basis == Basis::Us { surge } else { 0.0 }) / turnover;
        let mut area_f = flow / rate;
        let backwash_flow = area_f / f64::from(filters) * bw_rate;
        let backwash_volume = backwash_flow * bw_minutes / 60.0;
        if basis == Basis::Uk {
            surge += backwash_volume;
        } else {
            flow = (g.volume + surge) / turnover;
            area_f = flow / rate;
        }
        let (vs, vd) = match basis {
            Basis::Us => (1.8, 2.4),
            Basis::Uk => (1.5, 2.0),
        };
        let n_drains = near(&drains, true);
        let flow_per_drain = if n_drains >= 3 { flow / f64::from(n_drains - 1) } else { flow };
        let mut notes = Vec::new();
        if p.turnover.is_some_and(|t| t > range[1] + 1e-9) {
            notes.push(format!("A {turnover:.1} h turnover is longer than the {:.1} h for a {kind}.", range[1]));
        }
        if !deck_level {
            let width = {
                let (a, u) = (rec.axis[0], {
                    let [a, b] = rec.axis;
                    let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
                    [(b[0] - a[0]) / l, (b[1] - a[1]) / l]
                });
                let v = [-u[1], u[0]];
                let s: Vec<f64> = rec.outline.iter().map(|q| (q[0] - a[0]) * v[0] + (q[1] - a[1]) * v[1]).collect();
                s.iter().copied().fold(f64::MIN, f64::max) - s.iter().copied().fold(f64::MAX, f64::min)
            };
            if basis == Basis::Us && width > 9.144 + 1e-6 {
                notes.push(format!("A skimmer pool {width:.1} m wide: the MAHC keeps skimmer pools to 30 ft (9.1 m) wide; use perimeter overflow gutters (4.7.1.5)."));
            }
        }
        if class == PoolClass::Wading && basis == Basis::Us {
            notes.push("A wading pool needs secondary disinfection (UV or ozone) as well (MAHC Table 4.7.1.10, 4.7.3.3).".into());
        }
        if n_drains == 0 {
            notes.push("No main drains are modelled: two blockable drains each carry the whole flow (MAHC 4.7.1.6).".into());
        }
        notes.push(match basis {
            Basis::Us => "Each chemical store has its own continuous exhaust, not connected to the pool hall's, its outlets and make-up air 20 ft (6.1 m) from any air intake (MAHC 4.9.2.5, 4.9.2.6).".into(),
            Basis::Uk => "Store the acid and the chlorine apart, each with its own ventilation; bund the tanks and provide a drench shower and eyewash (Sport England).".into(),
        });
        out.push(Treatment {
            pool: id,
            name: e.name.clone().unwrap_or_default(),
            basis,
            kind: kind.into(),
            water_volume: g.volume,
            surge_volume: surge,
            turnover,
            turnover_range: range,
            flow,
            filter_rate: rate,
            filter_area: area_f,
            filters,
            filter_area_each: area_f / f64::from(filters),
            backwash_flow: area_f / f64::from(filters) * bw_rate,
            backwash_volume: area_f / f64::from(filters) * bw_rate * bw_minutes / 60.0,
            skimmers: (!deck_level).then(|| (area / 46.45 - 1e-9).ceil().max(1.0) as u32),
            gutter_capacity: deck_level.then_some(1.25 * flow),
            suction: pipe(flow, vs),
            discharge: pipe(flow, vd),
            main_drains: n_drains,
            flow_per_drain,
            plant_room: [0.15 * area, 0.30 * area],
            bathers_per_hour: flow / 1.7,
            dilution_per_day: bathers
                * match basis {
                    Basis::Us => 15.0,
                    Basis::Uk => 30.0,
                },
            notes,
        });
    }
    Ok(out)
}
