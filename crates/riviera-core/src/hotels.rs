// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Hotel guest rooms (ARC-INT-002, `INTERIORS_PLAN.md` §4.3): rooms marked as guest rooms with
//! their class and accessible features, and the room matrix that counts them floor by floor and
//! by class and checks the counts:
//!
//! - the 2010 ADA Standards: Table 224.2 (mobility features, with and without roll-in showers),
//!   Table 224.4 (communication features) and §224.5 (one mobility room with communication
//!   features, the 10 % overlap cap, and dispersion across classes);
//! - England's Approved Document M vol. 2 §4.24: one wheelchair-accessible bedroom per 20.
//!
//! A guest room is a Riviera room (`IfcSpace`) carrying `rxd:guest_room`. The hotel templates of
//! `interior.layout` mark the room they furnish; `hotel.set_room` marks any room. Until modular
//! units (ARC-GRP-004) are built, the counts are taken from the rooms placed.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef};
use crate::store::Store;
use crate::txn::Transaction;

/// The guest-room data on a room.
pub const GUEST_KEY: &str = "rxd:guest_room";

fn bad(code: &'static str, msg: impl Into<String>) -> Error {
    Error::new(code, msg)
}

/// A guest room's mobility features (ADA §806.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mobility {
    /// None.
    #[default]
    None,
    /// Mobility features, its bathroom with a tub or transfer shower.
    Mobility,
    /// Mobility features with a roll-in shower (§608.2.2 or §608.2.3).
    RollIn,
}

impl Mobility {
    pub fn accessible(self) -> bool {
        self != Mobility::None
    }
}

/// What makes a room a guest room, and of which class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GuestRoom {
    /// Its class for dispersion (§224.5): `king`, `double_queen`, `junior_suite`, or any name.
    pub room_type: String,
    /// The beds in it.
    pub beds: u32,
    #[serde(default)]
    pub mobility: Mobility,
    /// Communication features (§806.3): visible alarms and notification devices.
    #[serde(default)]
    pub communication: bool,
}

/// A room's guest-room data.
pub fn guest_room(e: &crate::element::Element) -> Option<GuestRoom> {
    serde_json::from_value(e.param(GUEST_KEY)?.clone()).ok()
}

// ---- Commands ----

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetRoomParams {
    /// Riviera rooms.
    pub room_ids: Vec<ElementRef>,
    /// Their class (default: as before, else `king`).
    pub room_type: Option<String>,
    /// Beds (default: as before, else 1).
    pub beds: Option<u32>,
    pub mobility: Option<Mobility>,
    pub communication: Option<bool>,
    /// Stop counting them as guest rooms.
    #[serde(default)]
    pub clear: bool,
}

/// Marks rooms as guest rooms, or changes their class and features (`hotel.set_room`).
pub(crate) fn set_room(txn: &mut Transaction<'_>, p: SetRoomParams) -> Result<()> {
    if p.room_ids.is_empty() {
        return Err(bad("hotel.no_rooms", "no rooms given"));
    }
    let store = txn.store().clone();
    for r in &p.room_ids {
        let id = store.resolve(r).filter(|id| store.get(*id).is_some_and(|e| crate::rooms::room_recipe(e).is_some())).ok_or_else(|| bad("hotel.not_a_room", "not a Riviera room"))?;
        if p.clear {
            txn.modify(id, |e| {
                e.params.remove(GUEST_KEY);
            })?;
            continue;
        }
        let before = store.get(id).and_then(guest_room);
        let g = GuestRoom {
            room_type: p.room_type.clone().or_else(|| before.as_ref().map(|b| b.room_type.clone())).unwrap_or_else(|| "king".into()),
            beds: p.beds.or(before.as_ref().map(|b| b.beds)).unwrap_or(1),
            mobility: p.mobility.or(before.as_ref().map(|b| b.mobility)).unwrap_or_default(),
            communication: p.communication.or(before.as_ref().map(|b| b.communication)).unwrap_or(false),
        };
        if g.room_type.trim().is_empty() || g.beds == 0 {
            return Err(bad("hotel.bad_room", "a guest room needs a class and at least one bed"));
        }
        mark(txn, id, &g)?;
    }
    Ok(())
}

