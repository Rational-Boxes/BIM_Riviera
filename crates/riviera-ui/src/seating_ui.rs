// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Seating Layout (ARC-INT-006): the selected room set out for a banquet, theatre, classroom,
//! boardroom, dining or reception, with its occupant load, exits and egress width.

use serde_json::json;
use wxdragon::prelude::*;

use super::{Gui, error};

impl Gui {
    /// Architecture › Build › Seating Layout.
    pub(crate) fn seating_layout(&mut self) {
        let frame = self.frame;
        let Some(room) = self.selected_room() else { return };
        let styles = ["banquet", "theatre", "classroom", "boardroom", "dining", "reception"];
        let labels: Vec<String> = ["Banquet rounds", "Theatre rows", "Classroom tables", "Boardroom", "Restaurant dining", "Reception (standing)"].iter().map(|s| (*s).to_owned()).collect();
        let f = super::reinforcement_ui::Form::new(&frame, "Seating Layout");
        let style = f.choice("Set-up", &labels, 0);
        let round = f.choice("Banquet rounds", &["72 in, ten seats".to_owned(), "60 in, eight seats".to_owned()], 0);
        let clear = f.text("Clear of the walls (m)", "1.12", "An egress aisle round the room (44 in)");
        let stage = f.text("Front kept clear (m)", "3.0", "Theatre and classroom: for a stage or screen at the room's front");
        let sprinklered = f.check("Sprinklered", "Sprinklers and voice alarm: 0.15 in of egress width an occupant (else 0.2)", true);
        let ok = f.show("The room's last set-up is replaced. The front is the room's lower side along its longest wall.", "Set Out");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let mut args = json!({ "room_id": room, "style": styles[style.get_selection().unwrap_or(0).min(5) as usize], "sprinklered": sprinklered.get_value() });
        if round.get_selection() == Some(1) {
            args["round"] = json!(60);
        }
        if let Some(v) = num(&clear) {
            args["perimeter"] = json!(v);
        }
        if let Some(v) = num(&stage) {
            args["stage"] = json!(v);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        match self.run("seating.layout", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let n = |k: &str| v[k].as_f64().unwrap_or(0.0);
                let mut text = format!(
                    "{} tables, {} seats ({:.1} ft² each)\nIBC occupant load {}; designed for {}: {} exit(s) needed, {} door(s); egress width {:.0} in needed, {:.0} in in the doors",
                    v["tables"], v["seats"], n("per_seat"), v["code_load"], v["design_load"], v["exits_needed"], v["doors"], n("width_needed"), n("width")
                );
                for f in v["findings"].as_array().into_iter().flatten() {
                    text.push_str(&format!("\n• {}: {}", f["rule"].as_str().unwrap_or(""), f["message"].as_str().unwrap_or("")));
                }
                MessageDialog::builder(&frame, &text, "Seating Layout").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build().show_modal();
            }
            Err(e) => error(&frame, &format!("The room was not set out:\n{e}")),
        }
    }
}
