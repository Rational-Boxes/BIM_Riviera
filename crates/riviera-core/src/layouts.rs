// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Layouts made again (EDIT_AUDIT phase 5): the pieces a layout tool places (a kitchen line-up, a
//! cabinet or counter run, a closet, a gym or studio, seating, an interior layout, a retail run,
//! sun-control baffles, tile, a locker room, a self-checkout pod, an atrium with its void, guards,
//! roof and walls) remember the command and the
//! arguments that made them. `layout.get` reads them back from any piece; `layout.redo` deletes
//! the set and runs the command again with changed arguments, in one undo step.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::command::CommandContext;
use crate::error::{Error, Result};
use crate::id::{ElementId, ElementRef, GlobalId};
use crate::store::Store;
use crate::txn::Transaction;

/// On each piece: `{set, command, params}`.
pub const KEY: &str = "rxd:made_by";

/// The layout commands whose pieces can be made again.
pub const COMMANDS: [&str; 14] = [
    "foodservice.lineup",
    "cabinet.run",
    "counter.run",
    "closet.create",
    "gym.layout",
    "gym.studio",
    "seating.layout",
    "interior.layout",
    "retail.run",
    "shading.baffles",
    "tile.create",
    "bath.locker_room",
    "sco.place_pod",
    "atrium.create",
];

pub fn is_layout(name: &str) -> bool {
    COMMANDS.contains(&name)
}

/// Marks what a layout command just made (in its transaction) with the command and arguments.
pub(crate) fn tag(txn: &mut Transaction<'_>, command: &str, params: &Value) -> Result<()> {
    let added = txn.pending_changes().added;
    if added.is_empty() {
        return Ok(());
    }
    let set = GlobalId::new().to_string();
    let v = json!({ "set": set, "command": command, "params": params });
    for id in added {
        if txn.store().get(id).is_some_and(|e| !crate::categories::is_type(e)) {
            let v = v.clone();
            txn.modify(id, |e| {
                e.params.insert(KEY.into(), v);
            })?;
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetParams {
    /// Any piece of the layout.
    pub element_id: ElementRef,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Layout {
    pub command: String,
    pub params: Value,
    /// Its pieces now.
    pub elements: Vec<ElementId>,
}

fn find(store: &Store, r: &ElementRef) -> Result<(String, String, Value)> {
    let id = store.resolve(r).ok_or_else(|| Error::new("element.not_found", format!("no element {r:?}")))?;
    let v = store.get(id).and_then(|e| e.param(KEY)).ok_or_else(|| Error::new("layout.none", format!("element {id} was not placed by a layout tool")))?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    Ok((s("set"), s("command"), v.get("params").cloned().unwrap_or(Value::Null)))
}

fn pieces(store: &Store, set: &str) -> Vec<ElementId> {
    store.iter().filter(|e| e.param(KEY).and_then(|v| v.get("set")).and_then(Value::as_str) == Some(set)).map(|e| e.id()).collect()
}

/// `layout.get`: the command and arguments that made a piece's layout, and its pieces.
pub fn get(store: &Store, p: GetParams) -> Result<Layout> {
    let (set, command, params) = find(store, &p.element_id)?;
    Ok(Layout { command, params, elements: pieces(store, &set) })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RedoParams {
    /// Any piece of the layout.
    pub element_id: ElementRef,
    /// Arguments to change (merged over the ones it was made with); `null` removes one.
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Redone {
    pub command: String,
    /// What the command returned.
    pub result: Value,
    pub removed: usize,
}

/// `layout.redo`: deletes a layout's pieces and runs its command again with changed arguments.
pub fn redo(cx: &mut CommandContext<'_>, p: RedoParams) -> Result<Redone> {
    let (set, command, mut params) = find(cx.store(), &p.element_id)?;
    if !is_layout(&command) {
        return Err(Error::new("layout.none", format!("`{command}` is not a layout command")));
    }
    let obj = params.as_object_mut().ok_or_else(|| Error::new("layout.none", "the layout's arguments are not an object"))?;
    for (k, v) in p.params {
        if v.is_null() {
            obj.remove(&k);
        } else {
            obj.insert(k, v);
        }
    }
    let ids = pieces(cx.store(), &set);
    let txn = cx.edit()?;
    for id in &ids {
        if txn.store().get(*id).is_some() {
            txn.delete(*id)?;
        }
    }
    let result = cx.run_nested(&command, params.clone())?;
    // The new pieces carry the changed arguments.
    let txn = cx.edit()?;
    tag(txn, &command, &params)?;
    Ok(Redone { command, result, removed: ids.len() })
}
