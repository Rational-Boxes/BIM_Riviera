// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Checkout front ends (ARC-RETAIL-003, `INTERIORS_PLAN.md` §6.3): a bank of checkstands or
//! self-checkout lanes laid at a pitch along the store front, with their handing, the accessible
//! lanes ADA Table 227.2 asks spread across the bank, and the check of every bank.
//!
//! A bank's line is where its lanes' exit ends stand (the power-aisle side); the lanes reach from
//! it toward the sales floor. A checkstand's length is its local x, its in-feed at −x and its
//! cashier and lane light at +x; its customer aisle is on its front (−y). Mirroring a lane puts
//! the aisle on its other side. The lanes not made accessible are marked so (`rxd:accessible`
//! false), so the ADA's aisle zone applies only to the accessible ones ([`crate::rules`]).
//!
//! Each bank serves one function; a self-checkout bank is always its own function (the owner's
//! answer to the plan's Question 5).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::store::Store;
use crate::txn::Transaction;

/// The tag on a bank's lanes: `{frontend, function, lane, accessible}`.
pub const LANE_KEY: &str = "rxd:checkout_lane";
const IN: f64 = 0.0254;
const FT2: f64 = 0.092_903_04;
/// Table 227.2's exception: under 5,000 ft² of selling space, one accessible aisle.
const SMALL_STORE: f64 = 5000.0 * FT2;
const DEFAULT_ITEM: &str = "Checkstand, carousel rear 142 × 48 in";

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

/// Which side each lane's customer aisle is on, looking from the exit toward the sales floor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Handing {
    /// Every lane alike, the aisle on its right.
    #[default]
    Right,
    /// Every lane alike, the aisle on its left.
    Left,
    /// Tandem pairs, cashier to cashier, sharing nothing but their backs.
    Paired,
}

