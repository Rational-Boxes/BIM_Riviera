// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Sump (MEP-PIPE-011): groundwater control for a basement, its pit clicked in the basement's
//! plan.

use serde_json::json;
use wxdragon::prelude::*;

use super::{Gui, error};

impl Gui {
    /// Systems › Plumbing › Sump: the inflow, then the pit's place on the basement's floor.
    pub(crate) fn sump(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Sump");
        let inflow = f.text("Groundwater inflow (L/s)", "", "From the site investigation; empty: 0.01 L/s a metre of drain");
        let starts = f.text("Pump starts an hour", "10", "The pit holds the pumps to this many starts");
        let depth = f.text("Pit depth (m)", "1.2", "Below the basement's floor");
        let drain = f.check("Perimeter drain", "A subsoil drain round the basement outside its walls, branched into the pit", true);
        let ok = f.show("Then click where the pit goes on the basement's floor. Duty and standby pumps lift the water to the storm drain.", "Pick the Pit");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let mut args = json!({ "drain": drain.get_value() });
        if let Some(v) = num(&inflow) {
            args["inflow"] = json!(v);
        }
        if let Some(v) = num(&starts) {
            args["starts"] = json!(v);
        }
        if let Some(v) = num(&depth) {
            args["depth"] = json!(v);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking("sump.create", vec![json!({ "key": "at", "kind": "at", "prompt": "Sump: click where the pit goes" })], args);
    }

    pub(crate) fn sump_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let at = args["at"].take();
        args.as_object_mut().expect("an object").remove("at");
        args["point"] = at["point"].clone();
        args["level"] = at["level"].clone();
        match self.run("sump.create", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let n = |k: &str| v[k].as_f64().unwrap_or(0.0);
                let mut text = format!(
                    "{:.0} m of drain, {:.2} L/s in ({:.2} L/s design)\nA {:.1} m square pit {:.1} m deep, {:.2} m³ working\nDuty and standby pumps: {:.2} kW at {:.1} m head",
                    n("drain_length"), n("inflow"), n("design_flow"), n("pit_size"), n("pit_depth"), n("working_volume"), n("motor"), n("head")
                );
                for note in v["notes"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
                    text.push_str("\n• ");
                    text.push_str(note);
                }
                MessageDialog::builder(&frame, &text, "Sump").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build().show_modal();
            }
            Err(e) => error(&frame, &format!("The sump was not placed:\n{e}")),
        }
    }
}
