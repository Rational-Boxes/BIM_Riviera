// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Modular units (ARC-GRP-004): Create Unit Type makes the selection a unit type with its bay,
//! its corner clicked in a plan; Unit Array repeats a type along a corridor clicked start to end,
//! on a range of levels; Connect Units runs every copy's open connectors into the selected mains.

use serde_json::json;
use wxdragon::prelude::*;

use super::{Gui, error};

impl Gui {
    /// Architecture › Model › Create Unit Type: the bay's size, then its corridor-face corner.
    pub(crate) fn unit_type(&mut self) {
        let frame = self.frame;
        let ids: Vec<u64> = self.selection().into_iter().map(|id| id.0).collect();
        if ids.is_empty() {
            return error(&frame, "Select the unit's elements first: its rooms, walls, floors, furniture and terminals.");
        }
        let f = super::reinforcement_ui::Form::new(&frame, "Create Unit Type");
        let name = f.text("Name", "Guest room", "The unit type's name");
        let width = f.text("Bay width (m)", "4.0", "Along the corridor face: the pitch between copies");
        let depth = f.text("Bay depth (m)", "6.0", "From the corridor face to the façade face");
        let turn = f.text("Corridor face direction (°)", "0", "The corridor face runs this way from the corner; the unit lies to its left");
        let keep = f.check("Keep", "Leave the selected elements in place (otherwise they become the type)", false);
        let ok = f.show("Then click the bay's corner where its corridor face starts.", "Pick the Corner");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let args = json!({
            "element_ids": ids,
            "name": name.get_value().trim(),
            "width": num(&width).unwrap_or(4.0),
            "depth": num(&depth).unwrap_or(6.0),
            "rotation": num(&turn).unwrap_or(0.0).to_radians(),
            "keep": keep.get_value(),
        });
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking("unit.create_type", vec![json!({ "key": "origin", "kind": "at", "prompt": "Create Unit Type: click the bay's corner on its corridor face" })], args);
    }