/// ADA Table 227.2: accessible check-out aisles for `n` of one function.
pub fn table_227_2(n: u32) -> u32 {
    match n {
        0 => 0,
        1..=4 => 1,
        5..=8 => 2,
        9..=15 => 3,
        _ => 3 + ((n - 15) * 20).div_ceil(100),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutParams {
    /// The line the lanes' exit ends stand on: from here …
    pub start: [f64; 2],
    /// … to here.
    pub end: [f64; 2],
    pub level: ElementRef,
    /// A point on the sales floor side.
    pub toward: [f64; 2],
    /// The lanes (default as many as fit).
    pub count: Option<u32>,
    /// Centre to centre (default 84 in: a 48 in checkstand and a 36 in aisle); for tandem pairs,
    /// half a pair's pitch.
    #[schemars(extend("x-riviera-data-type" = "length"))]
    pub pitch: Option<f64>,
    /// The catalogue item (default the carousel checkstand; or a self-checkout lane).
    pub item: Option<String>,
    #[serde(default)]
    pub handing: Handing,
    /// What the bank serves (default `checkout`; `express`, `self_checkout`, …).
    pub function: Option<String>,
    /// The accessible lanes, by index from the start (default as many as Table 227.2 asks, spread
    /// across the bank).
    pub accessible: Option<Vec<u32>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Lane {
    pub element: ElementId,
    pub index: u32,
    pub accessible: bool,
    /// The customer aisle beside it (metres; to the next lane, or none at the bank's end).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aisle: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Bank {
    pub frontend: String,
    pub function: String,
    pub lanes: Vec<Lane>,
    pub pitch: f64,
    /// Accessible lanes Table 227.2 asks of this bank alone.
    pub accessible_required: u32,
}

/// Lays out a bank of checkouts (`frontend.layout`).
pub(crate) fn layout(txn: &mut Transaction<'_>, p: LayoutParams) -> Result<Bank> {
    let item = p.item.clone().unwrap_or_else(|| DEFAULT_ITEM.to_owned());
    let size = crate::interior_layouts::sizes().get(&item).copied().ok_or_else(|| bad("frontend.no_item", format!("no catalogue item {item:?}")))?;
    let types = crate::furnishings::load(txn, crate::furnishings::LoadParams { names: vec![item.clone()] })?;
    let ty = types[&item];
    let store = txn.store().clone();
    let kind = store.get(ty).and_then(|t| t.param("@ElementType")).and_then(Value::as_str).unwrap_or_default().to_owned();
    if kind != "Checkstand" && kind != "SelfCheckout" {
        return Err(bad("frontend.not_a_checkout", format!("{item} is not a checkstand or self-checkout")));
    }
    let (len, width) = (size[0], size[1]);
    let v = [p.end[0] - p.start[0], p.end[1] - p.start[1]];
    let line = v[0].hypot(v[1]);
    if line < width {
        return Err(bad("frontend.too_short", "the front is narrower than one lane"));
    }
    let u = [v[0] / line, v[1] / line];
    // Toward the sales floor, square to the line.
    let mut n = [-u[1], u[0]];
    if (p.toward[0] - p.start[0]) * n[0] + (p.toward[1] - p.start[1]) * n[1] < 0.0 {
        n = [-n[0], -n[1]];
    }
    let pitch = p.pitch.unwrap_or(84.0 * IN);
    if pitch < width + 1e-6 {
        return Err(bad("frontend.pitch", format!("the pitch is under a lane's {:.0} mm width", width * 1000.0)));
    }
    let paired = p.handing == Handing::Paired;
    // Lane centres along the line: each lane `width` wide; same-handed at the pitch, pairs back
    // to back at twice it.
    let centre = |k: u32| -> f64 {
        if paired {
            let pair = f64::from(k / 2) * 2.0 * pitch;
            pair + width / 2.0 + if k % 2 == 1 { width + 0.05 } else { 0.0 }
        } else {
            width / 2.0 + f64::from(k) * pitch
        }
    };
    let fits = |k: u32| centre(k) + width / 2.0 <= line + 1e-6;
    let count = match p.count {
        Some(c) if c == 0 => return Err(bad("frontend.no_lanes", "a bank needs a lane")),
        Some(c) if !fits(c - 1) => return Err(bad("frontend.too_short", format!("{c} lanes do not fit the {line:.2} m front at this pitch"))),
        Some(c) => c,
        None => (0..10_000).take_while(|k| fits(*k)).count() as u32,
    };
    let function = if kind == "SelfCheckout" { "self_checkout".to_owned() } else { p.function.clone().unwrap_or_else(|| "checkout".into()) };
    let required = table_227_2(count);
    let accessible: Vec<u32> = match &p.accessible {
        Some(list) => {
            if let Some(k) = list.iter().find(|k| **k >= count) {
                return Err(bad("frontend.bad_lane", format!("there is no lane {k}: the bank has {count}")));
            }
            list.clone()
        }
        // Spread: the middle of each of `required` equal shares of the bank.
        None => (0..required).map(|j| ((f64::from(j) + 0.5) * f64::from(count) / f64::from(required)).floor() as u32).collect(),
    };
    let frontend = crate::id::GlobalId::new().as_str().to_owned();
    // A lane's local x points from the sales floor to the exit.
    let ax = [-n[0], -n[1]];
    let rotation = ax[1].atan2(ax[0]);
    // Its front (local −y) is then on the right of `ax` (looking toward the exit): toward −u or
    // +u. The aisle side the handing asks for, looking toward the sales floor.
    let front_dir = [ax[1], -ax[0]];
    let mut lanes = Vec::new();
    for k in 0..count {
        let s = centre(k);
        // Looking toward the sales floor (along n), right is [n.y, −n.x]. A pair's lanes have
        // their aisles away from each other, along the line.
        let right = [n[1], -n[0]];
        let aisle_dir = match p.handing {
            Handing::Right => right,
            Handing::Left => [-right[0], -right[1]],
            Handing::Paired if k % 2 == 0 => [-u[0], -u[1]],
            Handing::Paired => u,
        };
        let mirror = aisle_dir[0] * front_dir[0] + aisle_dir[1] * front_dir[1] < 0.0;
        let point = [p.start[0] + u[0] * s + n[0] * len / 2.0, p.start[1] + u[1] * s + n[1] * len / 2.0];
        let id = crate::mesh_objects::place(txn, crate::mesh_objects::PlaceParams { type_id: ElementRef::Id(ty), point, level: p.level.clone(), offset: 0.0, rotation, mirror, scale: 1.0 })?;
        let acc = accessible.contains(&k);
        let tag = json!({ "frontend": frontend, "function": function, "lane": k, "accessible": acc });
        txn.modify(id, |e| {
            e.params.insert(LANE_KEY.into(), tag);
            if !acc {
                e.params.insert(crate::rules::ACCESSIBLE_KEY.into(), Value::Bool(false));
            }
        })?;
        lanes.push((id, k, acc, aisle_dir, s));
    }
    // Each lane's aisle: to the next lane on its aisle side.
    let out = lanes
        .iter()
        .map(|(id, k, acc, dir, s)| {
            let toward_plus = dir[0] * u[0] + dir[1] * u[1] > 0.0;
            let edge = if toward_plus { s + width / 2.0 } else { s - width / 2.0 };
            let next = lanes
                .iter()
                .filter(|o| o.1 != *k)
                .map(|o| if toward_plus { o.4 - width / 2.0 - edge } else { edge - (o.4 + width / 2.0) })
                .filter(|g| *g > -1e-6)
                .fold(f64::INFINITY, f64::min);
            Lane { element: *id, index: *k, accessible: *acc, aisle: next.is_finite().then_some(next) }
        })
        .collect();
    Ok(Bank { frontend, function, lanes: out, pitch, accessible_required: required })
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckParams {
    /// The store's selling space (m²), for Table 227.2's small-store exception; default the sales
    /// floor rooms marked (`retail.set_area`).
    #[schemars(extend("x-riviera-data-type" = "area"))]
    pub selling_area: Option<f64>,
    /// An alteration: one accessible lane per function is enough until the table is met
    /// (§227.2.1).
    #[serde(default)]
    pub alteration: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Function {
    /// The function, or a self-checkout bank's own key.
    pub function: String,
    pub lanes: u32,
    pub accessible: u32,
    pub required: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CheckoutFinding {
    pub rule: String,
    pub source: String,
    /// `error` or `note`.
    pub severity: String,
    pub function: String,
    pub message: String,
    pub short: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CheckoutCheck {
    pub functions: Vec<Function>,
    /// The selling space used for the exception, and whether it applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selling_area: Option<f64>,
    pub small_store: bool,
    pub findings: Vec<CheckoutFinding>,
}

/// Checks the check-out aisles (`checkout.check`): ADA Table 227.2 per function, with the small
/// store exception and the alterations rule; whether the accessible lanes are spread (a note).
pub(crate) fn check(store: &Store, p: Option<CheckParams>) -> Result<CheckoutCheck> {
    let p = p.unwrap_or_default();
    // function key → [(bank, lane, accessible)].
    let mut by: BTreeMap<String, Vec<(String, u64, bool)>> = BTreeMap::new();
    for e in store.iter() {
        let Some(t) = e.param(LANE_KEY) else { continue };
        let (Some(bank), Some(f)) = (t.get("frontend").and_then(Value::as_str), t.get("function").and_then(Value::as_str)) else { continue };
        let key = if f == "self_checkout" { format!("self_checkout {}", &bank[..bank.len().min(8)]) } else { f.to_owned() };
        let acc = e.param(crate::rules::ACCESSIBLE_KEY).and_then(Value::as_bool) != Some(false);
        by.entry(key).or_default().push((bank.to_owned(), t.get("lane").and_then(Value::as_u64).unwrap_or(0), acc));
    }
    let selling_area = p.selling_area.or_else(|| {
        let a: f64 = crate::rooms::all(store)
            .into_iter()
            .filter(|r| !r.redundant && store.get(r.id).and_then(|e| e.param(crate::retail::AREA_KEY)).and_then(Value::as_str) == Some("sales"))
            .map(|r| r.geometry.area())
            .sum();
        (a > 0.0).then_some(a)
    });
    let small_store = selling_area.is_some_and(|a| a < SMALL_STORE);
    let mut functions = Vec::new();
    let mut findings = Vec::new();
    const SRC: &str = "S1354";
    for (f, lanes) in by {
        let n = u32::try_from(lanes.len()).unwrap_or(u32::MAX);
        let accessible = u32::try_from(lanes.iter().filter(|l| l.2).count()).unwrap_or(u32::MAX);
        let mut required = table_227_2(n);
        if small_store || p.alteration {
            required = required.min(1);
        }
        if accessible < required {
            let why = if small_store { " (one, under 5,000 ft² of selling space)" } else if p.alteration { " (one, in an alteration, §227.2.1)" } else { "" };
            findings.push(CheckoutFinding {
                rule: "ADA 2010 Table 227.2".into(),
                source: SRC.into(),
                severity: "error".into(),
                function: f.clone(),
                message: format!("{n} {} lanes need {required} accessible{why}; {accessible} are", f.replace('_', " ")),
                short: required - accessible,
            });
        }
        // Spread: with more than one accessible lane in a bank, not all side by side.
        let mut banks: BTreeMap<&str, Vec<(u64, bool)>> = BTreeMap::new();
        for l in &lanes {
            banks.entry(l.0.as_str()).or_default().push((l.1, l.2));
        }
        for (_, mut b) in banks {
            b.sort();
            let idx: Vec<u64> = b.iter().filter(|l| l.1).map(|l| l.0).collect();
            if idx.len() > 1 && b.len() > idx.len() + 1 && idx.windows(2).all(|w| w[1] == w[0] + 1) {
                findings.push(CheckoutFinding {
                    rule: "ADA 2010 227.2 dispersion".into(),
                    source: SRC.into(),
                    severity: "note".into(),
                    function: f.clone(),
                    message: format!("the accessible {} lanes are side by side; spread them across the bank", f.replace('_', " ")),
                    short: 0,
                });
            }
        }
        functions.push(Function { function: f, lanes: n, accessible, required });
    }
    findings.sort_by_key(|f| f.severity != "error");
    Ok(CheckoutCheck { functions, selling_area, small_store, findings })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EstimateParams {
    /// Customers at the peak hour.
    pub customers_per_hour: f64,
    pub items_per_basket: f64,
    /// Items a staffed lane scans an hour (default 900).
    pub items_per_hour: Option<f64>,
    /// The average wait in the queue to keep under (minutes, default 3).
    pub target_wait: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Estimate {
    pub lanes: u32,
    /// Each lane's share of the time busy.
    pub utilisation: f64,
    /// The average wait in the queue (minutes).
    pub wait: f64,
    /// Always true: a queueing estimate, not a requirement.
    pub estimate: bool,
    pub note: String,
}

/// Erlang C: the chance an arrival waits, with `c` servers and offered load `a` (erlangs).
fn erlang_c(c: u32, a: f64) -> f64 {
    // Erlang B by recursion, then C from B.
    let mut b = 1.0;
    for k in 1..=c {
        b = a * b / (f64::from(k) + a * b);
    }
    let rho = a / f64::from(c);
    b / (1.0 - rho + rho * b)
}

/// Estimates how many lanes a peak hour needs (`checkout.lanes_estimate`): an M/M/c queue, the
/// fewest lanes whose average wait is under the target.
pub(crate) fn lanes_estimate(_: &Store, p: EstimateParams) -> Result<Estimate> {
    let rate = p.items_per_hour.unwrap_or(900.0);
    if !(p.customers_per_hour > 0.0 && p.items_per_basket > 0.0 && rate > 0.0) {
        return Err(bad("checkout.bad_estimate", "customers, items per basket and items an hour must be above zero"));
    }
    let target = p.target_wait.unwrap_or(3.0);
    let mu = rate / p.items_per_basket;
    let a = p.customers_per_hour / mu;
    let mut c = a.floor() as u32 + 1;
    loop {
        let wq = erlang_c(c, a) / (f64::from(c) * mu - p.customers_per_hour) * 60.0;
        if wq <= target || c > 10_000 {
            return Ok(Estimate {
                lanes: c,
                utilisation: a / f64::from(c),
                wait: wq,
                estimate: true,
                note: "An M/M/c queueing estimate from the peak hour; arrivals and service vary more in stores, so check it against simulation or the operator's experience.".into(),
            });
        }
        c += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_and_queue() {
        assert_eq!([1, 4, 5, 8, 9, 15, 16, 20, 21].map(table_227_2), [1, 1, 2, 2, 3, 3, 4, 4, 5]);
        // 300 customers an hour with 20 items at 900 items an hour: 6.67 erlangs, so at least 7
        // lanes; the wait falls under 3 minutes with 8.
        let e = lanes_estimate(&Store::default(), EstimateParams { customers_per_hour: 300.0, items_per_basket: 20.0, items_per_hour: None, target_wait: None }).unwrap();
        assert!(e.lanes >= 7 && e.wait <= 3.0 && e.utilisation < 1.0, "{e:?}");
        let fewer = erlang_c(e.lanes - 1, 300.0 / 45.0) / (f64::from(e.lanes - 1) * 45.0 - 300.0) * 60.0;
        assert!(fewer > 3.0 || e.lanes - 1 <= 6, "one fewer lane waits {fewer} min");
    }
}