pub(crate) fn mark(txn: &mut Transaction<'_>, id: ElementId, g: &GuestRoom) -> Result<()> {
    let v = serde_json::to_value(g).expect("serialises");
    txn.modify(id, |e| {
        e.params.insert(GUEST_KEY.into(), v);
    })?;
    Ok(())
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoomsParams {
    /// Only these levels.
    #[serde(default)]
    pub level_ids: Vec<ElementRef>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Listed {
    pub room: ElementId,
    pub number: String,
    pub name: String,
    /// The level's name.
    pub level: String,
    #[serde(flatten)]
    pub guest: GuestRoom,
}

fn rooms_of(store: &Store, levels: &[ElementRef]) -> Result<Vec<(Listed, f64)>> {
    let wanted: Vec<String> = levels
        .iter()
        .map(|r| store.resolve(r).and_then(|id| store.get(id)).map(|e| e.guid().to_string()).ok_or_else(|| bad("element.not_found", "no such level")))
        .collect::<Result<_>>()?;
    let mut out = Vec::new();
    for e in store.iter() {
        let (Some(r), Some(g)) = (crate::rooms::room_recipe(e), guest_room(e)) else { continue };
        if !wanted.is_empty() && !wanted.contains(&r.level) {
            continue;
        }
        let level = crate::id::GlobalId::parse(&r.level).and_then(|g| store.find_by_guid(&g)).and_then(|l| store.get(l));
        let (number, name) = crate::rooms::number_and_name(e);
        let elevation = level.map_or(0.0, crate::levels::elevation);
        let level = level.and_then(|l| l.name.clone()).unwrap_or_default();
        out.push((Listed { room: e.id(), number, name, level, guest: g }, elevation));
    }
    out.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| natural(&a.0.number).cmp(&natural(&b.0.number))).then(a.0.room.cmp(&b.0.room)));
    Ok(out)
}

/// Room numbers sorted as numbers when they are.
fn natural(s: &str) -> (u64, String) {
    (s.trim().parse().unwrap_or(u64::MAX), s.to_owned())
}

/// The guest rooms, floor by floor (`hotel.rooms`).
pub(crate) fn rooms(store: &Store, p: Option<RoomsParams>) -> Result<Vec<Listed>> {
    Ok(rooms_of(store, &p.unwrap_or_default().level_ids)?.into_iter().map(|r| r.0).collect())
}

/// The standards a matrix is checked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Standard {
    /// The 2010 ADA Standards, §224.
    #[serde(rename = "ada_2010")]
    Ada2010,
    /// Approved Document M vol. 2 (England), §4.24.
    #[serde(rename = "uk_adm_m2")]
    UkAdmM2,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MatrixParams {
    /// Only these levels (default every level).
    #[serde(default)]
    pub level_ids: Vec<ElementRef>,
    /// Default: Approved Document M when the building code is England's, else the ADA.
    #[serde(default)]
    pub standards: Vec<Standard>,
    /// The rooms the counts are based on, when not every room placed (an alteration counts the
    /// rooms altered or added, §224.1.1).
    pub total: Option<u32>,
}

/// A row of the matrix: a floor or a class.
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
pub struct Row {
    pub rooms: u32,
    /// With mobility features, roll-in or not.
    pub mobility: u32,
    pub roll_in: u32,
    pub communication: u32,
}

