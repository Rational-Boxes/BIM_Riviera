// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Edit Layout (EDIT_AUDIT phase 5): a piece of a layout (a kitchen line-up, cabinet or counter
//! run, closet, gym or studio, seating, interior layout, retail run, baffles, tile, locker room
//! or self-checkout pod) selected, the arguments it was made with are shown one per row and the
//! layout is made again with those changed (`layout.get`, `layout.redo`; one undo step).

use serde_json::{Value, json};
use wxdragon::prelude::*;

use super::{Gui, error};

/// A value as it is typed: text bare, everything else as JSON.
fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// What was typed: JSON if it reads as JSON, else text.
fn typed(text: &str, was: &Value) -> Value {
    let t = text.trim();
    if was.is_string() {
        return Value::String(t.to_owned());
    }
    serde_json::from_str(t).unwrap_or_else(|_| Value::String(t.to_owned()))
}

/// A row's label from its argument's name: `toward` → "Toward".
fn label(key: &str) -> String {
    let s = key.replace('_', " ");
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

impl Gui {
    /// Modify | Components › Edit Layout.
    pub(crate) fn edit_layout(&mut self) {
        let frame = self.frame;
        let Some(piece) = self.selection().first().copied() else {
            return error(&frame, "Select a piece of the layout to edit.");
        };
        let layout = match self.run("layout.get", json!({ "element_id": piece })) {
            Ok(l) => l,
            Err(e) => return error(&frame, &format!("This was not placed by a layout tool, so it cannot be made again:\n{e}")),
        };
        let command = layout["command"].as_str().unwrap_or_default().to_owned();
        let params = layout["params"].as_object().cloned().unwrap_or_default();
        let n = layout["elements"].as_array().map_or(0, Vec::len);
        let f = super::reinforcement_ui::Form::new(&frame, "Edit Layout");
        let rows: Vec<(String, Value, TextCtrl)> = params
            .iter()
            .map(|(k, v)| (k.clone(), v.clone(), f.text(&label(k), &shown(v), "Lists and points as JSON, for example [1.5, 2.0]")))
            .collect();
        let ok = f.show(&format!("Made by {command} ({n} pieces). Changed values make it again in one step; Undo puts it back."), "Make Again");
        let mut changed = serde_json::Map::new();
        for (k, was, ctrl) in &rows {
            let now = typed(&ctrl.get_value(), was);
            if now != *was {
                changed.insert(k.clone(), now);
            }
        }
        f.dialog.destroy();
        if !ok || changed.is_empty() {
            return;
        }
        match self.run("layout.redo", json!({ "element_id": piece, "params": changed })) {
            Ok(r) => {
                self.clear_selection();
                self.properties_stale = true;
                self.refresh_scene(&frame);
                frame.set_status_text(&format!("Layout made again ({} pieces replaced)", r["removed"]), 0);
            }
            Err(e) => error(&frame, &format!("The layout was not made again:\n{e}")),
        }
    }
}
