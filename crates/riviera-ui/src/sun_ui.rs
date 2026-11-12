// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Sun-control baffles (ARC-GLZ-019): Baffles fills a region clicked corner to corner with
//! blades; Sun Check reports the direct sun the selected pools and rooms get through them. PV Array
//! (MEP-ELEC-022) fills a roof with photovoltaic modules round what stands on it.

use serde_json::json;
use wxdragon::prelude::*;

use super::{Gui, error};

impl Gui {
    /// Architecture › Build › Baffles: the blades, then two opposite corners of their region.
    pub(crate) fn baffles(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Baffles");
        let height = f.text("Blades' tops above the level (m)", "6.0", "Under (or over) the glazed roof");
        let direction = f.text("Direction (°)", "0", "The blades run this way in plan (0 is along x)");
        let spacing = f.text("Spacing (m)", "0.6", "Between the blades");
        let depth = f.text("Depth (m)", "0.4", "Each blade's depth");
        let tilt = f.text("Tilt (°)", "0", "From vertical; tilted blades cut the high sun");
        let ok = f.show("Then click two opposite corners of the region the blades fill. Sun Check shows what they let through.", "Pick the Region");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let args = json!({
            "height": num(&height).unwrap_or(6.0),
            "direction": num(&direction).unwrap_or(0.0).to_radians(),
            "spacing": num(&spacing).unwrap_or(0.6),
            "depth": num(&depth).unwrap_or(0.4),
            "tilt": num(&tilt).unwrap_or(0.0).to_radians(),
        });
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking("shading.baffles", vec![json!({ "key": "from", "kind": "at", "prompt": "Baffles: click a corner of the region" }), json!({ "key": "to", "kind": "point", "prompt": "Baffles: click the opposite corner" })], args);
    }

    pub(crate) fn baffles_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let from = args["from"].take();
        let to = args["to"].take();
        let (a, b) = ([from["point"][0].as_f64().unwrap_or(0.0), from["point"][1].as_f64().unwrap_or(0.0)], [to[0].as_f64().unwrap_or(0.0), to[1].as_f64().unwrap_or(0.0)]);
        let o = args.as_object_mut().expect("an object");
        o.remove("from");
        o.remove("to");
        args["level"] = from["level"].clone();
        args["outline"] = json!([[a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])], [a[0].min(b[0]), a[1].max(b[1])]]);
        match self.run("shading.baffles", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                frame.set_status_text(&format!("{} blades, {:.0} m; no direct sun beyond {:.0}° and {:.0}° from vertical across them", v["blades"], v["length"].as_f64().unwrap_or(0.0), v["cutoff"][0].as_f64().unwrap_or(0.0), v["cutoff"][1].as_f64().unwrap_or(0.0)), 0);
            }
            Err(e) => error(&frame, &format!("The baffles were not made:\n{e}")),
        }
    }

    /// View › Coordination › Sun Check: the selected pools and rooms.
    pub(crate) fn sun_check(&mut self) {
        let frame = self.frame;
        let Some(store) = self.store_now() else { return };
        let areas: Vec<u64> = self.selection().into_iter().filter(|id| store.get(*id).is_some_and(|e| riviera_core::pools::recipe(e).is_some() || riviera_core::rooms::room_recipe(e).is_some())).map(|id| id.0).collect();
        if areas.is_empty() {
            return error(&frame, "Select the pools and rooms to check.");
        }
        match self.run("shading.sun_check", json!({ "elements": areas })) {
            Ok(v) => {
                let mut text = format!("At {:.1}°, {:.1}° on the solstices and equinoxes; at most {:.0} % of the direct sun:", v["latitude"].as_f64().unwrap_or(0.0), v["longitude"].as_f64().unwrap_or(0.0), v["max_share"].as_f64().unwrap_or(0.0) * 100.0);
                for a in v["areas"].as_array().into_iter().flatten() {
                    text.push_str(&format!(
                        "\n• {}: {:.0} % ({}), the most {:.0} % at {:.1} h on {}/{}",
                        a["name"].as_str().unwrap_or("?"),
                        a["share"].as_f64().unwrap_or(0.0) * 100.0,
                        if a["passes"] == true { "passes" } else { "too much" },
                        a["peak"].as_f64().unwrap_or(0.0) * 100.0,
                        a["peak_at"][1].as_f64().unwrap_or(0.0),
                        a["peak_at"][0][1],
                        a["peak_at"][0][0],
                    ));
                }
                for n in v["notes"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
                    text.push_str("\n\n");
                    text.push_str(n);
                }
                MessageDialog::builder(&frame, &text, "Sun Check").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build().show_modal();
            }
            Err(e) => error(&frame, &format!("The sun was not checked:\n{e}")),
        }
    }
}

impl Gui {
    /// Systems › Electrical › PV Array (MEP-ELEC-022): the modules and clearances, then a click on
    /// the roof's level.
    pub(crate) fn pv_array(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "PV Array");
        let watts = f.text("Module (W)", "400", "1.722 × 1.134 m modules");
        let tilt = f.text("Tilt (°)", "10", "Facing the equator; rows spaced clear of each other's winter shadow");
        let setback = f.text("Setback from the edge (m)", "1.22", "A 4 ft fire-service pathway");
        let clearance = f.text("Clear of equipment (m)", "1.2", "Round roofs, glazing, plant and lift overruns standing on the roof");
        let ok = f.show("Then click anywhere on the roof: its floors are filled.", "Pick the Roof");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let args = json!({
            "module": { "width": 1.134, "length": 1.722, "watts": num(&watts).unwrap_or(400.0) },
            "tilt": num(&tilt).unwrap_or(10.0),
            "setback": num(&setback).unwrap_or(1.22),
            "clearance": num(&clearance).unwrap_or(1.2),
        });
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking("pv.array", vec![json!({ "key": "at", "kind": "at", "prompt": "PV Array: click on the roof" })], args);
    }

    pub(crate) fn pv_array_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let at = args["at"].take();
        args.as_object_mut().expect("an object").remove("at");
        args["level"] = at["level"].clone();
        match self.run("pv.array", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let mut text = format!("{} modules in {} rows: {:.1} kWp over {:.0} m², rows {:.2} m apart; kept clear of {} things on the roof", v["modules"], v["rows"], v["kwp"].as_f64().unwrap_or(0.0), v["area"].as_f64().unwrap_or(0.0), v["row_pitch"].as_f64().unwrap_or(0.0), v["kept_clear_of"].as_array().map_or(0, Vec::len));
                for n in v["notes"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
                    text.push_str("\n• ");
                    text.push_str(n);
                }
                MessageDialog::builder(&frame, &text, "PV Array").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build().show_modal();
            }
            Err(e) => error(&frame, &format!("The array was not placed:\n{e}")),
        }
    }
}