impl Row {
    fn add(&mut self, g: &GuestRoom) {
        self.rooms += 1;
        self.mobility += u32::from(g.mobility.accessible());
        self.roll_in += u32::from(g.mobility == Mobility::RollIn);
        self.communication += u32::from(g.communication);
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Floor {
    pub level: String,
    #[serde(flatten)]
    pub row: Row,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Class {
    pub room_type: String,
    pub beds: u32,
    #[serde(flatten)]
    pub row: Row,
}

/// Required against provided under the ADA.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AdaCounts {
    /// Table 224.2: mobility rooms without roll-in showers, with them, and in all.
    pub without_roll_in_required: u32,
    pub roll_in_required: u32,
    pub mobility_required: u32,
    pub without_roll_in: u32,
    pub roll_in: u32,
    /// Table 224.4.
    pub communication_required: u32,
    /// Rooms with communication features only, plus mobility rooms with them up to the cap.
    pub communication_counted: u32,
    /// §224.5: at most 10 % of the mobility rooms required count toward communication.
    pub overlap_cap: u32,
}

/// Required against provided under Approved Document M.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UkCounts {
    pub required: u32,
    pub accessible: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Finding {
    pub rule: String,
    pub source: String,
    /// `error` or `note`.
    pub severity: String,
    pub message: String,
    /// The shortfall, in rooms.
    pub short: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Matrix {
    /// Guest rooms placed, and the number the counts are based on.
    pub placed: u32,
    pub total: u32,
    pub floors: Vec<Floor>,
    pub classes: Vec<Class>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ada: Option<AdaCounts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uk: Option<UkCounts>,
    /// Errors first.
    pub findings: Vec<Finding>,
}

/// `n` per 100, or part, over 1000.
fn per_100_over_1000(n: u32, each: u32) -> u32 {
    n.saturating_sub(1000).div_ceil(100) * each
}

/// A percentage of `n`, rounded up.
fn percent(n: u32, pc: u32) -> u32 {
    (n * pc).div_ceil(100)
}

/// ADA Table 224.2: (without roll-in showers, with roll-in showers).
pub fn table_224_2(n: u32) -> (u32, u32) {
    match n {
        0 => (0, 0),
        1..=25 => (1, 0),
        26..=50 => (2, 0),
        51..=75 => (3, 1),
        76..=100 => (4, 1),
        101..=150 => (5, 2),
        151..=200 => (6, 2),
        201..=300 => (7, 3),
        301..=400 => (8, 4),
        401..=500 => (9, 4),
        501..=1000 => (percent(n, 2), percent(n, 1)),
        _ => (20 + per_100_over_1000(n, 1), 10 + per_100_over_1000(n, 1)),
    }
}

/// ADA Table 224.4: rooms with communication features.
pub fn table_224_4(n: u32) -> u32 {
    match n {
        0 => 0,
        // The table starts at 2 rooms; a single room is its own requirement.
        1 => 1,
        2..=25 => 2,
        26..=50 => 4,
        51..=75 => 7,
        76..=100 => 9,
        101..=150 => 12,
        151..=200 => 14,
        201..=300 => 17,
        301..=400 => 20,
        401..=500 => 22,
        501..=1000 => percent(n, 5),
        _ => 50 + per_100_over_1000(n, 3),
    }
}

/// The guest rooms counted and checked (`hotel.room_matrix`).
pub(crate) fn matrix(store: &Store, p: Option<MatrixParams>) -> Result<Matrix> {
    let p = p.unwrap_or_default();
    let listed = rooms_of(store, &p.level_ids)?;
    let mut floors: Vec<Floor> = Vec::new();
    let mut classes: BTreeMap<(String, u32), Row> = BTreeMap::new();
    let mut all = Row::default();
    // Mobility rooms that also have communication features.
    let mut both = 0;
    for (r, _) in &listed {
        match floors.last_mut() {
            Some(f) if f.level == r.level => f.row.add(&r.guest),
            _ => {
                let mut row = Row::default();
                row.add(&r.guest);
                floors.push(Floor { level: r.level.clone(), row });
            }
        }
        classes.entry((r.guest.room_type.clone(), r.guest.beds)).or_default().add(&r.guest);
        all.add(&r.guest);
        both += u32::from(r.guest.mobility.accessible() && r.guest.communication);
    }
    let placed = all.rooms;
    let total = p.total.unwrap_or(placed);
    let standards = if p.standards.is_empty() {
        if crate::codes::settings(store).code == crate::codes::Code::UkAdk { vec![Standard::UkAdmM2] } else { vec![Standard::Ada2010] }
    } else {
        p.standards.clone()
    };
    let mut findings = Vec::new();
    let mut find = |rule: &str, source: &str, error: bool, short: u32, message: String| {
        findings.push(Finding { rule: rule.into(), source: source.into(), severity: if error { "error" } else { "note" }.into(), message, short });
    };
    let mut ada = None;
    if standards.contains(&Standard::Ada2010) {
        let (without_req, roll_req) = table_224_2(total);
        let mobility_req = without_req + roll_req;
        let comm_req = table_224_4(total);
        let without = all.mobility - all.roll_in;
        // Rooms with only communication features count in full; mobility rooms with them count
        // up to a tenth of the mobility rooms required.
        let cap = mobility_req / 10;
        let counted = (all.communication - both) + both.min(cap);
        const SRC: &str = "2010 ADA Standards (S1354)";
        if all.roll_in < roll_req {
            find("ADA 2010 Table 224.2 roll-in", SRC, true, roll_req - all.roll_in, format!("{total} guest rooms need {roll_req} with mobility features and roll-in showers; {} are", all.roll_in));
        }
        if without < without_req {
            find("ADA 2010 Table 224.2", SRC, true, without_req - without, format!("{total} guest rooms need {without_req} with mobility features without roll-in showers; {without} are"));
        }
        if counted < comm_req {
            let over = if both > cap { format!(" ({} mobility rooms with them count only up to {cap}, §224.5)", both) } else { String::new() };
            find("ADA 2010 Table 224.4", SRC, true, comm_req - counted, format!("{total} guest rooms need {comm_req} with communication features; {counted} count{over}"));
        }
        if mobility_req > 0 && both == 0 {
            find("ADA 2010 224.5", SRC, true, 1, "at least one guest room with mobility features must also have communication features".into());
        }
        // Dispersion: while the rooms required reach round the classes, each class should have
        // some; the rest is a judgement the check cannot make, so it is a note.
        let n_classes = u32::try_from(classes.len()).unwrap_or(u32::MAX);
        for (what, required, has) in [("mobility", mobility_req, (|r: &Row| r.mobility) as fn(&Row) -> u32), ("communication", comm_req, |r: &Row| r.communication)] {
            if n_classes > 1 && required >= n_classes {
                let missing: Vec<String> = classes.iter().filter(|(_, r)| has(r) == 0).map(|((t, b), _)| format!("{} ({b} bed{})", t.replace('_', " "), if *b == 1 { "" } else { "s" })).collect();
                if !missing.is_empty() {
                    find("ADA 2010 224.5 dispersion", SRC, false, 0, format!("no {what}-feature room among: {}; disperse them across the classes", missing.join(", ")));
                }
            }
        }
        ada = Some(AdaCounts {
            without_roll_in_required: without_req,
            roll_in_required: roll_req,
            mobility_required: mobility_req,
            without_roll_in: without,
            roll_in: all.roll_in,
            communication_required: comm_req,
            communication_counted: counted,
            overlap_cap: cap,
        });
    }
    let mut uk = None;
    if standards.contains(&Standard::UkAdmM2) {
        let required = total.div_ceil(20);
        if all.mobility < required {
            find("ADM vol. 2 4.24", "Approved Document M vol. 2 (S1466)", true, required - all.mobility, format!("{total} bedrooms need {required} wheelchair-accessible (1 per 20 or part); {} are", all.mobility));
        }
        uk = Some(UkCounts { required, accessible: all.mobility });
    }
    if p.total.is_some_and(|t| t != placed) {
        find("count basis", "", false, 0, format!("counted on {total} rooms, not the {placed} placed"));
    }
    findings.sort_by_key(|f| f.severity != "error");
    let classes = classes.into_iter().map(|((room_type, beds), row)| Class { room_type, beds, row }).collect();
    Ok(Matrix { placed, total, floors, classes, ada, uk, findings })
}

/// The hotel templates of `interior.layout` and the guest room each makes: (template, class,
/// beds, mobility).
pub(crate) fn template_room(template: &str) -> Option<(&'static str, u32, Mobility)> {
    Some(match template {
        "hotel_king" => ("king", 1, Mobility::None),
        "hotel_double_queen" => ("double_queen", 2, Mobility::None),
        "hotel_accessible_king" => ("king", 1, Mobility::Mobility),
        "hotel_junior_suite" => ("junior_suite", 1, Mobility::None),
        _ => return None,
    })
}

/// After a hotel template is laid out: the room at the point marked as a guest room, placed first
/// when there is none. Its communication features are kept, and a roll-in room laid out with the
/// accessible template stays roll-in.
pub(crate) fn mark_layout(txn: &mut Transaction<'_>, template: &str, level: ElementId, point: [f64; 2]) -> Result<Option<ElementId>> {
    let Some((room_type, beds, mobility)) = template_room(template) else { return Ok(None) };
    let room = match crate::rooms::room_at(txn.store(), level, point) {
        Some(r) => r,
        None => crate::rooms::place(
            txn,
            crate::rooms::PlaceParams { level_id: ElementRef::Id(level), point, upper_limit: None, limit_offset: None, base_offset: None, name: Some("Guest Room".into()), number: None },
        )?,
    };
    let before = txn.store().get(room).and_then(guest_room);
    let mobility = match (mobility, before.as_ref().map(|b| b.mobility)) {
        (Mobility::Mobility, Some(Mobility::RollIn)) => Mobility::RollIn,
        (m, _) => m,
    };
    let g = GuestRoom { room_type: room_type.into(), beds, mobility, communication: before.is_some_and(|b| b.communication) };
    mark(txn, room, &g)?;
    Ok(Some(room))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_follow_the_ada() {
        assert_eq!(table_224_2(1), (1, 0));
        assert_eq!(table_224_2(60), (3, 1));
        assert_eq!(table_224_2(120), (5, 2));
        // 501–1000: 2 % and 1 %, rounded up.
        assert_eq!(table_224_2(620), (13, 7));
        // Over 1000: 20 and 10, plus 1 each per 100 or part.
        assert_eq!(table_224_2(1150), (22, 12));
        assert_eq!(table_224_4(1), 1);
        assert_eq!(table_224_4(25), 2);
        assert_eq!(table_224_4(120), 12);
        assert_eq!(table_224_4(620), 31);
        assert_eq!(table_224_4(1150), 56);
    }
}