    pub(crate) fn unit_type_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let at = args["origin"].take();
        args["origin"] = at["point"].clone();
        match self.run("unit.create_type", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let took: Vec<String> = v["members"].as_object().map(|m| m.iter().map(|(k, n)| format!("{n} {k}")).collect()).unwrap_or_default();
                frame.set_status_text(&format!("Unit type made of {}", took.join(", ")), 0);
            }
            Err(e) => error(&frame, &format!("The unit type was not made:\n{e}")),
        }
    }

    /// Architecture › Model › Unit Array: the type, levels and layout, then the corridor's line.
    pub(crate) fn unit_array(&mut self) {
        let frame = self.frame;
        let types: Vec<String> = self.run("unit.types", json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|t| t["name"].as_str().map(str::to_owned)).collect();
        if types.is_empty() {
            return error(&frame, "No unit types: select a unit's elements and choose Create Unit Type.");
        }
        let levels: Vec<(String, serde_json::Value)> = self.run("level.list", json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().map(|l| (l["name"].as_str().unwrap_or_default().to_owned(), l["id"].clone())).collect();
        let names: Vec<String> = levels.iter().map(|(n, _)| n.clone()).collect();
        let f = super::reinforcement_ui::Form::new(&frame, "Unit Array");
        let ty = f.choice("Unit type", &types, 0);
        let from = f.choice("From level", &names, 1.min(names.len().saturating_sub(1)));
        let to = f.choice("To level", &names, names.len().saturating_sub(1));
        let sides = f.choice("Sides", &["Left of the line".to_owned(), "Right of the line".to_owned(), "Both sides".to_owned()], 2);
        let corridor = f.text("Corridor width (m)", "1.8", "The bays start half of it each side of the line");
        let start = f.text("First bay at (m)", "0", "Along the line from its start");
        let exclusions = f.text("Skip (m)", "", "Stretches along the line to leave out, as 12-18, 30-34 (cores, stairs)");
        let mirror = f.check("Handing", "Mirror every other copy so neighbours share walls and services", true);
        let variants: Vec<String> = std::iter::once("None".to_owned()).chain(types.iter().cloned()).collect();
        let variant = f.choice("Every n-th copy", &variants, 0);
        let every = f.text("n", "4", "The variant's interval on each level, from the first copy");
        let ok = f.show("Then click the corridor's centre line, start and end. Copies are numbered by floor (201, 202, …).", "Pick the Line");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let (a, b) = (from.get_selection().unwrap_or(0) as usize, to.get_selection().unwrap_or(0) as usize);
        let chosen: Vec<serde_json::Value> = levels.iter().enumerate().filter(|(k, _)| (a.min(b)..=a.max(b)).contains(k)).map(|(_, (_, id))| id.clone()).collect();
        let skip: Vec<[f64; 2]> = exclusions
            .get_value()
            .split(',')
            .filter_map(|r| {
                let (x, y) = r.trim().split_once('-')?;
                Some([x.trim().parse().ok()?, y.trim().parse().ok()?])
            })
            .collect();
        let side = ["left", "right", "both"][sides.get_selection().unwrap_or(2).min(2) as usize];
        let mut args = json!({
            "unit_type": types[ty.get_selection().unwrap_or(0) as usize],
            "levels": chosen,
            "sides": side,
            "corridor": num(&corridor).unwrap_or(0.0),
            "start": num(&start).unwrap_or(0.0),
            "exclusions": skip,
            "handing": if mirror.get_value() { "alternate" } else { "none" },
        });
        let v = variant.get_selection().unwrap_or(0) as usize;
        if v > 0 {
            args["variants"] = json!([{ "unit_type": variants[v], "every": every.get_value().trim().parse::<u32>().unwrap_or(4).max(1) }]);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking("unit.array", vec![json!({ "key": "from", "kind": "at", "prompt": "Unit Array: click the corridor line's start" }), json!({ "key": "to", "kind": "point", "prompt": "Unit Array: click its end" })], args);
    }

    pub(crate) fn unit_array_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let from = args["from"].take();
        let to = args["to"].take();
        args.as_object_mut().expect("an object").remove("from");
        args.as_object_mut().expect("an object").remove("to");
        args["path"] = json!([from["point"], to]);
        match self.run("unit.array", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let per: Vec<String> = v["per_type"].as_object().map(|m| m.iter().map(|(k, n)| format!("{n} {k}")).collect()).unwrap_or_default();
                let mut text = format!("{} copies: {}", v["copies"], per.join(", "));
                let un = v["unmerged"].as_array().cloned().unwrap_or_default();
                if !un.is_empty() {
                    text.push_str(&format!("\n\nFound nothing to merge with ({}):", un.len()));
                    for u in un.iter().take(20).filter_map(|x| x.as_str()) {
                        text.push_str("\n• ");
                        text.push_str(u);
                    }
                }
                MessageDialog::builder(&frame, &text, "Unit Array").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build().show_modal();
            }
            Err(e) => error(&frame, &format!("The units were not placed:\n{e}")),
        }
    }

    /// Architecture › Model › Connect Units: every array's copies into the selected mains.
    pub(crate) fn unit_connect(&mut self) {
        let frame = self.frame;
        let Some(store) = self.store_now() else { return };
        let spines: Vec<u64> = self.selection().into_iter().filter(|id| store.get(*id).and_then(riviera_core::mep_runs::segment_recipe).is_some()).map(|id| id.0).collect();
        if spines.is_empty() {
            return error(&frame, "Select the mains to connect to: straight level ducts and pipes along the corridors.");
        }
        let arrays: Vec<u64> = store.iter().filter(|e| riviera_core::modular_units::array_recipe(e).is_some()).map(|e| e.id().0).collect();
        if arrays.is_empty() {
            return error(&frame, "No unit arrays: place some with Unit Array.");
        }
        let (mut branches, mut lost) = (0, Vec::new());
        for a in arrays {
            match self.run("unit.connect", json!({ "array_id": a, "spines": spines })) {
                Ok(v) => {
                    branches += v["branches"].as_u64().unwrap_or(0);
                    lost.extend(v["unconnected"].as_array().cloned().unwrap_or_default());
                }
                Err(e) => return error(&frame, &format!("The units were not connected:\n{e}")),
            }
        }
        self.refresh_scene(&frame);
        let mut text = format!("{branches} branches into the mains");
        if !lost.is_empty() {
            text.push_str(&format!("; {} not connected:", lost.len()));
            for u in lost.iter().take(15) {
                text.push_str(&format!("\n• {} {}: {}", u["number"].as_str().unwrap_or("?"), u["connector"].as_str().unwrap_or("?"), u["reason"].as_str().unwrap_or("")));
            }
        }
        MessageDialog::builder(&frame, &text, "Connect Units").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build().show_modal();
    }
}
