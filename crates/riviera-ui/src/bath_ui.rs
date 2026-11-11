// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Check Bathrooms (ARC-BATH-002): runs `bath.check` under the bathroom rule packs in use, lists
//! the findings (errors first), and selects and frames the fixture and what is in its way. Place
//! Grab Bars (ARC-BATH-003) puts the bars the packs ask for at the selected fixtures, or all.

use serde_json::json;
use wxdragon::prelude::*;

use super::{Gui, error};

impl Gui {
    pub(crate) fn check_bathrooms(&mut self) {
        let frame = self.frame;
        let in_use: Vec<String> = self
            .run("bath.rule_packs", json!({}))
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter(|p| p["in_use"] == true)
            .filter_map(|p| p["title"].as_str().map(str::to_owned))
            .collect();
        if in_use.is_empty() {
            return error(
                &frame,
                "No bathroom rule packs are in use. Choose them in Manage › Model Properties, or set a building code or accessibility rule set they follow.",
            );
        }
        let found = match self.run("bath.check", json!({})) {
            Ok(v) => v.as_array().cloned().unwrap_or_default(),
            Err(e) => return error(&frame, &format!("The check did not run:\n{e}")),
        };
        self.show_findings("Check Bathrooms", "bath.check", &in_use.join("; "), &found);
    }

    /// Check Rules (ARC-RULE-002): furniture's use zones under the shared rule packs in use.
    pub(crate) fn check_rules(&mut self) {
        let frame = self.frame;
        let in_use: Vec<String> = self
            .run("rules.packs", json!({}))
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter(|p| p["in_use"] == true && p["domain"] != "bath" && p["domain"] != "vt")
            .filter_map(|p| p["title"].as_str().map(str::to_owned))
            .collect();
        if in_use.is_empty() {
            return error(
                &frame,
                "No interiors rule packs are in use. Choose them in Manage › Model Properties, or set the accessibility rule set they follow.",
            );
        }
        let found = match self.run("rules.check", json!({})) {
            Ok(v) => v.as_array().cloned().unwrap_or_default(),
            Err(e) => return error(&frame, &format!("The check did not run:\n{e}")),
        };
        self.show_findings("Check Rules", "rules.check", &in_use.join("; "), &found);
    }

    /// Lists a check's findings, after a first row that saves them as a report (DOC-OUT-016); the
    /// one picked is selected and framed with what is in its way.
    fn show_findings(&mut self, title: &str, command: &str, packs: &str, found: &[serde_json::Value]) {
        let frame = self.frame;
        if found.is_empty() {
            let d = MessageDialog::builder(&frame, &format!("No problems found under: {packs}.\n\nSave a report of this check?"), title)
                .with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconInformation)
                .build();
            if d.show_modal() == ID_YES {
                self.save_report(command, json!({}), title);
            }
            return;
        }
        let rows: Vec<String> = std::iter::once("Save Report…".to_owned())
            .chain(found.iter().enumerate().map(|(k, f)| {
                let severity = if f["severity"] == "note" { "Note" } else { "Error" };
                let mut row = format!("{}. {severity}: {}", k + 1, f["message"].as_str().unwrap_or_default());
                if f["confidence"] != "verified" {
                    row.push_str(&format!(" [{}]", f["confidence"].as_str().unwrap_or_default()));
                }
                row
            }))
            .collect();
        let labels: Vec<&str> = rows.iter().map(String::as_str).collect();
        let errors = found.iter().filter(|f| f["severity"] == "error").count();
        let dialog = SingleChoiceDialog::builder(
            &frame,
            &format!(
                "{errors} error(s) and {} note(s) under: {packs}.\nPick one to select and frame it, or save them all as a report:",
                found.len() - errors
            ),
            title,
            &labels,
        )
        .build();
        if dialog.show_modal() != ID_OK {
            return;
        }
        let picked = usize::try_from(dialog.get_selection()).ok();
        dialog.destroy();
        if picked == Some(0) {
            return self.save_report(command, json!({}), title);
        }
        let Some(f) = picked.and_then(|i| found.get(i.checked_sub(1)?)) else {
            return;
        };
        // Show the clear spaces, then the fixture and what is in its way.
        if let Some(v) = self.active_view()
            && self.virtual_mode(v) == riviera_render::VirtualMode::Hidden
            && let Some(view) = v.element
        {
            let _ = self.run("view.set_virtual_volumes", json!({ "view_id": view, "mode": "translucent" }));
            self.refresh_scene(&frame);
        }
        let mut ids = vec![f["element"].clone()];
        if !f["other"].is_null() {
            ids.push(f["other"].clone());
        }
        let _ = self.run("selection.set", json!({ "ids": ids }));
        self.selection_changed();
        self.zoom_to_fit();
        frame.set_status_text(
            &format!(
                "{} — {} ({}, {})",
                f["rule"].as_str().unwrap_or_default(),
                f["description"].as_str().unwrap_or_default(),
                f["edition"].as_str().unwrap_or_default(),
                f["source"].as_str().unwrap_or_default()
            ),
            0,
        );
    }

    /// Saves a check or report as a file (DOC-OUT-016): on this computer, or in a FileEngine
    /// folder when one is connected; HTML, Markdown, CSV or JSON by the name's extension.
    pub(crate) fn save_report(&mut self, command: &str, args: serde_json::Value, title: &str) {
        let frame = self.frame;
        let stem: String = title.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
        let default = format!("{}.html", stem.trim_matches('-'));
        let wildcard = "HTML report (*.html)|*.html|Markdown (*.md)|*.md|CSV table (*.csv)|*.csv|JSON (*.json)|*.json";
        let Some(path) = self.save_target(&format!("Save Report: {title}"), wildcard, &default) else {
            return;
        };
        match self.run("report.write", json!({ "command": command, "args": args, "path": path, "title": title })) {
            Ok(v) => frame.set_status_text(&format!("Report saved: {} ({} rows)", path, v["rows"].as_u64().unwrap_or(0)), 0),
            Err(e) => error(&frame, &format!("The report was not saved:\n{e}")),
        }
    }

    /// A result shown as a message, with Save Report… beside Close (DOC-OUT-016): the report is the
    /// command's result, written by `report.write`.
    pub(crate) fn report_message(&mut self, title: &str, message: &str, command: &str, args: serde_json::Value) {
        let frame = self.frame;
        let d = MessageDialog::builder(&frame, message, title).with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconInformation).build();
        d.set_yes_no_labels("Save Report…", "Close");
        if d.show_modal() == ID_YES {
            self.save_report(command, args, title);
        }
    }

    /// Findings to pick from, after a first row that saves them all as a report (DOC-OUT-016);
    /// the finding picked, by index.
    pub(crate) fn pick_or_report(&mut self, title: &str, prompt: &str, rows: &[String], command: &str, args: serde_json::Value) -> Option<usize> {
        let frame = self.frame;
        let labels: Vec<&str> = std::iter::once("Save Report…").chain(rows.iter().map(String::as_str)).collect();
        let dialog = SingleChoiceDialog::builder(&frame, &format!("{prompt}\n(or save them all as a report)"), title, &labels).build();
        if dialog.show_modal() != ID_OK {
            return None;
        }
        let picked = usize::try_from(dialog.get_selection()).ok();
        dialog.destroy();
        if picked == Some(0) {
            self.save_report(command, args, title);
            return None;
        }
        picked.and_then(|i| i.checked_sub(1))
    }

    /// Manage › Settings › Save Report… (DOC-OUT-016): any of the checks and reports, saved.
    pub(crate) fn save_any_report(&mut self) {
        let frame = self.frame;
        let list = self.run("report.list", json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
        if list.is_empty() {
            return frame.set_status_text("Open a model first", 0);
        }
        let labels: Vec<String> = list.iter().map(|r| r["label"].as_str().unwrap_or_default().to_owned()).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let d = SingleChoiceDialog::builder(&frame, "Which check or report?", "Save Report", &refs).build();
        if d.show_modal() != ID_OK {
            return;
        }
        let picked = usize::try_from(d.get_selection()).ok();
        d.destroy();
        let Some(r) = picked.and_then(|i| list.get(i)) else { return };
        let (command, label) = (r["command"].as_str().unwrap_or_default().to_owned(), labels[picked.unwrap_or(0)].clone());
        self.save_report(&command, json!({}), &label);
    }

    pub(crate) fn place_grab_bars(&mut self) {
        let frame = self.frame;
        let selected: Vec<serde_json::Value> = self.selection().into_iter().map(|id| json!(id.0)).collect();
        let args = if selected.is_empty() { json!({}) } else { json!({ "element_ids": selected }) };
        let out = match self.run("bath.place_grab_bars", args) {
            Ok(v) => v,
            Err(e) => return error(&frame, &format!("Grab bars were not placed:\n{e}")),
        };
        self.refresh_scene(&frame);
        let placed = out["placed"].as_array().map_or(0, Vec::len);
        let skipped: Vec<String> = out["skipped"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|s| format!("{} ({}): {}", self.element_label(&s["fixture"]), s["rule"].as_str().unwrap_or_default(), s["reason"].as_str().unwrap_or_default()))
            .collect();
        if placed == 0 && skipped.is_empty() {
            return error(&frame, "No grab bars to place: the bathroom rule packs in use ask for none at these fixtures (the ADA pack places WC bars).");
        }
        if skipped.is_empty() {
            frame.set_status_text(&format!("{placed} grab bar(s) placed"), 0);
        } else {
            let d = MessageDialog::builder(&frame, &format!("{placed} grab bar(s) placed. Not placed:\n{}", skipped.join("\n")), "Place Grab Bars")
                .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
                .build();
            d.show_modal();
        }
    }

    /// Architecture › Build › Bathroom (ARC-BATH-004): a generic bathroom fixture or accessory,
    /// loaded the first time; each click near a wall puts one on its face at its own height.
    pub(crate) fn place_bathroom_component(&mut self) {
        let frame = self.frame;
        let components = |store: &riviera_core::Store| -> Vec<(riviera_core::ElementId, String)> {
            store.iter().filter(|e| riviera_core::bath_components::is_component(e)).map(|e| (e.id(), e.name.clone().unwrap_or_default())).collect()
        };
        let Some(store) = self.store_now() else {
            return frame.set_status_text("Open or create a project first (File > New)", 0);
        };
        let mut types = components(&store);
        if types.len() < riviera_core::bath_components::names().len() {
            if let Err(e) = self.run("bath.load_components", json!({})) {
                return error(&frame, &e);
            }
            let Some(store) = self.store_now() else { return };
            types = components(&store);
        }
        // In the catalogue's order: fixtures, then accessories.
        let order = riviera_core::bath_components::names();
        types.sort_by_key(|t| order.iter().position(|n| *n == t.1).unwrap_or(usize::MAX));
        let f = super::reinforcement_ui::Form::new(&frame, "Place Bathroom Component");
        let names: Vec<String> = types.iter().map(|t| t.1.clone()).collect();
        let ty = f.choice("Component", &names, 0);
        let height = f.text("Raise by (mm)", "0", "Above its own mounting height");
        let ok = f.show("Click near a wall to put it on that face, its back to the wall (it follows the wall); away from walls it stands on the work plane. Spacebar turns it there.", "Place");
        let k = ty.get_selection().map_or(0, |k| k as usize);
        let h = height.get_value().trim().replace(',', ".").parse::<f64>().unwrap_or(0.0) / 1000.0;
        f.dialog.destroy();
        let Some((id, name)) = types.get(k).cloned().filter(|_| ok) else { return };
        self.start_place_tool(super::components::Source::Type(id), name);
        self.set_place_offset(h);
    }

    /// Architecture › Build › Bathroom Layout (ARC-BATH-005): a template and its options, then a
    /// click inside a room closed by walls lays it out there.
    pub(crate) fn bathroom_layout(&mut self) {
        let frame = self.frame;
        let templates = match self.run("bath.layouts", json!({})) {
            Ok(v) => v.as_array().cloned().unwrap_or_default(),
            Err(e) => return error(&frame, &e),
        };
        let f = super::reinforcement_ui::Form::new(&frame, "Bathroom Layout");
        let titles: Vec<String> = templates.iter().map(|t| t["title"].as_str().unwrap_or_default().to_owned()).collect();
        let t = f.choice("Template", &titles, 0);
        let start = f.choice("Start from", &["The left end, facing the wet wall".to_owned(), "The right end".to_owned()], 0);
        let replace = f.choice("An earlier layout here", &["Replace it".to_owned(), "Keep it".to_owned()], 0);
        let ok = f.show("Then click inside a room closed by walls, in a plan: the fixtures run along its longer wall, spaced by the bathroom rule packs in use.", "Choose Room");
        let k = t.get_selection().map_or(0, |k| k as usize);
        let right = start.get_selection() == Some(1);
        let keep = replace.get_selection() == Some(1);
        f.dialog.destroy();
        let Some(id) = templates.get(k).and_then(|t| t["id"].as_str()).filter(|_| ok).map(str::to_owned) else { return };
        self.start_room_tool();
        if let Some(tool) = self.room_tool.as_mut() {
            tool.layout = Some(json!({ "template": id, "start": if right { "right" } else { "left" }, "replace": !keep }));
            frame.set_status_text("Bathroom Layout: click inside the room; Esc ends", 0);
        }
    }

    /// Architecture › Build › Room Layout (ARC-INT-001): a bedroom, living or dining room,
    /// office or meeting room furnished from a template, fitted with the interiors rule packs'
    /// use zones; then a click inside the room.
    pub(crate) fn room_layout(&mut self) {
        let frame = self.frame;
        let templates = match self.run("interior.layouts", json!({})) {
            Ok(v) => v.as_array().cloned().unwrap_or_default(),
            Err(e) => return error(&frame, &e),
        };
        let f = super::reinforcement_ui::Form::new(&frame, "Room Layout");
        let titles: Vec<String> = templates.iter().map(|t| t["title"].as_str().unwrap_or_default().to_owned()).collect();
        let t = f.choice("Template", &titles, 0);
        let replace = f.choice("An earlier layout here", &["Replace it".to_owned(), "Keep it".to_owned()], 0);
        let ok = f.show("Then click inside a room closed by walls, in a plan. The main piece (a bed's head, a sofa's back, a desk) goes against one of the longer walls; every piece and its use zones must fit.", "Choose Room");
        let k = t.get_selection().map_or(0, |k| k as usize);
        let keep = replace.get_selection() == Some(1);
        f.dialog.destroy();
        let Some(id) = templates.get(k).and_then(|t| t["id"].as_str()).filter(|_| ok).map(str::to_owned) else { return };
        self.start_room_tool();
        if let Some(tool) = self.room_tool.as_mut() {
            tool.layout = Some(json!({ "command": "interior.layout", "template": id, "replace": !keep }));
            frame.set_status_text("Room Layout: click inside the room; Esc ends", 0);
        }
    }

    /// Architecture › Build › Decorative Lighting (ARC-INT-003): sconces along the selected wall,
    /// or a chandelier or pendants over the selected table or counter; then Check Rules.
    pub(crate) fn decorative_lighting(&mut self) {
        let frame = self.frame;
        let Some(store) = self.store_now() else { return frame.set_status_text("Open a model first", 0) };
        let sel = self.selection();
        let Some(e) = (sel.len() == 1).then(|| store.get(sel[0])).flatten() else {
            return error(&frame, "Select one wall (for sconces), or one table or counter (for a chandelier or pendants).");
        };
        let level_of = |guid: &str| riviera_core::GlobalId::parse(guid).and_then(|g| store.find_by_guid(&g)).map(|l| l.0);
        let (command, args) = if let Some(w) = riviera_core::walls::wall_recipe(e) {
            ("lighting.sconces", json!({ "wall_id": e.id().0, "level": level_of(&w.base_level) }))
        } else if let Some(r) = riviera_core::mesh_objects::mesh_object_recipe(e) {
            let d = SingleChoiceDialog::builder(&frame, "Over the selected piece:", "Decorative Lighting", &["A chandelier, sized from the room and the piece", "Pendants in a row along it"]).build();
            if d.show_modal() != ID_OK {
                return;
            }
            let pick = d.get_selection();
            d.destroy();
            (if pick == 1 { "lighting.pendants" } else { "lighting.chandelier" }, json!({ "over_id": e.id().0, "level": level_of(&r.level) }))
        } else {
            return error(&frame, "Select one wall (for sconces), or one table or counter (for a chandelier or pendants).");
        };
        match self.run(command, args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let n = v["placed"].as_array().map_or(0, Vec::len);
                let problems = self.run("rules.check", json!({ "element_ids": v["placed"] })).ok().and_then(|f| f.as_array().map(Vec::len)).unwrap_or(0);
                let notes: Vec<&str> = v["notes"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
                frame.set_status_text(&format!("{n} × {}, bottom {:.0} mm; Check Rules finds {problems}{}", v["item"].as_str().unwrap_or(""), v["bottom"].as_f64().unwrap_or(0.0) * 1000.0, if notes.is_empty() { String::new() } else { format!("; {}", notes.join("; ")) }), 0);
            }
            Err(e) => error(&frame, &format!("The lighting was not placed:\n{e}")),
        }
    }

    /// Architecture › Build › Cabinet Run (ARC-KIT-001): base, wall or tall units along a back
    /// line clicked from start to end along a wall.
    pub(crate) fn cabinet_run(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Cabinet Run");
        let kind = f.choice("Units", &["Base units".to_owned(), "Wall units".to_owned(), "Tall units".to_owned()], 0);
        let region = f.choice("Sizes", &["From the building code".to_owned(), "US (inches)".to_owned(), "EU (600 mm modules)".to_owned()], 0);
        let ok = f.show("Then click the run's start and end along the wall's face, in a plan: the units face away from the wall, in standard widths, with a filler for what is left.", "Pick the Run");
        let k = ["base", "wall", "tall"][kind.get_selection().unwrap_or(0) as usize];
        let r = match region.get_selection() {
            Some(1) => json!("us"),
            Some(2) => json!("eu"),
            _ => serde_json::Value::Null,
        };
        f.dialog.destroy();
        if !ok {
            return;
        }
        let mut args = json!({ "kind": k });
        if !r.is_null() {
            args["region"] = r;
        }
        self.start_picking(
            "cabinet.run",
            vec![json!({ "key": "start", "kind": "at", "prompt": "Cabinet Run: click the run's start on the wall face" }), json!({ "key": "end", "kind": "point", "prompt": "Cabinet Run: click its end" })],
            args,
        );
    }

    /// The run picked: made, and reported.
    pub(crate) fn cabinet_run_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let start = args["start"].take();
        args["start"] = start["point"].clone();
        args["level"] = start["level"].clone();
        match self.run("cabinet.run", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let n = v["units"].as_array().map_or(0, Vec::len);
                frame.set_status_text(&format!("{n} unit(s) over {:.2} m, filler {:.0} mm; Countertop tops base runs", v["length"].as_f64().unwrap_or(0.0), v["filler"].as_f64().unwrap_or(0.0) * 1000.0), 0);
            }
            Err(e) => error(&frame, &format!("The run was not made:\n{e}")),
        }
    }

    /// Architecture › Build › Gondola Run (ARC-RETAIL-001): an island or wall run of gondolas,
    /// clicked from start to end, then the aisles checked.
    pub(crate) fn gondola_run(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Gondola Run");
        let kind = f.choice("Run", &["Island, with end caps (click its centre line)".to_owned(), "Wall (click its back line on the wall face)".to_owned()], 0);
        let section = f.choice("Sections", &["48 in".to_owned(), "36 in".to_owned(), "30 in".to_owned(), "24 in".to_owned()], 0);
        let height = f.text("Height (in)", "", "Default 72 in for an island, 84 in for a wall run");
        let ok = f.show("Then click the run's start and end in a plan. Sections fill the run in standard widths; Check Rules keeps the aisles clear (ADA §403.5, IBC Group M, and practice if chosen).", "Pick the Run");
        let k = ["island", "wall"][kind.get_selection().unwrap_or(0) as usize];
        let w = [48.0, 36.0, 30.0, 24.0][section.get_selection().unwrap_or(0) as usize];
        let h = height.get_value().trim().replace(',', ".").parse::<f64>().ok();
        f.dialog.destroy();
        if !ok {
            return;
        }
        let mut args = json!({ "kind": k, "section": w * 0.0254 });
        if let Some(h) = h {
            args["height"] = json!(h * 0.0254);
        }
        self.start_picking(
            "retail.run",
            vec![json!({ "key": "start", "kind": "at", "prompt": "Gondola Run: click the run's start" }), json!({ "key": "end", "kind": "point", "prompt": "Gondola Run: click its end" })],
            args,
        );
    }

    /// The gondola run picked: made, its aisles checked, and reported.
    pub(crate) fn gondola_run_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let start = args["start"].take();
        args["start"] = start["point"].clone();
        args["level"] = start["level"].clone();
        match self.run("retail.run", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let ids: Vec<serde_json::Value> = v["sections"].as_array().map(|a| a.iter().map(|s| s["element"].clone()).collect()).unwrap_or_default();
                let problems = self.run("rules.check", json!({ "element_ids": ids })).ok().and_then(|f| f.as_array().map(|a| a.iter().filter(|x| x["severity"] == "error").count())).unwrap_or(0);
                frame.set_status_text(&format!("{} fixture(s) over {:.2} m, {:.0} mm left over; Check Rules finds {problems} aisle error(s)", ids.len(), v["overall"].as_f64().unwrap_or(0.0), v["left_over"].as_f64().unwrap_or(0.0) * 1000.0), 0);
            }
            Err(e) => error(&frame, &format!("The run was not made:\n{e}")),
        }
    }

    /// Architecture › Build › Store Area (ARC-RETAIL-002): the selected rooms marked as a sales
    /// floor or stock rooms.
    pub(crate) fn store_area(&mut self) {
        let frame = self.frame;
        let Some(store) = self.store_now() else { return frame.set_status_text("Open a model first", 0) };
        let rooms: Vec<u64> = self.selection().into_iter().filter(|id| store.get(*id).is_some_and(|e| riviera_core::rooms::room_recipe(e).is_some())).map(|id| id.0).collect();
        if rooms.is_empty() {
            return error(&frame, "Select one or more rooms first.");
        }
        let labels = ["Sales floor (60 ft² a person)", "Stock room (300 ft² a person)", "Not part of a store"];
        let d = SingleChoiceDialog::builder(&frame, &format!("The {} selected room(s) are:", rooms.len()), "Store Area", &labels).build();
        if d.show_modal() != ID_OK {
            return;
        }
        let pick = d.get_selection();
        d.destroy();
        let area = match pick {
            0 => json!("sales"),
            1 => json!("stock"),
            _ => serde_json::Value::Null,
        };
        match self.run("retail.set_area", json!({ "room_ids": rooms, "area": area })) {
            Ok(_) => frame.set_status_text(&format!("{} room(s) marked", rooms.len()), 0),
            Err(e) => error(&frame, &format!("The rooms were not marked:\n{e}")),
        }
    }

    /// View › Coordination › Check Store (ARC-RETAIL-002): occupant loads and the storage checks,
    /// the finding picked selected and framed.
    pub(crate) fn check_store(&mut self) {
        let frame = self.frame;
        let c = match self.run("retail.check", json!({})) {
            Ok(v) => v,
            Err(e) => return error(&frame, &e),
        };
        let mut head = vec![format!("{} occupants", c["occupants"])];
        for l in c["levels"].as_array().cloned().unwrap_or_default() {
            head.push(format!("{}: {:.0} m² sales, {:.0} m² stock, {} occupants", l["level"].as_str().unwrap_or_default(), l["sales_area"].as_f64().unwrap_or(0.0), l["stock_area"].as_f64().unwrap_or(0.0), l["occupants"]));
        }
        let findings = c["findings"].as_array().cloned().unwrap_or_default();
        if findings.is_empty() {
            let note = if c["areas"].as_array().is_none_or(Vec::is_empty) { "\n\nNo rooms are marked as store areas (Architecture › Build › Store Area)." } else { "" };
            return self.report_message("Check Store", &format!("{}\n\nThe storage fixtures pass.{note}", head.join("\n")), "retail.check", json!({}));
        }
        let rows: Vec<String> = findings.iter().map(|f| format!("{} {}: {}", if f["severity"] == "error" { "✗" } else { "•" }, f["rule"].as_str().unwrap_or_default(), f["message"].as_str().unwrap_or_default())).collect();
        if let Some(k) = self.pick_or_report("Check Store", &head.join("\n"), &rows, "retail.check", json!({}))
            && let Some(id) = findings[k]["element"].as_u64()
        {
            let _ = self.run("selection.set", json!({ "ids": [id] }));
            self.selection_changed();
            self.zoom_to_fit();
        }
    }

    /// Architecture › Build › Checkout Bank (ARC-RETAIL-003): checkstands or self-checkout lanes
    /// along the store front, clicked from start to end, then a click on the sales floor side.
    pub(crate) fn checkout_bank(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Checkout Bank");
        let items = ["Checkstand, carousel rear 142 × 48 in", "Checkstand, belted take-away 150 × 48 in", "Cash desk, compact 2500 × 1250", "Self-checkout lane"];
        let item = f.choice("Lanes", &items.map(str::to_owned), 0);
        let handing = f.choice("Handing", &["Aisle on the right".to_owned(), "Aisle on the left".to_owned(), "Tandem pairs".to_owned()], 0);
        let function = f.choice("Function", &["Checkout".to_owned(), "Express".to_owned(), "Pharmacy".to_owned()], 0);
        let count = f.text("Lanes", "", "Leave empty for as many as fit");
        let pitch = f.text("Pitch (in)", "84", "Centre to centre: a 48 in stand and a 36 in aisle");
        let ok = f.show("Then click the line the lanes' exit ends stand on, from start to end, and a point on the sales floor. Table 227.2's accessible lanes are spread across the bank.", "Pick the Front");
        let item = items[item.get_selection().unwrap_or(0) as usize];
        let handing = ["right", "left", "paired"][handing.get_selection().unwrap_or(0) as usize];
        let function = ["checkout", "express", "pharmacy"][function.get_selection().unwrap_or(0) as usize];
        let mut args = json!({ "item": item, "handing": handing, "function": function });
        if let Ok(n) = count.get_value().trim().parse::<u32>() {
            args["count"] = json!(n);
        }
        if let Ok(p) = pitch.get_value().trim().replace(',', ".").parse::<f64>() {
            args["pitch"] = json!(p * 0.0254);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking(
            "frontend.layout",
            vec![
                json!({ "key": "start", "kind": "at", "prompt": "Checkout Bank: click the start of the lanes' exit line" }),
                json!({ "key": "end", "kind": "point", "prompt": "Checkout Bank: click its end" }),
                json!({ "key": "toward", "kind": "point", "prompt": "Checkout Bank: click a point on the sales floor" }),
            ],
            args,
        );
    }

    /// The bank picked: laid out, and reported.
    pub(crate) fn checkout_bank_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let start = args["start"].take();
        args["start"] = start["point"].clone();
        args["level"] = start["level"].clone();
        match self.run("frontend.layout", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let lanes = v["lanes"].as_array().cloned().unwrap_or_default();
                let acc = lanes.iter().filter(|l| l["accessible"] == true).count();
                frame.set_status_text(&format!("{} lane(s), {acc} accessible (Table 227.2 asks {}); Check Checkouts counts every bank", lanes.len(), v["accessible_required"]), 0);
            }
            Err(e) => error(&frame, &format!("The bank was not laid out:\n{e}")),
        }
    }

    /// View › Coordination › Check Checkouts (ARC-RETAIL-003): Table 227.2 per function.
    pub(crate) fn check_checkouts(&mut self) {
        let frame = self.frame;
        let c = match self.run("checkout.check", json!({})) {
            Ok(v) => v,
            Err(e) => return error(&frame, &e),
        };
        let functions = c["functions"].as_array().cloned().unwrap_or_default();
        if functions.is_empty() {
            return error(&frame, "No check-out lanes: lay a bank out with Architecture › Build › Checkout Bank.");
        }
        let mut lines: Vec<String> = functions.iter().map(|f| format!("{}: {} lanes, {} accessible, {} needed", f["function"].as_str().unwrap_or_default().replace('_', " "), f["lanes"], f["accessible"], f["required"])).collect();
        if c["small_store"] == true {
            lines.push("Under 5,000 ft² of selling space: one accessible lane is enough.".into());
        }
        let findings = c["findings"].as_array().cloned().unwrap_or_default();
        lines.push(if findings.is_empty() { "\nTable 227.2 is met.".into() } else { String::new() });
        for x in &findings {
            lines.push(format!("{} {}: {}", if x["severity"] == "error" { "✗" } else { "•" }, x["rule"].as_str().unwrap_or_default(), x["message"].as_str().unwrap_or_default()));
        }
        self.report_message("Check Checkouts", &lines.join("\n"), "checkout.check", json!({}));
    }

    /// View › Coordination › Checkout Lanes Estimate (ARC-RETAIL-003): an M/M/c estimate of the lanes a peak
    /// hour needs.
    pub(crate) fn checkout_estimate(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Checkout Lanes Estimate");
        let customers = f.text("Customers at the peak hour", "300", "");
        let basket = f.text("Items per basket", "20", "");
        let rate = f.text("Items a lane scans an hour", "900", "");
        let wait = f.text("Average wait under (min)", "3", "");
        let ok = f.show("An estimate from a queueing model, not a requirement: check it against the operator's experience.", "Estimate");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let args = json!({ "customers_per_hour": num(&customers).unwrap_or(0.0), "items_per_basket": num(&basket).unwrap_or(0.0), "items_per_hour": num(&rate), "target_wait": num(&wait) });
        f.dialog.destroy();
        if !ok {
            return;
        }
        match self.run("checkout.lanes_estimate", args) {
            Ok(e) => {
                let msg = format!("About {} lanes: each busy {:.0} % of the time, an average wait of {:.1} min.\n\n{}", e["lanes"], e["utilisation"].as_f64().unwrap_or(0.0) * 100.0, e["wait"].as_f64().unwrap_or(0.0), e["note"].as_str().unwrap_or_default());
                let d = MessageDialog::builder(&frame, &msg, "Checkout Lanes Estimate").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build();
                d.show_modal();
            }
            Err(e) => error(&frame, &e),
        }
    }

    /// Architecture › Build › Self-checkout Pod (ARC-RETAIL-004): units in two facing rows round a
    /// courtyard, clicked at its open end and then down the pod.
    pub(crate) fn sco_pod(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Self-checkout Pod");
        let items = ["Self-checkout, pedestal", "Self-checkout lane"];
        let item = f.choice("Units", &items.map(str::to_owned), 0);
        let count = f.text("How many", "4", "NCR's quad pod is 4; pods of 4–8 are usual");
        let court = f.text("Courtyard (ft)", "11", "Between the rows' fronts: NCR leaves 10–12 ft");
        let queue = f.text("Shared queue (m)", "3", "Kept clear beyond the open end");
        let attendant = f.check("Attendant", "Attendant station at the closed end", true);
        let ok = f.show("Then click the pod's open end and a point down the pod. Table 227.2's accessible units are spread across both rows.", "Pick the Pod");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let item = items[item.get_selection().unwrap_or(0) as usize];
        let mut args = json!({ "item": item, "attendant": attendant.get_value() });
        if let Ok(n) = count.get_value().trim().parse::<u32>() {
            args["units"] = json!(n);
        }
        if let Some(c) = num(&court) {
            args["courtyard"] = json!(c * 0.3048);
        }
        if let Some(q) = num(&queue) {
            args["queue"] = json!(q);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking(
            "sco.place_pod",
            vec![
                json!({ "key": "entry", "kind": "at", "prompt": "Self-checkout Pod: click the courtyard's open end" }),
                json!({ "key": "toward", "kind": "point", "prompt": "Self-checkout Pod: click a point down the pod" }),
            ],
            args,
        );
    }

    /// The pod picked: placed, and reported.
    pub(crate) fn sco_pod_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let entry = args["entry"].take();
        args["entry"] = entry["point"].clone();
        args["level"] = entry["level"].clone();
        match self.run("sco.place_pod", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let n = v["units"].as_array().map_or(0, Vec::len);
                let acc = v["accessible"].as_array().map_or(0, Vec::len);
                frame.set_status_text(&format!("{n} unit(s), {acc} accessible; state their features with Self-checkout Profile, then Check Self-checkouts"), 0);
            }
            Err(e) => error(&frame, &format!("The pod was not placed:\n{e}")),
        }
    }

    /// Architecture › Build › Self-checkout Profile (ARC-RETAIL-004): the selected units'
    /// accessibility features, as their maker states them.
    pub(crate) fn sco_profile(&mut self) {
        let frame = self.frame;
        let Some(store) = self.store_now() else { return frame.set_status_text("Open a model first", 0) };
        let units: Vec<u64> = self
            .selection()
            .into_iter()
            .filter(|id| store.get(*id).and_then(|e| e.param(riviera_core::checkouts::LANE_KEY)).is_some_and(|t| t["function"] == "self_checkout"))
            .map(|id| id.0)
            .collect();
        if units.is_empty() {
            return error(&frame, "Select one or more self-checkout units first.");
        }
        let f = super::reinforcement_ui::Form::new(&frame, "Self-checkout Profile");
        let lo = f.text("Lowest operable part (mm)", "", "Touch targets, PIN pad, card and cash slots, receipt");
        let hi = f.text("Highest operable part (mm)", "", "");
        let approach = f.choice("Approach", &["Forward".to_owned(), "Side".to_owned()], 0);
        let depth = f.text("Reach over the unit (mm)", "0", "To the highest part, over the deck or shelf");
        let screen = f.text("Screen centre (mm)", "", "1200–1400 mm is recommended");
        let speech = f.check("Speech", "Speech output through a headphone jack", false);
        let keypad = f.check("Tactile keypad", "12 keys with a raised 5", false);
        let braille = f.check("Braille", "Braille instructions to start speech", false);
        let ok = f.show(&format!("The features of {} unit(s), checked against EN 301 549 cl. 8.3 and the Access Board's 2022 ANPRM (advisory).", units.len()), "Set");
        let mm = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok().map(|v| v / 1000.0);
        let approach = ["forward", "side"][approach.get_selection().unwrap_or(0) as usize];
        let mut args = json!({
            "element_ids": units,
            "approach": approach,
            "obstruction_depth": mm(&depth),
            "screen_height": mm(&screen),
            "speech": speech.get_value(),
            "tactile_keypad": keypad.get_value(),
            "braille": braille.get_value(),
        });
        if let (Some(l), Some(h)) = (mm(&lo), mm(&hi)) {
            args["operable"] = json!([l, h]);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        match self.run("sco.set_profile", args) {
            Ok(_) => frame.set_status_text(&format!("{} unit(s) stated", units.len()), 0),
            Err(e) => error(&frame, &format!("The profile was not set:\n{e}")),
        }
    }

    /// View › Coordination › Check Self-checkouts (ARC-RETAIL-004): the pods' practice notes and
    /// the accessibility profile.
    pub(crate) fn check_scos(&mut self) {
        let frame = self.frame;
        let c = match self.run("sco.check", json!({})) {
            Ok(v) => v,
            Err(e) => return error(&frame, &e),
        };
        let pods = c["pods"].as_array().cloned().unwrap_or_default();
        if pods.is_empty() {
            return error(&frame, "No self-checkout pods: place one with Architecture › Build › Self-checkout Pod.");
        }
        let mut lines: Vec<String> = pods
            .iter()
            .map(|p| format!("Pod {}: {} units, {} attendant station(s), {} meeting the profile, courtyard {:.2} m", p["pod"].as_str().unwrap_or_default(), p["units"], p["attendants"], p["meeting_profile"], p["courtyard"].as_f64().unwrap_or(0.0)))
            .collect();
        let findings = c["findings"].as_array().cloned().unwrap_or_default();
        lines.push(if findings.is_empty() { "\nThe pods follow the practice and meet the profile.".into() } else { String::new() });
        for x in &findings {
            lines.push(format!("• {} ({}): {}", x["rule"].as_str().unwrap_or_default(), x["severity"].as_str().unwrap_or_default(), x["message"].as_str().unwrap_or_default()));
        }
        lines.push("\nThere is no fixed US rule for self-checkout: these are practice notes and advice.".into());
        self.report_message("Check Self-checkouts", &lines.join("\n"), "sco.check", json!({}));
    }

    /// Architecture › Build › Wall Mirror (ARC-GYM-001): a mirror along a wall's face, clicked from
    /// start to end, then a point in the room.
    pub(crate) fn gym_mirror(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Wall Mirror");
        let bottom = f.text("Bottom edge (m)", "0.4", "Makers give 12–16 in; above about 0.6 m where there are wall outlets");
        let top = f.text("Top edge (m)", "2.1", "");
        let safety = f.check("Safety glazing", "Tempered glass or a safety-film backing", true);
        let ok = f.show("Then click along the wall's face from start to end, and a point in the room. Sport England keeps mirrors to the free-weight area, away from treadmills.", "Pick the Wall");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let args = json!({ "bottom": num(&bottom), "top": num(&top), "safety_glazing": safety.get_value() });
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking(
            "gym.mirror",
            vec![
                json!({ "key": "start", "kind": "at", "prompt": "Wall Mirror: click the start along the wall's face" }),
                json!({ "key": "end", "kind": "point", "prompt": "Wall Mirror: click its end" }),
                json!({ "key": "toward", "kind": "point", "prompt": "Wall Mirror: click a point in the room" }),
            ],
            args,
        );
    }

    /// The mirror picked: placed.
    pub(crate) fn gym_mirror_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let start = args["start"].take();
        args["start"] = start["point"].clone();
        args["level"] = start["level"].clone();
        match self.run("gym.mirror", args) {
            Ok(_) => {
                self.refresh_scene(&frame);
                frame.set_status_text("Mirror placed; Check Gym looks for mirrors in line with treadmills", 0);
            }
            Err(e) => error(&frame, &format!("The mirror was not placed:\n{e}")),
        }
    }

    /// View › Coordination › Check Gym (ARC-GYM-001): ADA §236.1 by type, ceilings and mirrors, the
    /// finding picked selected and framed.
    pub(crate) fn check_gym(&mut self) {
        let frame = self.frame;
        let c = match self.run("gym.check", json!({})) {
            Ok(v) => v,
            Err(e) => return error(&frame, &e),
        };
        let types = c["types"].as_array().cloned().unwrap_or_default();
        if types.is_empty() {
            return error(&frame, "No gym equipment: place some from the Riviera Gym Equipment library.");
        }
        let head: Vec<String> = types.iter().map(|t| format!("{}: {} units, {} accessible with a clear space", t["kind"].as_str().unwrap_or_default(), t["units"], t["with_space"])).collect();
        let findings = c["findings"].as_array().cloned().unwrap_or_default();
        if findings.is_empty() {
            return self.report_message("Check Gym", &format!("{}\n\nThe gym passes; Check Rules covers the equipment's use zones.", head.join("\n")), "gym.check", json!({}));
        }
        let rows: Vec<String> = findings.iter().map(|f| format!("{} {}: {}", if f["severity"] == "error" { "✗" } else { "•" }, f["rule"].as_str().unwrap_or_default(), f["message"].as_str().unwrap_or_default())).collect();
        if let Some(k) = self.pick_or_report("Check Gym", &head.join("\n"), &rows, "gym.check", json!({}))
            && let Some(id) = findings[k]["element"].as_u64()
        {
            let _ = self.run("selection.set", json!({ "ids": [id] }));
            self.selection_changed();
            self.zoom_to_fit();
        }
    }

    /// View › Coordination › Check Pools (ARC-POOL-002): each pool against the pool packs in use,
    /// the finding picked selected and framed.
    pub(crate) fn check_pools(&mut self) {
        let frame = self.frame;
        let c = match self.run("pool.check", json!({})) {
            Ok(v) => v,
            Err(e) => return error(&frame, &e),
        };
        let pools = c["pools"].as_array().cloned().unwrap_or_default();
        if pools.is_empty() {
            return error(&frame, "No swimming pools: place one with Architecture › Build › Swimming Pool.");
        }
        let packs: Vec<&str> = c["packs"].as_array().map(|a| a.iter().filter_map(|p| p.as_str()).collect()).unwrap_or_default();
        let mut head: Vec<String> = vec![format!("Packs: {}", if packs.is_empty() { "none (choose them in Rule Packs)".to_owned() } else { packs.join(", ") })];
        for p in &pools {
            let loads: Vec<String> = p["loads"].as_array().map(|a| a.iter().map(|l| format!("{} bathers ({})", l["bathers"], l["rule"].as_str().unwrap_or_default())).collect()).unwrap_or_default();
            head.push(format!(
                "{} ({}): {} entries, {} accessible, {} drains{}",
                p["name"].as_str().unwrap_or_default(),
                p["class"].as_str().unwrap_or_default(),
                p["entries"],
                p["accessible_entries"],
                p["drains"],
                if loads.is_empty() { String::new() } else { format!("; {}", loads.join(", ")) }
            ));
        }
        let findings = c["findings"].as_array().cloned().unwrap_or_default();
        if findings.is_empty() {
            return self.report_message("Check Pools", &format!("{}\n\nEvery pool passes its packs.", head.join("\n")), "pool.check", json!({}));
        }
        let rows: Vec<String> = findings
            .iter()
            .map(|f| {
                let secondary = if f["confidence"] == "secondary" { " (secondary source: check the code's text)" } else { "" };
                format!("{} {} {}: {}{secondary}", if f["severity"] == "error" { "✗" } else { "•" }, f["pack"].as_str().unwrap_or_default(), f["rule"].as_str().unwrap_or_default(), f["message"].as_str().unwrap_or_default())
            })
            .collect();
        if let Some(k) = self.pick_or_report("Check Pools", &head.join("\n"), &rows, "pool.check", json!({}))
            && let Some(id) = findings[k]["element"].as_u64().or_else(|| findings[k]["pool"].as_u64())
        {
            let _ = self.run("selection.set", json!({ "ids": [id] }));
            self.selection_changed();
            self.zoom_to_fit();
        }
    }

    /// The one room selected, or an error shown.
    fn selected_room(&mut self) -> Option<u64> {
        let frame = self.frame;
        let store = self.store_now()?;
        let rooms: Vec<u64> = self.selection().into_iter().filter(|id| store.get(*id).is_some_and(|e| riviera_core::rooms::room_recipe(e).is_some())).map(|id| id.0).collect();
        if rooms.len() != 1 {
            error(&frame, "Select one room first.");
            return None;
        }
        Some(rooms[0])
    }

    /// Architecture › Build › Gym Layout (ARC-GYM-002): the selected room filled in rows with the
    /// counts given, each piece kept clear by its zones.
    pub(crate) fn gym_layout(&mut self) {
        let frame = self.frame;
        let Some(room) = self.selected_room() else { return };
        let items: Vec<String> = riviera_core::furnishings::list(&riviera_core::Store::default(), None)
            .unwrap_or_default()
            .into_iter()
            .filter(|i| i.library == riviera_core::furnishings::Library::Gym)
            .map(|i| i.name)
            .collect();
        let f = super::reinforcement_ui::Form::new(&frame, "Gym Layout");
        let counts: Vec<TextCtrl> = items.iter().map(|n| f.text(n, "0", "")).collect();
        let ok = f.show("Rows run from the room's longest wall, the pieces in this order, each kept clear by its own zones with a 36 in aisle between rows.", "Lay Out");
        let pieces: Vec<serde_json::Value> = items
            .iter()
            .zip(&counts)
            .filter_map(|(n, t)| t.get_value().trim().parse::<u32>().ok().filter(|c| *c > 0).map(|c| json!({ "item": n, "count": c })))
            .collect();
        f.dialog.destroy();
        if !ok || pieces.is_empty() {
            return;
        }
        match self.run("gym.layout", json!({ "room_id": room, "pieces": pieces })) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let left: u64 = v["unplaced"].as_object().map_or(0, |m| m.values().filter_map(serde_json::Value::as_u64).sum());
                let tail = if left > 0 { format!("; {left} did not fit") } else { String::new() };
                frame.set_status_text(&format!("{} pieces in {} row(s){tail}; Check Gym and Check Rules check them", v["placed"].as_array().map_or(0, Vec::len), v["rows"]), 0);
            }
            Err(e) => error(&frame, &format!("The gym was not laid out:\n{e}")),
        }
    }

    /// Architecture › Build › Studio (ARC-GYM-002): the selected room's class capacity, and a
    /// cycling studio's bikes.
    pub(crate) fn gym_studio(&mut self) {
        let frame = self.frame;
        let Some(room) = self.selected_room() else { return };
        let labels = ["Indoor cycling (1.5 × 2.0 m a bike), placing the bikes", "Yoga and mat Pilates (25 ft² a person)", "HIIT and functional (50 ft² a person)"];
        let d = SingleChoiceDialog::builder(&frame, "The selected room is a studio for:", "Studio", &labels).build();
        if d.show_modal() != ID_OK {
            return;
        }
        let kind = ["cycle", "mat", "hiit"][d.get_selection().max(0) as usize];
        d.destroy();
        match self.run("gym.studio", json!({ "room_id": room, "kind": kind })) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let placed = v["placed"].as_array().map_or(0, Vec::len);
                let msg = format!("{:.0} m²: about {} places ({}).{}", v["area"].as_f64().unwrap_or(0.0), v["capacity"], v["note"].as_str().unwrap_or_default(), if placed > 0 { format!(" {placed} bikes placed facing the instructor's wall.") } else { String::new() });
                frame.set_status_text(&msg, 0);
            }
            Err(e) => error(&frame, &format!("The studio was not sized:\n{e}")),
        }
    }

    /// Architecture › Build › Swimming Pool (ARC-POOL-001): a template placed by its centre and the
    /// direction of its deep end, or an outline typed as coordinates.
    pub(crate) fn swimming_pool(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Swimming Pool");
        let kinds = ["competition_25m", "competition_50m", "competition_25yd", "teaching", "lap", "rectangle", "wading"];
        let labels = ["Competition 25 m", "Competition 50 m", "Competition 25 yd", "Teaching 12 × 7 m", "Lap pool", "Rectangle", "Wading pool"];
        let kind = f.choice("Pool", &labels.map(str::to_owned), 0);
        let lanes = f.text("Lanes", "", "Competition pools: default 6 (8 for 50 m), each 2.5 m");
        let length = f.text("Length (m)", "", "Rectangle and lap pools");
        let width = f.text("Width (m)", "", "");
        let outline = f.text("Or an outline", "", "x,y; x,y; … from the shallow end's wall, counter-clockwise (metres)");
        let depths = f.text("Depths (m)", "", "at:depth, … along the axis, e.g. 0:1.0, 7:1.52, 10:2.0; empty for the template's");
        let edges = f.choice("Edge", &["Skimmer".to_owned(), "Deck-level gutter".to_owned()], 0);
        let ok = f.show("Then click the pool's centre and a point toward its deep end (a typed outline places at once).", "Place");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let pairs = |s: &str, sep: char| -> Option<Vec<[f64; 2]>> {
            let v: Option<Vec<[f64; 2]>> = s
                .split(sep)
                .filter(|p| !p.trim().is_empty())
                .map(|p| {
                    let mut it = p.split(if sep == ';' { ',' } else { ':' }).map(|x| x.trim().parse::<f64>().ok());
                    Some([it.next()??, it.next()??])
                })
                .collect();
            v.filter(|v| !v.is_empty())
        };
        let kind = kinds[kind.get_selection().unwrap_or(0) as usize];
        let edge = if edges.get_selection().unwrap_or(0) == 1 { json!([{ "kind": "deck_level" }]) } else { json!([{ "kind": "skimmer" }]) };
        let mut args = json!({ "edges": edge });
        if let Some(d) = pairs(&depths.get_value(), ',') {
            args["profile"] = json!(d.iter().map(|p| json!({ "at": p[0], "depth": p[1] })).collect::<Vec<_>>());
        }
        let typed = pairs(&outline.get_value(), ';');
        let mut template = json!({ "kind": kind, "origin": [0.0, 0.0] });
        if let Ok(n) = lanes.get_value().trim().parse::<u32>() {
            template["lanes"] = json!(n);
        }
        if let Some(l) = num(&length) {
            template["length"] = json!(l);
        }
        if let Some(w) = num(&width) {
            template["width"] = json!(w);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        if let Some(o) = typed {
            // A typed outline goes on the lowest level.
            let Some(level) = self.levels().first().map(|l| l.id.0) else { return error(&frame, "The model has no level.") };
            args["outline"] = json!(o);
            args["level"] = json!(level);
            return self.swimming_pool_done(args);
        }
        args["template"] = template;
        self.start_picking(
            "pool.create",
            vec![
                json!({ "key": "origin", "kind": "at", "prompt": "Swimming Pool: click the pool's centre" }),
                json!({ "key": "toward", "kind": "point", "prompt": "Swimming Pool: click a point toward its deep end" }),
            ],
            args,
        );
    }

    /// The pool picked: its template turned toward the deep end and placed.
    pub(crate) fn swimming_pool_finish(&mut self, mut args: serde_json::Value) {
        let origin = args["origin"].take();
        let toward = args["toward"].take();
        let (o, t) = (origin["point"].clone(), toward);
        let (ox, oy) = (o[0].as_f64().unwrap_or(0.0), o[1].as_f64().unwrap_or(0.0));
        let (tx, ty) = (t[0].as_f64().unwrap_or(ox + 1.0), t[1].as_f64().unwrap_or(oy));
        args["template"]["origin"] = json!([ox, oy]);
        args["template"]["rotation"] = json!((ty - oy).atan2(tx - ox));
        args["level"] = origin["level"].clone();
        self.swimming_pool_done(args);
    }

    /// Architecture › Build › Pool Entry (ARC-POOL-003): stairs, a sloped entry or a bench built
    /// into the selected pool (or the only one), against the edge clicked.
    pub(crate) fn pool_entry(&mut self) {
        let frame = self.frame;
        let Some(pool) = self.selected_pool() else { return };
        let f = super::reinforcement_ui::Form::new(&frame, "Pool Entry");
        let kinds = ["stairs", "sloped_entry", "bench", "recessed_steps", "beach", "swimout"];
        let names = ["Stairs", "Sloped entry (ADA)", "Underwater bench", "Recessed steps with grab rails", "Beach (zero-depth) entry", "Swimout (a seat in an alcove)"].map(str::to_owned);
        let kind = f.choice("Entry", &names, 0);
        let width = f.text("Width (m)", "", "Along the edge: 1.5 m for stairs, 1.0 m for a sloped entry, 2.0 m for a bench, 0.457 m for recessed treads; a beach takes the whole edge");
        let tread = f.text("Tread (m)", "", "Stairs: default 0.305 (12 in)");
        let riser = f.text("Highest riser (m)", "", "Stairs: default 0.25, the risers made equal under it; recessed steps: their spacing, default 0.254 (10 in)");
        let slope = f.text("Slope 1:", "", "Sloped or beach entry: default 12");
        let reach = f.text("Reaches (m below water)", "", "Sloped entry: default 0.69 (27 in)");
        let seat = f.text("Seat (m below water)", "", "Bench or swimout: default 0.45");
        let depth = f.text("Recess (m)", "", "Recessed steps: how deep the treads go into the wall, default 0.14 (5½ in); swimout: how far out behind the wall, default 0.6 (24 in)");
        let top = f.text("Top tread (m below deck)", "", "Recessed steps: default 0.254 (10 in)");
        let rails = f.text("Handrails", "", "Default 2, one at each side (grab rails for recessed steps); 0 for none; more are spread across");
        let rail_h = f.text("Handrail height (m)", "", "Above the nosings or ramp: default 0.914 (36 in); grab rails above the deck, 0.85");
        let rail_gap = f.text("Between handrails (m, clear)", "", "Default the width less 30 mm a side; grab rails 0.533 (21 in)");
        let nosing = f.text("Nosing stripe (m)", "", "The contrasting stripe on each tread's or seat's edge: default 0.038 (1.5 in) on stairs and swimouts; 0 for none");
        let ok = f.show("Then click by the pool's wall where the entry's middle goes: it runs from that edge into the pool.", "Pick the Edge");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok().filter(|v| *v > 0.0);
        let mut args = json!({ "pool_id": pool, "kind": kinds[kind.get_selection().unwrap_or(0) as usize] });
        for (k, t) in [("width", &width), ("tread", &tread), ("riser", &riser), ("reach", &reach), ("seat", &seat), ("depth", &depth), ("top", &top), ("handrail_height", &rail_h), ("handrail_gap", &rail_gap)] {
            if let Some(v) = num(t) {
                args[k] = json!(v);
            }
        }
        if let Ok(n) = rails.get_value().trim().parse::<u32>() {
            args["handrails"] = json!(n);
        }
        if let Some(v) = nosing.get_value().trim().replace(',', ".").parse::<f64>().ok().filter(|v| *v >= 0.0) {
            args["nosing"] = json!(v);
        }
        if let Some(n) = num(&slope) {
            args["slope"] = json!(1.0 / n);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking("pool.add_entry", vec![json!({ "key": "point", "kind": "point", "prompt": "Pool Entry: click by the wall where its middle goes" })], args);
    }

    /// View › Coordination › Pool Hall Air (ARC-POOL-008): the selected pool's hall conditions set,
    /// and its evaporation, latent load, water heat and outdoor air shown.
    pub(crate) fn pool_hall_air(&mut self) {
        let frame = self.frame;
        let Some(pool) = self.selected_pool() else { return };
        let now = self.run("pool.natatorium", json!({ "pool_id": pool })).unwrap_or_default();
        let c = &now[0]["conditions"];
        let v = |k: &str| c[k].as_f64().map(|x| format!("{x}")).unwrap_or_default();
        let f = super::reinforcement_ui::Form::new(&frame, "Pool Hall Air");
        let water = f.text("Water (°C)", &v("water"), "PWTAG and ASHRAE: 27–29 °C for swimming, 31 °C for children");
        let air = f.text("Air (°C)", &v("air"), "1–2 K warmer than the water, to about 30 °C");
        let rh = f.text("Relative humidity (%)", &c["rh"].as_f64().map(|x| format!("{:.0}", x * 100.0)).unwrap_or_default(), "ASHRAE 50–60 %; Sport England 60 % ± 10 %");
        let af = f.text("Activity factor", &v("activity"), "ASHRAE: public and schools 1.0, hotels 0.8, competition 0.65, residential 0.5");
        let cover = f.text("Cover (fraction let through)", &v("cover"), "Empty for no cover; a sealed cover about 0.1, a slatted movable floor about 0.3");
        let spectators = f.text("Spectators", &c["spectators"].as_u64().map(|x| x.to_string()).unwrap_or_default(), "Each needs 3.8 L/s of outdoor air");
        let dew = f.text("Outdoor design dew point (°C)", "", "Optional: the outdoor air the evaporation alone would need");
        let ok = f.show("The values are kept on the pool and feed the design loads of the space that holds it.", "Work It Out");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let mut hall = json!({});
        for (k, t) in [("water", &water), ("air", &air), ("activity", &af), ("cover", &cover)] {
            if let Some(x) = num(t) {
                hall[k] = json!(x);
            }
        }
        if let Some(x) = num(&rh) {
            hall["rh"] = json!(x / 100.0);
        }
        if let Ok(n) = spectators.get_value().trim().parse::<u32>() {
            hall["spectators"] = json!(n);
        }
        let mut args = json!({ "pool_id": pool });
        if let Some(d) = num(&dew) {
            args["outdoor_dew_point"] = json!(d);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        if let Err(e) = self.run("pool.set", json!({ "pool_id": pool, "hall": hall })) {
            return error(&frame, &format!("The conditions were not kept:\n{e}"));
        }
        let n = match self.run("pool.natatorium", args.clone()) {
            Ok(v) => v[0].clone(),
            Err(e) => return error(&frame, &e),
        };
        let g = |k: &str| n[k].as_f64().unwrap_or(0.0);
        let mut text = format!(
            "{}: {:.0} m² of water, {:.0} m² of wet deck\nEvaporation {:.1} kg/h in use, {:.1} unoccupied{}\nLatent load {:.1} kW; the water loses {:.1} kW (evaporation {:.1}, convection {:.1}, make-up {:.1})\nWater lost {:.0} L a day\nOutdoor air {:.0} L/s (purge {:.0}, exhaust {:.0} to keep the hall negative)\nHall air dew point {:.1} °C",
            n["name"].as_str().unwrap_or_default(),
            g("water_area"),
            g("wet_deck_area"),
            g("evaporation"),
            g("evaporation_unoccupied"),
            n["evaporation_covered"].as_f64().map(|x| format!(", {x:.1} covered")).unwrap_or_default(),
            g("latent_load") / 1000.0,
            n["water_heat"]["total"].as_f64().unwrap_or(0.0) / 1000.0,
            n["water_heat"]["evaporation"].as_f64().unwrap_or(0.0) / 1000.0,
            n["water_heat"]["convection"].as_f64().unwrap_or(0.0) / 1000.0,
            n["water_heat"]["make_up"].as_f64().unwrap_or(0.0) / 1000.0,
            g("water_per_day"),
            g("outdoor_air"),
            g("purge"),
            g("exhaust"),
            g("dew_point"),
        );
        if let Some(a) = n["air_changes"].as_f64() {
            text.push_str(&format!("\nThe hall: {:.0} m³, {a:.1} outdoor air changes an hour", g("hall_volume")));
        }
        if let Some(a) = n["outdoor_air_for_moisture"].as_f64() {
            text.push_str(&format!("\nOutdoor air to remove the evaporation alone: {a:.0} L/s"));
        }
        for t in n["notes"].as_array().into_iter().flatten() {
            text.push_str(&format!("\n• {}", t.as_str().unwrap_or_default()));
        }
        self.refresh_scene(&frame);
        self.report_message("Pool Hall Air", &text, "pool.natatorium", args);
    }

    /// View › Coordination › Pool Water Treatment (ARC-POOL-009): the selected pool's turnover,
    /// filters, balance tank, pipes and plant room.
    pub(crate) fn pool_water_treatment(&mut self) {
        let frame = self.frame;
        let Some(pool) = self.selected_pool() else { return };
        let f = super::reinforcement_ui::Form::new(&frame, "Pool Water Treatment");
        let basis = f.choice("Basis", &["From the building code".to_owned(), "US: the MAHC".to_owned(), "UK: Sport England and PWTAG".to_owned()], 0);
        let turnover = f.text("Turnover (h)", "", "Empty for the basis's for the pool's type");
        let rate = f.text("Filtration rate (m/h)", "", "Empty for 36.7 (US high-rate media) or 25 (UK medium-rate sand)");
        let filters = f.text("Filters", "2", "How many share the flow");
        let ok = f.show("Sizes the plant from the pool's volume and type.", "Size It");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let mut args = json!({ "pool_id": pool });
        match basis.get_selection().unwrap_or(0) {
            1 => args["basis"] = json!("us"),
            2 => args["basis"] = json!("uk"),
            _ => {}
        }
        if let Some(v) = num(&turnover) {
            args["turnover"] = json!(v);
        }
        if let Some(v) = num(&rate) {
            args["filter_rate"] = json!(v);
        }
        if let Ok(n) = filters.get_value().trim().parse::<u32>() {
            args["filters"] = json!(n);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        let t = match self.run("pool.treatment", args.clone()) {
            Ok(v) => v[0].clone(),
            Err(e) => return error(&frame, &e),
        };
        let g = |k: &str| t[k].as_f64().unwrap_or(0.0);
        let mut text = format!(
            "{} ({}): {:.0} m³ of water and {:.1} m³ of surge or balance tank\nTurnover {:.1} h ({}) → {:.1} m³/h filtered\nFilters: {:.2} m² at {:.1} m/h, {} of {:.2} m²; backwash {:.0} m³/h, {:.1} m³ each time\n{}\nPipes: suction {:.0} mm at {:.1} m/s, discharge {:.0} mm at {:.1} m/s\nMain drains: {} counted, each for {:.1} m³/h\nPlant room {:.0}–{:.0} m²; {:.0} bathers an hour; {:.0} L a day of dilution",
            t["name"].as_str().unwrap_or_default(),
            t["basis"].as_str().unwrap_or_default().to_uppercase(),
            g("water_volume"),
            g("surge_volume"),
            g("turnover"),
            t["kind"].as_str().unwrap_or_default(),
            g("flow"),
            g("filter_area"),
            g("filter_rate"),
            t["filters"],
            g("filter_area_each"),
            g("backwash_flow"),
            g("backwash_volume"),
            match (t["skimmers"].as_u64(), t["gutter_capacity"].as_f64()) {
                (Some(n), _) => format!("{n} skimmers (one per 46 m²)"),
                (_, Some(c)) => format!("Gutters for {c:.0} m³/h (125 %)"),
                _ => String::new(),
            },
            t["suction"]["diameter"].as_f64().unwrap_or(0.0),
            t["suction"]["velocity"].as_f64().unwrap_or(0.0),
            t["discharge"]["diameter"].as_f64().unwrap_or(0.0),
            t["discharge"]["velocity"].as_f64().unwrap_or(0.0),
            t["main_drains"],
            g("flow_per_drain"),
            t["plant_room"][0].as_f64().unwrap_or(0.0),
            t["plant_room"][1].as_f64().unwrap_or(0.0),
            g("bathers_per_hour"),
            g("dilution_per_day"),
        );
        for n in t["notes"].as_array().into_iter().flatten() {
            text.push_str(&format!("\n• {}", n.as_str().unwrap_or_default()));
        }
        self.report_message("Pool Water Treatment", &text, "pool.treatment", args);
    }

    /// View › Coordination › Pool Structure (ARC-POOL-010): the selected pool's loads, full and
    /// empty, with the water table's uplift.
    pub(crate) fn pool_structure(&mut self) {
        let frame = self.frame;
        let Some(pool) = self.selected_pool() else { return };
        let f = super::reinforcement_ui::Form::new(&frame, "Pool Structure");
        let gw = f.text("Water table (m)", "", "Its height above the pool's level (negative below); empty skips the uplift check");
        let factor = f.text("Uplift factor", "1.1", "The empty pool's weight over the uplift");
        let on = f.choice("Stands on", &["As the levels say".to_owned(), "The ground".to_owned(), "A structure".to_owned()], 0);
        let ok = f.show("Works out the water's and the shell's weight, the walls' pressure and the uplift on the empty pool.", "Work Out");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let mut args = json!({ "pool_id": pool });
        if let Some(v) = num(&gw) {
            args["groundwater"] = json!(v);
        }
        if let Some(v) = num(&factor) {
            args["factor"] = json!(v);
        }
        match on.get_selection().unwrap_or(0) {
            1 => args["elevated"] = json!(false),
            2 => args["elevated"] = json!(true),
            _ => {}
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        let s = match self.run("pool.structure", args.clone()) {
            Ok(v) => v[0].clone(),
            Err(e) => return error(&frame, &e),
        };
        let g = |k: &str| s[k].as_f64().unwrap_or(0.0);
        let w = |k: &str| s["wall"][k].as_f64().unwrap_or(0.0);
        let mut text = format!(
            "{}: {:.0} m² of water on a {:.0} m² footprint, its shell {:.0} mm\nWater {:.0} kN, shell {:.0} kN, finishes {:.0} kN\nFull {:.1} kN/m² over the footprint on average ({:.1} on the floor at its deepest, the walls {:.1} kN/m); empty {:.1} kN/m²\nWalls {:.2} m deep: {:.1} kPa at the base, {:.1} kN/m shear, {:.1} kN·m/m moment",
            s["name"].as_str().unwrap_or_default(),
            g("area"),
            g("footprint"),
            g("thickness") * 1000.0,
            g("water_weight"),
            g("shell_weight"),
            g("finishes"),
            g("full"),
            g("peak"),
            g("wall_line_load"),
            g("empty"),
            w("depth"),
            w("pressure"),
            w("shear"),
            w("moment"),
        );
        let u = &s["uplift"];
        if u.is_object() {
            let h = |k: &str| u[k].as_f64().unwrap_or(0.0);
            text.push_str(&format!("\nUplift empty: {:.0} kN against {:.0} kN of weight ({:.2}, {} the {:.2} asked)", h("force"), h("resisting"), h("ratio"), if u["ok"] == true { "meeting" } else { "under" }, h("factor")));
        }
        for p in s["point_loads"].as_array().into_iter().flatten() {
            text.push_str(&format!("\n{} ({}): {:.1} kN at its anchorage (advisory)", p["name"].as_str().unwrap_or_default(), p["object_type"].as_str().unwrap_or_default(), p["load"].as_f64().unwrap_or(0.0)));
        }
        for n in s["notes"].as_array().into_iter().flatten() {
            text.push_str(&format!("\n• {}", n.as_str().unwrap_or_default()));
        }
        self.report_message("Pool Structure", &text, "pool.structure", args);
    }

    /// View › Coordination › Pool Barriers (ARC-POOL-012): what encloses the selected pool.
    pub(crate) fn pool_barriers(&mut self) {
        let frame = self.frame;
        let Some(pool) = self.selected_pool() else { return };
        let args = json!({ "pool_id": pool });
        let b = match self.run("pool.barriers", args.clone()) {
            Ok(v) => v[0].clone(),
            Err(e) => return error(&frame, &e),
        };
        let mut text = if b["enclosed"] == true {
            format!("{}: enclosed, {:.0} m² inside the barrier", b["name"].as_str().unwrap_or_default(), b["area"].as_f64().unwrap_or(0.0))
        } else {
            let at: Vec<String> = b["openings"].as_array().into_iter().flatten().map(|p| format!("({:.1}, {:.1})", p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0))).collect();
            format!("{}: not enclosed{}", b["name"].as_str().unwrap_or_default(), if at.is_empty() { String::new() } else { format!(" (open at {})", at.join(", ")) })
        };
        for x in b["barriers"].as_array().into_iter().flatten() {
            let mm = |k: &str| x[k].as_f64().map(|v| format!(", {k} {:.0} mm", v * 1000.0)).unwrap_or_default();
            text.push_str(&format!("\n{} ({}): {:.2} m high{}{}{}", x["name"].as_str().unwrap_or_default(), x["kind"].as_str().unwrap_or_default(), x["height"].as_f64().unwrap_or(0.0), mm("clearance"), mm("opening"), mm("rails_apart")));
        }
        for g in b["gates"].as_array().into_iter().flatten() {
            text.push_str(&format!("\nGate {:.2} m wide: swings {}", g["width"].as_f64().unwrap_or(0.0), if g["swings_out"] == true { "out, away from the pool" } else { "in, toward the pool" }));
        }
        for o in b["wall_openings"].as_array().into_iter().flatten() {
            text.push_str(&format!("\nA {} in a barrier wall, {:.2} m up", o["kind"].as_str().unwrap_or_default(), o["sill"].as_f64().unwrap_or(0.0)));
        }
        text.push_str("\n• Check Pools judges them by the ISPSC (48 in) or the MAHC (6 ft).");
        self.report_message("Pool Barriers", &text, "pool.barriers", args);
    }

    /// View › Coordination › Pool Electrical Zones (ARC-POOL-010): the devices round the selected
    /// pool against NEC 680 or IEC 60364-7-702, and its bonding.
    pub(crate) fn pool_bonding(&mut self) {
        let frame = self.frame;
        let Some(pool) = self.selected_pool() else { return };
        let f = super::reinforcement_ui::Form::new(&frame, "Pool Electrical Zones");
        let basis = f.choice("Basis", &["From the building code".to_owned(), "US: NEC Article 680".to_owned(), "IEC 60364-7-702 (BS 7671 702)".to_owned()], 0);
        let ok = f.show("Checks the receptacles, switches, luminaires and switchgear round the pool, and lists what to bond.", "Check");
        let mut args = json!({ "pool_id": pool });
        match basis.get_selection().unwrap_or(0) {
            1 => args["basis"] = json!("nec"),
            2 => args["basis"] = json!("iec"),
            _ => {}
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        let b = match self.run("pool.bonding", args.clone()) {
            Ok(v) => v[0].clone(),
            Err(e) => return error(&frame, &e),
        };
        let mut text = format!(
            "{} ({}, {}): {} electrical elements within reach, {} to bond",
            b["name"].as_str().unwrap_or_default(),
            if b["basis"] == "iec" { "IEC 60364-7-702" } else { "NEC Article 680" },
            if b["indoor"] == true { "indoors" } else { "outdoors" },
            b["devices"].as_array().map_or(0, Vec::len),
            b["bond"].as_array().map_or(0, Vec::len),
        );
        let grid = &b["grid"];
        if grid.is_object() {
            let h = |k: &str| grid[k].as_f64().unwrap_or(0.0);
            text.push_str(&format!("\nBonding grid: {:.1} m² of perimeter surface, {:.0} m of 8 AWG copper on a 12 in grid (or a {:.1} m ring 18–24 in out), tied at four points; {:.0} mm² touching the water", h("perimeter_surface"), h("grid_conductor"), h("ring_length"), h("water_contact")));
        }
        for x in b["findings"].as_array().into_iter().flatten() {
            text.push_str(&format!("\n{} [{}] {}", x["severity"].as_str().unwrap_or_default(), x["source"].as_str().unwrap_or_default(), x["message"].as_str().unwrap_or_default()));
        }
        for x in b["bond"].as_array().into_iter().flatten() {
            text.push_str(&format!("\nBond: {} ({}), {:.2} m from the water", x["name"].as_str().unwrap_or_default(), x["what"].as_str().unwrap_or_default(), x["distance"].as_f64().unwrap_or(0.0)));
        }
        for n in b["notes"].as_array().into_iter().flatten() {
            text.push_str(&format!("\n• {}", n.as_str().unwrap_or_default()));
        }
        self.report_message("Pool Electrical Zones", &text, "pool.bonding", args);
    }

    /// The selected pool, or the model's only one; else an error shown.
    /// Modify | Railings › Gate Hardware (ARC-POOL-016): the selected gates' (or the selected
    /// railings' gates') closing and latching hardware.
    pub(crate) fn gate_hardware(&mut self) {
        let frame = self.frame;
        let Some(store) = self.store_now() else { return };
        let selected = self.selection();
        let railings: Vec<String> = selected.iter().filter_map(|id| store.get(*id)).filter(|e| riviera_core::railings::railing_recipe(e).is_some()).map(|e| e.guid().to_string()).collect();
        let gates: Vec<&riviera_core::Element> = store.iter().filter(|e| riviera_core::railings::gate_of(e).is_some_and(|(r, _)| selected.contains(&e.id()) || railings.contains(&r))).collect();
        let Some(first) = gates.first() else {
            return error(&frame, "Select a gate, or a railing with gates, first.");
        };
        let h = riviera_core::railings::hardware(first).unwrap_or_default();
        let ids: Vec<u64> = gates.iter().map(|e| e.id().0).collect();
        let yes_no = ["Yes".to_owned(), "No".to_owned()];
        let f = super::reinforcement_ui::Form::new(&frame, "Gate Hardware");
        let closing = f.choice("Self-closing", &yes_no, usize::from(!h.self_closing));
        let latching = f.choice("Self-latching", &yes_no, usize::from(!h.self_latching));
        let lockable = f.choice("Lockable", &yes_no, usize::from(!h.lockable));
        let kinds = ["outside", "key", "pool_side"];
        let latch = f.choice("Latch released", &["Outside, by hand".to_owned(), "By a key, card or combination".to_owned(), "From the pool side only".to_owned()], kinds.iter().position(|k| serde_json::to_value(h.latch).ok().and_then(|v| v.as_str().map(|s| s == *k)).unwrap_or(false)).unwrap_or(0));
        let height = f.text("Latch height (m)", &format!("{:.3}", h.latch_height), "Its release above the ground: the ISPSC asks 52–54 in (1.32–1.37 m) at public pools, 54 in at homes; the MAHC 54 in");
        let mesh = f.choice("½ in mesh within 18 in of the latch", &yes_no, usize::from(!h.latch_mesh));
        let ok = f.show(&format!("For {} gate(s); Check Pools holds pool gates to it.", ids.len()), "Set");
        let pick = |c: &Choice| c.get_selection().unwrap_or(0) == 0;
        let mut args = json!({
            "self_closing": pick(&closing),
            "self_latching": pick(&latching),
            "lockable": pick(&lockable),
            "latch": kinds[latch.get_selection().unwrap_or(0) as usize],
            "latch_mesh": pick(&mesh),
        });
        if let Some(v) = height.get_value().trim().replace(',', ".").parse::<f64>().ok().filter(|v| *v > 0.0) {
            args["latch_height"] = json!(v);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        for id in ids {
            let mut a = args.clone();
            a["gate_id"] = json!(id);
            if let Err(e) = self.run("railing.gate_hardware", a) {
                return error(&frame, &format!("The hardware was not set:\n{e}"));
            }
        }
        frame.set_status_text("Gate hardware set; Check Pools checks it", 0);
    }

    fn selected_pool(&mut self) -> Option<u64> {
        let frame = self.frame;
        let store = self.store_now()?;
        let pools: Vec<u64> = store.iter().filter(|e| riviera_core::pools::recipe(e).is_some()).map(|e| e.id().0).collect();
        let selected: Vec<u64> = self.selection().into_iter().map(|id| id.0).filter(|id| pools.contains(id)).collect();
        match (selected.as_slice(), pools.as_slice()) {
            ([one], _) | ([], [one]) => Some(*one),
            ([], []) => {
                error(&frame, "No swimming pool: place one with Architecture › Build › Swimming Pool.");
                None
            }
            _ => {
                error(&frame, "Select one swimming pool first.");
                None
            }
        }
    }

    /// Architecture › Build › Pool Lanes (ARC-POOL-005): the selected pool's competition lanes and
    /// markings.
    pub(crate) fn pool_lanes(&mut self) {
        let frame = self.frame;
        let Some(pool) = self.selected_pool() else { return };
        let now = self.run("pool.info", json!({ "pool_id": pool })).unwrap_or_default();
        let lanes = &now["lanes"];
        let count = lanes["widths"].as_array().map_or(0, Vec::len);
        let f = super::reinforcement_ui::Form::new(&frame, "Pool Lanes");
        let n = f.text("Lanes", &count.to_string(), "0 takes them away");
        let width = f.text("Lane width (m)", "", "The middle lanes; default 2.5");
        let gap = f.text("Side gaps (m)", "", "Outside the first and last lanes; default 0.1 (World Aquatics 2026)");
        let pads = f.choice("Touch pads", &["At both ends".to_owned(), "At one end".to_owned(), "None".to_owned()], 0);
        let marks = f.check("Markings", "Draw the lane lines, T-ends, 15 m lines and end-wall targets", true);
        let ok = f.show("The outer lanes take what is left of the pool's width. Check Pools checks them against World Aquatics where the pool is marked for competition.", "Set Lanes");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let mut args = json!({ "pool_id": pool, "count": n.get_value().trim().parse::<u32>().unwrap_or(0), "pads": 2 - pads.get_selection().unwrap_or(0).min(2), "markings": marks.get_value() });
        if let Some(v) = num(&width) {
            args["width"] = json!(v);
        }
        if let Some(v) = num(&gap) {
            args["gap"] = json!(v);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        match self.run("pool.lanes", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let text = if v.is_null() {
                    "Lanes taken away".to_owned()
                } else {
                    let w: Vec<String> = v["widths"].as_array().map(|a| a.iter().map(|w| format!("{:.2}", w.as_f64().unwrap_or(0.0))).collect()).unwrap_or_default();
                    format!("{} lanes ({} m); {:.3} m between the touch pads", w.len(), w.join(", "), v["course"].as_f64().unwrap_or(0.0))
                };
                frame.set_status_text(&text, 0);
            }
            Err(e) => error(&frame, &format!("The lanes were not set:\n{e}")),
        }
    }

    /// Architecture › Build › Pool Depth Markings (ARC-POOL-011): the selected pool's depth and
    /// NO DIVING markers and its break-point band.
    pub(crate) fn pool_depth_markings(&mut self) {
        let frame = self.frame;
        let Some(pool) = self.selected_pool() else { return };
        let f = super::reinforcement_ui::Form::new(&frame, "Pool Depth Markings");
        let units = f.choice("Units", &["From the building code".to_owned(), "Feet and inches".to_owned(), "Metres".to_owned(), "Both".to_owned()], 0);
        let increment = f.text("Every (m of depth)", "", "Markers at each step of depth to 5 ft; default 1 ft (0.3048), 0.5 in metres");
        let spacing = f.text("At most apart (m)", "", "Round the perimeter; default 25 ft (7.62)");
        let no_diving = f.check("NO DIVING", "Where the water is 5 ft deep or less", true);
        let band = f.check("Break line", "The 4 in band at the 5 ft break and the float line 1 ft shallower", true);
        let remove = f.check("Remove", "Take the markings away", false);
        let ok = f.show("Markers go at each end's and side's least and greatest depth, every slope change and step of depth, and wherever two would be too far apart; each reads the depth 3 ft out from the wall.", "Mark Depths");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let mut args = json!({ "pool_id": pool, "no_diving": no_diving.get_value(), "break_line": band.get_value(), "remove": remove.get_value() });
        match units.get_selection().unwrap_or(0) {
            1 => args["units"] = json!("feet"),
            2 => args["units"] = json!("metres"),
            3 => args["units"] = json!("both"),
            _ => {}
        }
        if let Some(v) = num(&increment) {
            args["increment"] = json!(v);
        }
        if let Some(v) = num(&spacing) {
            args["spacing"] = json!(v);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        match self.run("pool.depth_markings", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let n = v["markers"].as_array().map_or(0, Vec::len);
                let dive = v["markers"].as_array().map_or(0, |a| a.iter().filter(|m| m["no_diving"] == true).count());
                let text = if n == 0 { "Depth markings taken away".to_owned() } else { format!("{n} depth markers, {dive} with NO DIVING{}", if v["lines"].is_object() { "; the break-point band and float line" } else { "" }) };
                frame.set_status_text(&text, 0);
            }
            Err(e) => error(&frame, &format!("The markings were not set:\n{e}")),
        }
    }

    /// Architecture › Build › Pool Plant (ARC-POOL-014): the selected pool's treatment plant,
    /// sized, then a click in the plant room.
    pub(crate) fn pool_plant(&mut self) {
        let frame = self.frame;
        let Some(pool) = self.selected_pool() else { return };
        let f = super::reinforcement_ui::Form::new(&frame, "Pool Plant");
        let basis = f.choice("Basis", &["From the building code".to_owned(), "US: the MAHC".to_owned(), "UK: Sport England and PWTAG".to_owned()], 0);
        let filters = f.text("Filters", "2", "How many share the flow");
        let head = f.text("Pump head (m)", "15", "The filters, strainer and pipework");
        let ok = f.show("Then click inside the plant room: the plant goes along its longer side. Outside a room, the row starts at the click.", "Pick the Room");
        let mut args = json!({ "pool_id": pool });
        match basis.get_selection().unwrap_or(0) {
            1 => args["basis"] = json!("us"),
            2 => args["basis"] = json!("uk"),
            _ => {}
        }
        if let Ok(n) = filters.get_value().trim().parse::<u32>() {
            args["filters"] = json!(n);
        }
        if let Some(v) = head.get_value().trim().replace(',', ".").parse::<f64>().ok().filter(|v| *v > 0.0) {
            args["head"] = json!(v);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking("pool.plant", vec![json!({ "key": "at", "kind": "at", "prompt": "Pool Plant: click inside the plant room" })], args);
    }

    /// The plant room picked: the plant placed, and its numbers shown.
    pub(crate) fn pool_plant_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let at = args["at"].take();
        args.as_object_mut().expect("an object").remove("at");
        args["point"] = at["point"].clone();
        args["level"] = at["level"].clone();
        match self.run("pool.plant", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let n = |k: &str| v[k].as_f64().unwrap_or(0.0);
                let mut text = format!(
                    "{} items for {:.0} m³/h: pumps {:.1} kW ({:.2} kW at {:.0} m), filters Ø{:.0} mm, a {:.1} m³ balance tank; the row {:.1} × {:.1} m",
                    v["items"].as_array().map_or(0, Vec::len),
                    n("flow"),
                    n("motor"),
                    n("pump_power"),
                    n("head"),
                    n("filter_diameter") * 1000.0,
                    n("balance_tank"),
                    v["footprint"][0].as_f64().unwrap_or(0.0),
                    v["footprint"][1].as_f64().unwrap_or(0.0),
                );
                for note in v["notes"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
                    text.push_str("\n• ");
                    text.push_str(note);
                }
                MessageDialog::builder(&frame, &text, "Pool Plant").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build().show_modal();
            }
            Err(e) => error(&frame, &format!("The plant was not placed:\n{e}")),
        }
    }

    /// The entry's edge picked: built, and its numbers shown.
    pub(crate) fn pool_entry_finish(&mut self, args: serde_json::Value) {
        let frame = self.frame;
        match self.run("pool.add_entry", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let n = |k: &str| v[k].as_f64().unwrap_or(0.0);
                let text = match v["kind"].as_str() {
                    Some("stairs") => format!("Stairs: {} risers of {:.0} mm on {:.0} mm treads", v["risers"], n("riser") * 1000.0, n("tread") * 1000.0),
                    Some("sloped_entry") => format!("Sloped entry: 1:{:.1} over {:.2} m, reaching {:.0} mm below the water", 1.0 / n("slope").max(1e-9), n("run"), n("reaches") * 1000.0),
                    Some("recessed_steps") => format!("Recessed steps: {} treads {:.0} mm apart, {:.0} mm into the wall", v["risers"], n("riser") * 1000.0, n("recess") * 1000.0),
                    Some("swimout") => format!("Swimout: a {:.2} m² seat {:.0} mm below the water, {:.2} m out behind the wall", n("area"), n("seat") * 1000.0, n("recess")),
                    Some("beach") => format!("Beach entry: 1:{:.1}, the water's edge {:.2} m out, meeting the floor {:.2} m out", 1.0 / n("slope").max(1e-9), n("waterline"), n("run")),
                    _ => format!("Bench: its seat {:.0} mm below the water, in {:.2} m of water", n("seat") * 1000.0, n("water_depth")),
                };
                let rails = match v["handrails"].as_u64() {
                    Some(0) | None => String::new(),
                    Some(k) => format!(", {k} handrails {:.0} mm high", n("handrail_height") * 1000.0),
                };
                frame.set_status_text(&format!("{text}{rails}; Check Pools checks it"), 0);
            }
            Err(e) => error(&frame, &format!("The entry was not made:\n{e}")),
        }
    }

    fn swimming_pool_done(&mut self, args: serde_json::Value) {
        let frame = self.frame;
        match self.run("pool.create", args) {
            Ok(id) => {
                self.refresh_scene(&frame);
                let i = self.run("pool.info", json!({ "pool_id": id })).unwrap_or_default();
                frame.set_status_text(&format!("Pool: {:.1} m² of water, {:.1} m³, {:.2}–{:.2} m deep", i["area"].as_f64().unwrap_or(0.0), i["volume"].as_f64().unwrap_or(0.0), i["shallowest"].as_f64().unwrap_or(0.0), i["deepest"].as_f64().unwrap_or(0.0)), 0);
            }
            Err(e) => error(&frame, &format!("The pool was not made:\n{e}")),
        }
    }

    /// Architecture › Build › Counter (ARC-INT-004): a transaction, service, bar or work counter
    /// clicked from start to end along a wall, with an accessible portion if asked.
    pub(crate) fn counter_run(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Counter");
        let kinds = ["Transaction counter, 42 in".to_owned(), "Service counter, 36 in".to_owned(), "Bar counter, 42 in".to_owned(), "Work surface, 29 in".to_owned()];
        let kind = f.choice("Counter", &kinds, 0);
        let portions = [
            "None".to_owned(),
            "Parallel approach at the end".to_owned(),
            "Parallel approach at the start".to_owned(),
            "Forward approach at the end".to_owned(),
            "Forward approach at the start".to_owned(),
        ];
        let portion = f.choice("Accessible portion", &portions, 1);
        let ok = f.show("Then click the counter's start and end along the wall's face, in a plan. An accessible portion is lowered to 36 in (ADA §904.4): 36 in long for a parallel approach, 30 in and open below for a forward one.", "Pick the Counter");
        let k = ["transaction", "service", "bar", "work"][kind.get_selection().unwrap_or(0) as usize];
        let pi = portion.get_selection().unwrap_or(0) as usize;
        f.dialog.destroy();
        if !ok {
            return;
        }
        let mut args = json!({ "kind": k });
        if pi > 0 {
            args["accessible"] = json!({ "approach": if pi <= 2 { "parallel" } else { "forward" }, "at": if pi % 2 == 1 { "end" } else { "start" } });
        }
        self.start_picking(
            "counter.run",
            vec![json!({ "key": "start", "kind": "at", "prompt": "Counter: click its start on the wall face" }), json!({ "key": "end", "kind": "point", "prompt": "Counter: click its end" })],
            args,
        );
    }

    /// The counter picked: made, and reported.
    pub(crate) fn counter_run_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let start = args["start"].take();
        args["start"] = start["point"].clone();
        args["level"] = start["level"].clone();
        match self.run("counter.run", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let portion = v["portion"].as_object().map_or(String::new(), |p| format!(", an accessible portion {:.2}–{:.2} m at {:.0} in", p["from"].as_f64().unwrap_or(0.0), p["to"].as_f64().unwrap_or(0.0), p["height"].as_f64().unwrap_or(0.0) / 0.0254));
                frame.set_status_text(&format!("A {:.2} m counter at {:.0} in{portion}; Check Rules checks it", v["length"].as_f64().unwrap_or(0.0), v["height"].as_f64().unwrap_or(0.0) / 0.0254), 0);
            }
            Err(e) => error(&frame, &format!("The counter was not made:\n{e}")),
        }
    }

    /// Architecture › Build › Closet (ARC-INT-005): rods and a shelf along a closet's back wall,
    /// clicked from one side to the other.
    pub(crate) fn closet(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Closet");
        let layouts = ["Single rod at 66 in, shelf at 68 in".to_owned(), "Double hang at 40 and 80 in, shelf at 84 in".to_owned(), "Accessible: rod at 42 in, shelf at 46 in".to_owned()];
        let layout = f.choice("Fittings", &layouts, 0);
        let ok = f.show("Then click the closet's back wall face from one side to the other, in a plan. The rods are centred in a 24 in deep closet; the accessible fittings are within ADA §308.3.2's reach.", "Pick the Closet");
        let l = ["single", "double", "accessible"][layout.get_selection().unwrap_or(0) as usize];
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking(
            "closet.create",
            vec![json!({ "key": "start", "kind": "at", "prompt": "Closet: click one end of its back wall face" }), json!({ "key": "end", "kind": "point", "prompt": "Closet: click the other end" })],
            json!({ "layout": l }),
        );
    }

    /// The closet picked: fitted, and reported.
    pub(crate) fn closet_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let start = args["start"].take();
        args["start"] = start["point"].clone();
        args["level"] = start["level"].clone();
        match self.run("closet.create", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                frame.set_status_text(&format!("{} rod(s) and a shelf over {:.2} m", v["rods"].as_array().map_or(0, Vec::len), v["length"].as_f64().unwrap_or(0.0)), 0);
            }
            Err(e) => error(&frame, &format!("The closet was not fitted:\n{e}")),
        }
    }

    /// Architecture › Build › Guest Room (ARC-INT-002): the selected rooms marked as hotel guest
    /// rooms, with their class, beds and accessible features.
    pub(crate) fn guest_room(&mut self) {
        let frame = self.frame;
        let Some(store) = self.store_now() else { return frame.set_status_text("Open a model first", 0) };
        let rooms: Vec<u64> = self.selection().into_iter().filter(|id| store.get(*id).is_some_and(|e| riviera_core::rooms::room_recipe(e).is_some())).map(|id| id.0).collect();
        if rooms.is_empty() {
            return error(&frame, "Select one or more rooms first.");
        }
        let before = store.get(riviera_core::ElementId(rooms[0])).and_then(riviera_core::hotels::guest_room);
        let classes = ["king", "double_queen", "junior_suite"];
        let class_labels = ["King".to_owned(), "Two queens".to_owned(), "Junior suite".to_owned()];
        let class_at = before.as_ref().and_then(|b| classes.iter().position(|c| *c == b.room_type)).unwrap_or(0);
        let mobilities = [riviera_core::hotels::Mobility::None, riviera_core::hotels::Mobility::Mobility, riviera_core::hotels::Mobility::RollIn];
        let mobility_at = before.as_ref().and_then(|b| mobilities.iter().position(|m| *m == b.mobility)).unwrap_or(0);
        let f = super::reinforcement_ui::Form::new(&frame, "Guest Room");
        let class = f.choice("Class", &class_labels, class_at);
        let beds = f.text("Beds", &before.as_ref().map_or(1, |b| b.beds).to_string(), "The beds in the room");
        let mobility = f.choice("Mobility features", &["None".to_owned(), "Mobility, tub or transfer shower".to_owned(), "Mobility, roll-in shower".to_owned()], mobility_at);
        let comm = f.check("Communication", "Visible alarms and notification (ADA §806.3)", before.as_ref().is_some_and(|b| b.communication));
        let clear = f.check("Not a guest room", "Stop counting these rooms", false);
        let ok = f.show(&format!("Marks the {} selected room(s) for the room matrix (View › Coordination › Room Matrix).", rooms.len()), "Mark");
        let room_type = classes[class.get_selection().map_or(0, |k| k as usize)];
        let mobility = ["none", "mobility", "roll_in"][mobility.get_selection().map_or(0, |k| k as usize)];
        let args = json!({
            "room_ids": rooms,
            "room_type": room_type,
            "beds": beds.get_value().trim().parse::<u32>().unwrap_or(1),
            "mobility": mobility,
            "communication": comm.get_value(),
            "clear": clear.get_value(),
        });
        f.dialog.destroy();
        if !ok {
            return;
        }
        match self.run("hotel.set_room", args) {
            Ok(_) => frame.set_status_text(&format!("{} guest room(s) marked", rooms.len()), 0),
            Err(e) => error(&frame, &format!("The rooms were not marked:\n{e}")),
        }
    }

    /// View › Coordination › Room Matrix (ARC-INT-002): the guest rooms counted by floor and class
    /// and checked, with Save Report.
    pub(crate) fn room_matrix(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Room Matrix");
        let standard = f.choice("Check against", &["The building code's (default)".to_owned(), "2010 ADA Standards".to_owned(), "Approved Document M vol. 2".to_owned(), "Both".to_owned()], 0);
        let total = f.text("Rooms counted", "", "Leave empty to count the rooms placed; an alteration counts the rooms altered or added");
        let ok = f.show("Counts the guest rooms with mobility, roll-in shower and communication features and checks them against the tables.", "Count");
        let standards: &[&str] = match standard.get_selection() {
            Some(1) => &["ada_2010"],
            Some(2) => &["uk_adm_m2"],
            Some(3) => &["ada_2010", "uk_adm_m2"],
            _ => &[],
        };
        let mut args = json!({ "standards": standards });
        if let Ok(n) = total.get_value().trim().parse::<u32>() {
            args["total"] = json!(n);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        let m = match self.run("hotel.room_matrix", args.clone()) {
            Ok(v) => v,
            Err(e) => return error(&frame, &e),
        };
        if m["placed"] == 0 {
            return error(&frame, "No guest rooms: lay rooms out with a hotel template, or mark them with Architecture › Build › Guest Room.");
        }
        let row = |r: &serde_json::Value| format!("{} rooms, {} mobility ({} roll-in), {} communication", r["rooms"], r["mobility"], r["roll_in"], r["communication"]);
        let mut lines = vec![format!("{} guest rooms placed; counted on {}.", m["placed"], m["total"]), String::new(), "By floor:".to_owned()];
        for fl in m["floors"].as_array().cloned().unwrap_or_default() {
            lines.push(format!("  {}: {}", fl["level"].as_str().unwrap_or_default(), row(&fl)));
        }
        lines.push("By class:".to_owned());
        for c in m["classes"].as_array().cloned().unwrap_or_default() {
            lines.push(format!("  {}, {} bed(s): {}", c["room_type"].as_str().unwrap_or_default().replace('_', " "), c["beds"], row(&c)));
        }
        if let Some(a) = m.get("ada") {
            lines.push(format!("\nADA: mobility {} + roll-in {} needed, {} + {} provided; communication {} needed, {} counted.", a["without_roll_in_required"], a["roll_in_required"], a["without_roll_in"], a["roll_in"], a["communication_required"], a["communication_counted"]));
        }
        if let Some(u) = m.get("uk") {
            lines.push(format!("Approved Document M: {} accessible bedrooms needed, {} provided.", u["required"], u["accessible"]));
        }
        let findings = m["findings"].as_array().cloned().unwrap_or_default();
        lines.push(if findings.is_empty() { "\nThe counts are met.".to_owned() } else { String::new() });
        for x in &findings {
            lines.push(format!("{} {}: {}", if x["severity"] == "error" { "✗" } else { "•" }, x["rule"].as_str().unwrap_or_default(), x["message"].as_str().unwrap_or_default()));
        }
        self.report_message("Room Matrix", &lines.join("\n"), "hotel.room_matrix", args);
    }

    /// Architecture › Build › Countertop (ARC-KIT-002): tops on the selected units' runs, or every
    /// base run.
    pub(crate) fn countertop(&mut self) {
        let frame = self.frame;
        let sel: Vec<u64> = self.selection().into_iter().map(|id| id.0).collect();
        match self.run("countertop.from_runs", json!({ "element_ids": sel })) {
            Ok(v) => {
                self.refresh_scene(&frame);
                frame.set_status_text(&format!("{} countertop(s) placed", v.as_array().map_or(0, Vec::len)), 0);
            }
            Err(e) => error(&frame, &format!("No countertop was placed:\n{e}")),
        }
    }

    /// Lays out a bathroom where the Bathroom Layout tool was clicked, and reports the fit.
    pub(crate) fn layout_bathroom_at(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        // The Locker Room, Sauna and Steam Room tools ride on the same click, with their own
        // commands.
        let command = args.as_object_mut().and_then(|o| o.remove("command")).and_then(|c| c.as_str().map(str::to_owned)).unwrap_or_else(|| "bath.layout".to_owned());
        // A point of a pick sequence (egress routes, platform clearing).
        if command == "ui.pick" {
            return self.picked(args);
        }
        // Elevators and escalators stand on the clicked plan's level as their lowest.
        if matches!(command.as_str(), "elevator.create" | "escalator.create")
            && let Some(o) = args.as_object_mut()
            && let Some(l) = o.remove("level")
        {
            o.insert("base_level".into(), l);
        }
        // A fare line starts at the click and runs at its angle.
        if command == "fare_line.create"
            && let Some(o) = args.as_object_mut()
            && let Some(p) = o.remove("point")
        {
            let a = o.remove("angle").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let (x, y) = (p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0));
            o.insert("toward".into(), json!([x + a.cos(), y + a.sin()]));
            o.insert("start".into(), p);
        }
        match self.app.execute(&command, args, &super::user()) {
            Ok(out) => {
                if command == "interior.layout" {
                    let fit = &out.result["fit"];
                    let placed = out.result["placed"].as_array().map_or(0, Vec::len);
                    if placed == 0 {
                        let mut why = Vec::new();
                        if fit["length_short"].as_f64().unwrap_or(0.0) > 0.0 {
                            why.push(format!("{:.0} mm short along the wall ({:.0} needed)", fit["length_short"].as_f64().unwrap_or(0.0) * 1000.0, fit["length_needed"].as_f64().unwrap_or(0.0) * 1000.0));
                        }
                        if fit["depth_short"].as_f64().unwrap_or(0.0) > 0.0 {
                            why.push(format!("{:.0} mm short across the room ({:.0} needed)", fit["depth_short"].as_f64().unwrap_or(0.0) * 1000.0, fit["depth_needed"].as_f64().unwrap_or(0.0) * 1000.0));
                        }
                        for c in fit["conflicts"].as_array().into_iter().flatten() {
                            why.push(format!("{} — {} — {}", c[0].as_str().unwrap_or(""), c[1].as_str().unwrap_or(""), c[2].as_str().unwrap_or("")));
                        }
                        return error(&frame, &format!("The room is too small for this layout, so nothing was placed:\n{}", why.join("\n")));
                    }
                    self.refresh_scene(&frame);
                    let problems = self.run("rules.check", json!({ "element_ids": out.result["placed"] })).ok().and_then(|v| v.as_array().map(|a| a.iter().filter(|f| f["severity"] == "error").count())).unwrap_or(0);
                    return frame.set_status_text(&format!("{placed} piece(s) laid out; Check Rules finds {problems} error(s)"), 0);
                }
                if command == "fare_line.create" {
                    self.refresh_scene(&frame);
                    return frame.set_status_text(
                        &format!("Fare line: {} gates over {:.2} m, {:.0} people a minute in an evacuation", out.result["aisles"].as_array().map_or(0, Vec::len), out.result["length"].as_f64().unwrap_or(0.0), out.result["capacity"].as_f64().unwrap_or(0.0)),
                        0,
                    );
                }
                if command == "elevator.create" {
                    return self.elevator_made(&out.result);
                }
                if command == "escalator.create" {
                    return self.escalator_made(&out.result);
                }
                if out.result.is_u64() {
                    // A tile layout: its counts.
                    self.refresh_scene(&frame);
                    let s = self.run("tile.summary", json!({ "element_ids": [out.result] })).ok();
                    let row = s.as_ref().and_then(|v| v.get(0)).cloned().unwrap_or_default();
                    frame.set_status_text(&format!("Tiles: {} full, {} cut, {} to order, {:.2} m²", row["full"], row["cut"], row["to_order"], row["area"].as_f64().unwrap_or(0.0)), 0);
                    return;
                }
                if let Some(views) = out.result["views"].as_array() {
                    self.refresh_scene(&frame);
                    frame.set_status_text(&format!("{} elevations made{}; see Project Browser › Elevations", views.len(), if out.result["sheet"].is_null() { "" } else { " on a new A3 sheet" }), 0);
                    return;
                }
                let placed = out.result["placed"].as_array().map_or(0, Vec::len);
                self.refresh_scene(&frame);
                let problems = self
                    .run("bath.check", json!({ "element_ids": out.result["placed"] }))
                    .ok()
                    .and_then(|v| v.as_array().map(|a| a.iter().filter(|f| f["severity"] == "error").count()))
                    .unwrap_or(0);
                let heater = out.result["heater_kw"]
                    .as_f64()
                    .map(|k| format!(", the heater {k} kW for {:.1} m³", out.result["sizing"]["design_volume"].as_f64().unwrap_or(0.0)))
                    .or_else(|| out.result["design_volume"].as_f64().map(|v| format!(", a steam generator for {v:.1} m³")))
                    .unwrap_or_default();
                frame.set_status_text(&format!("{placed} component(s) laid out{heater}; Check Bathrooms finds {problems} error(s)"), 0);
            }
            Err(e) => error(&frame, &format!("The layout was not made:\n{}", e.message)),
        }
    }

    /// Architecture › Make Product (ARC-BATH-006): a manufacturer's model file made into a product
    /// type of a generic component's kind, with its manufacturer data.
    pub(crate) fn make_product(&mut self) {
        let frame = self.frame;
        let Some(store) = self.store_now() else {
            return frame.set_status_text("Open or create a project first (File > New)", 0);
        };
        if !store.iter().any(|e| riviera_core::bath_components::is_component(e)) {
            if let Err(e) = self.run("bath.load_components", json!({})) {
                return error(&frame, &e);
            }
        }
        let Some(store) = self.store_now() else { return };
        let order = riviera_core::bath_components::names();
        let mut generics: Vec<(riviera_core::ElementId, String)> = store
            .iter()
            .filter(|e| riviera_core::bath_components::is_component(e))
            .filter_map(|e| e.name.clone().filter(|n| order.contains(&n.as_str())).map(|n| (e.id(), n)))
            .collect();
        generics.sort_by_key(|g| order.iter().position(|n| *n == g.1).unwrap_or(usize::MAX));
        let Some(path) = self.open_target("The manufacturer's model", "3D models (*.obj;*.stl;*.ply;*.gltf;*.glb)|*.obj;*.stl;*.ply;*.gltf;*.glb") else { return };
        let stem = std::path::Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let f = super::reinforcement_ui::Form::new(&frame, "Make Product");
        let names: Vec<String> = generics.iter().map(|g| g.1.clone()).collect();
        let generic = f.choice("In place of", &names, 0);
        let name = f.text("Name", &stem, "The product type's name");
        let turns = ["0°", "90°", "180°", "270°"].map(str::to_owned);
        let turn = f.choice("Turn", &turns, 0);
        let units_list = ["As the file says", "Millimetres", "Centimetres", "Metres", "Inches", "Feet"].map(str::to_owned);
        let units = f.choice("Units", &units_list, 0);
        let manufacturer = f.text("Manufacturer", "", "");
        let model = f.text("Model", "", "Its name in the manufacturer's range");
        let reference = f.text("Model reference", "", "");
        let article = f.text("Article number", "", "");
        let ok = f.show("Its back goes to the wall: turn it until its back faces +y. It is raised to the placeholder's mounting height and takes the placeholder's pipe connections.", "Make");
        let k = generic.get_selection().map_or(0, |k| k as usize);
        let unit = ["auto", "mm", "cm", "m", "in", "ft"][units.get_selection().unwrap_or(0) as usize];
        let args = json!({
            "generic": generics.get(k).map(|g| g.0 .0),
            "path": path,
            "name": name.get_value(),
            "turn": 90.0 * f64::from(turn.get_selection().unwrap_or(0)),
            "units": unit,
            "manufacturer": manufacturer.get_value(),
            "model_label": model.get_value(),
            "model_reference": reference.get_value(),
            "article_number": article.get_value(),
        });
        f.dialog.destroy();
        if !ok {
            return;
        }
        match self.run("bath.make_product", args) {
            Ok(_) => frame.set_status_text("Product made: select placeholders and use Swap for Product, or place it with Bathroom", 0),
            Err(e) => error(&frame, &format!("The product was not made:\n{e}")),
        }
    }

    /// Architecture › Swap for Product (ARC-BATH-006): the selected fixtures get a product type
    /// of their kind; the report says what changed.
    pub(crate) fn swap_for_product(&mut self) {
        let frame = self.frame;
        let selected = self.selection();
        let Some(store) = self.store_now() else { return };
        let category = selected
            .iter()
            .filter_map(|id| store.get(*id))
            .find_map(|e| riviera_core::mesh_objects::type_of(&store, e).map(|(_, t)| t.category));
        let Some(category) = category else {
            return error(&frame, "Select the placeholder fixtures to replace first.");
        };
        let products: Vec<(riviera_core::ElementId, String)> = store
            .iter()
            .filter(|e| riviera_core::bath_components::is_component(e) && riviera_core::mesh_objects::mesh_object_type(e).is_some_and(|t| t.category == category))
            .map(|e| (e.id(), e.name.clone().unwrap_or_default()))
            .collect();
        if products.is_empty() {
            return error(&frame, "No other component of this kind: make one with Make Product first.");
        }
        let names: Vec<String> = products.iter().map(|p| p.1.clone()).collect();
        let labels: Vec<&str> = names.iter().map(String::as_str).collect();
        let d = SingleChoiceDialog::builder(&frame, &format!("Give the {} selected the type:", selected.len()), "Swap for Product", &labels).build();
        if d.show_modal() != ID_OK {
            return;
        }
        let k = usize::try_from(d.get_selection()).unwrap_or(0);
        d.destroy();
        let ids: Vec<u64> = selected.iter().map(|id| id.0).collect();
        let report = match self.run("bath.swap_product", json!({ "element_ids": ids, "type_id": products[k].0 .0 })) {
            Ok(v) => v,
            Err(e) => return error(&frame, &format!("Not swapped:\n{e}")),
        };
        self.refresh_scene(&frame);
        let mut lines = Vec::new();
        for r in report.as_array().cloned().unwrap_or_default() {
            let size = |k: &str| r[k].as_array().map(|v| v.iter().map(|x| format!("{:.0}", x.as_f64().unwrap_or(0.0) * 1000.0)).collect::<Vec<_>>().join(" × ")).unwrap_or_default();
            lines.push(format!("{}: {} mm, was {} mm", self.element_label(&r["element"]), size("size_after"), size("size_before")));
            for m in r["connectors_moved"].as_array().cloned().unwrap_or_default() {
                lines.push(format!("  {} connection moved {:.0} mm", m["name"].as_str().unwrap_or_default(), m["distance"].as_f64().unwrap_or(0.0) * 1000.0));
            }
            for c in r["connectors_lost"].as_array().cloned().unwrap_or_default() {
                lines.push(format!("  {} connection: the product has none", c.as_str().unwrap_or_default()));
            }
            for f in r["findings_new"].as_array().cloned().unwrap_or_default() {
                lines.push(format!("  New: {}", f["message"].as_str().unwrap_or_default()));
            }
            for f in r["findings_gone"].as_array().cloned().unwrap_or_default() {
                lines.push(format!("  Resolved: {}", f["message"].as_str().unwrap_or_default()));
            }
        }
        let d = MessageDialog::builder(&frame, &lines.join("\n"), "Swap for Product").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build();
        d.show_modal();
    }

    /// Fixture Count (ARC-BATH-007): an occupancy and its load, then the fixtures it needs under
    /// the packs in use against those modelled.
    pub(crate) fn fixture_count(&mut self) {
        let frame = self.frame;
        let packs = match self.run("bath.rule_packs", json!({})) {
            Ok(v) => v.as_array().cloned().unwrap_or_default(),
            Err(e) => return error(&frame, &e),
        };
        // Every count row of the packs, those in use first: (pack, occupancy, label).
        let mut rows: Vec<(String, String, String)> = Vec::new();
        for in_use in [true, false] {
            for p in packs.iter().filter(|p| (p["in_use"] == true) == in_use) {
                for c in p["counts"].as_array().cloned().unwrap_or_default() {
                    let occ = c["occupancy"].as_str().unwrap_or_default().to_owned();
                    rows.push((p["id"].as_str().unwrap_or_default().to_owned(), occ.clone(), format!("{occ} — {} ({})", c["description"].as_str().unwrap_or_default(), p["title"].as_str().unwrap_or_default())));
                }
            }
        }
        if rows.is_empty() {
            return error(&frame, "No bathroom rule pack has fixture counts.");
        }
        let level = self.active_view().and_then(|v| v.plan.clone()).map(|p| p.level);
        let f = super::reinforcement_ui::Form::new(&frame, "Fixture Count");
        let labels: Vec<String> = rows.iter().map(|r| r.2.clone()).collect();
        let occ = f.choice("Occupancy", &labels, 0);
        let load = f.text("Occupant load", "100", "People");
        let share = f.text("Males (%)", "50", "The share of the load counted as male");
        let units = f.text("Units", "", "Dwelling or sleeping units, for rows counted per unit");
        let mode = f.choice("Rooms", &["Separate for each sex".to_owned(), "All-gender (IPC 2021 403.1.1)".to_owned()], 0);
        let scope_items = if level.is_some() { vec!["This plan's level".to_owned(), "The whole model".to_owned()] } else { vec!["The whole model".to_owned()] };
        let scope = f.choice("Compare with", &scope_items, 0);
        let ok = f.show("Counts the fixtures the occupancy needs and compares them with those modelled.", "Count");
        let k = occ.get_selection().map_or(0, |k| k as usize);
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let mut args = json!({
            "occupancy": rows[k].1, "pack": rows[k].0,
            "occupant_load": num(&load).unwrap_or(0.0),
            "male_share": num(&share).unwrap_or(50.0) / 100.0,
            "all_gender": mode.get_selection() == Some(1),
        });
        if let Some(u) = num(&units) {
            args["units"] = json!(u);
        }
        if let (Some(l), Some(0)) = (level, scope.get_selection()) {
            args["level_id"] = json!(l);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        let c = match self.run("bath.fixture_count", args) {
            Ok(v) => v,
            Err(e) => return error(&frame, &e),
        };
        let mut lines = vec![format!("{} — {} ({})", c["row"].as_str().unwrap_or_default(), c["description"].as_str().unwrap_or_default(), c["source"].as_str().unwrap_or_default()), String::new()];
        for n in c["needs"].as_array().cloned().unwrap_or_default() {
            let name = n["fixture"].as_str().unwrap_or_default().replace('_', " ");
            let split = match (n["male"].as_u64(), n["female"].as_u64()) {
                (Some(m), Some(fe)) => format!(" ({m} male, {fe} female)"),
                (Some(m), None) => format!(" ({m} male)"),
                (None, Some(fe)) => format!(" ({fe} female)"),
                _ => String::new(),
            };
            let short = n["short"].as_u64().unwrap_or(0);
            lines.push(format!("{name}: {} needed{split}, {} modelled{}", n["required"], n["modelled"], if short > 0 { format!(" — {short} short") } else { String::new() }));
        }
        let sub = c["urinals_may_replace"].as_u64().unwrap_or(0);
        if sub > 0 {
            lines.push(format!("\nUrinals may replace up to {sub} of the males' WCs."));
        }
        let d = MessageDialog::builder(&frame, &lines.join("\n"), "Fixture Count").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build();
        d.show_modal();
    }

    /// Toilet Core (ARC-BATH-008): on the selected wall, compartments, urinals and lavatories from
    /// numbers or an occupancy, along its whole face on the plan's side.
    pub(crate) fn toilet_core(&mut self) {
        let frame = self.frame;
        let Some(store) = self.store_now() else { return };
        let Some(wall) = self.selection().into_iter().find(|id| store.get(*id).is_some_and(|e| riviera_core::walls::wall_recipe(e).is_some())) else {
            return error(&frame, "Select the wall the toilet core stands against first.");
        };
        let Ok(g) = riviera_core::walls::regenerate(&store, store.get(wall).expect("selected")) else { return };
        let Some(level) = self.active_view().and_then(|v| v.plan.clone()).map(|p| p.level) else {
            return error(&frame, "Open a plan of the core's level first.");
        };
        let f = super::reinforcement_ui::Form::new(&frame, "Toilet Core");
        let side = f.choice("Room side", &["Interior face".to_owned(), "Exterior face".to_owned()], 0);
        let how = f.choice("Numbers", &["Given below".to_owned(), "Counted for an occupancy".to_owned()], 0);
        let wcs = f.text("WCs", "4", "Compartments, the accessible one included");
        let urinals = f.text("Urinals", "2", "");
        let lavatories = f.text("Lavatories", "3", "");
        let occ = f.text("Occupancy", "B", "As the count rules name it (B, A-3, workplace…)");
        let load = f.text("Occupant load", "100", "People in the building or floor");
        let sex = f.choice("Room for", &["Men".to_owned(), "Women".to_owned(), "All genders".to_owned()], 0);
        let acc = f.choice("Accessible compartment", &["Yes".to_owned(), "No".to_owned()], 0);
        let amb = f.choice("Ambulatory compartment", &["No".to_owned(), "Yes".to_owned()], 0);
        let ok = f.show("Lays out along the wall's whole face: an accessible compartment, standard ones, urinals with screens, lavatories. Too short a wall is reported with the shortfall.", "Lay Out");
        let num = |t: &TextCtrl| t.get_value().trim().parse::<f64>().ok();
        let s = if side.get_selection() == Some(1) { 1.0 } else { -1.0 };
        let n = g.normal();
        let off = g.width / 2.0 + 0.05;
        let start = [g.start[0] + n[0] * s * off, g.start[1] + n[1] * s * off];
        let end = [g.end[0] + n[0] * s * off, g.end[1] + n[1] * s * off];
        let mut args = json!({ "wall_id": wall.0, "start": start, "end": end, "level": level, "accessible": acc.get_selection() != Some(1), "ambulatory": amb.get_selection() == Some(1), "replace": true });
        if how.get_selection() == Some(1) {
            args["occupancy"] = json!(occ.get_value().trim());
            args["occupant_load"] = json!(num(&load).unwrap_or(0.0));
            args["sex"] = json!(["male", "female", "all"][sex.get_selection().unwrap_or(0) as usize]);
            args["urinals_for_wcs"] = json!(sex.get_selection() == Some(0));
        } else {
            args["wcs"] = json!(num(&wcs).unwrap_or(0.0) as u32);
            args["urinals"] = json!(num(&urinals).unwrap_or(0.0) as u32);
            args["lavatories"] = json!(num(&lavatories).unwrap_or(0.0) as u32);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        match self.run("bath.toilet_core", args) {
            Ok(c) => {
                self.refresh_scene(&frame);
                frame.set_status_text(&format!("Toilet core: {} WC(s), {} urinal(s), {} lavatory(ies)", c["wcs"], c["urinals"], c["lavatories"]), 0);
            }
            Err(e) => error(&frame, &format!("The core was not laid out:\n{e}")),
        }
    }

    /// Architecture › Build › Locker Room (ARC-BATH-009): lockers, tiers, showers, then a click
    /// inside a room closed by walls.
    pub(crate) fn locker_room(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Locker Room");
        let lockers = f.text("Lockers", "40", "Compartments");
        let tiers = f.choice("Lockers", &["Full height".to_owned(), "Two tier".to_owned()], 1);
        let showers = f.text("Gang showers", "6", "Heads along the end wall, 750 mm apart (0 for none)");
        let acc = f.choice("Accessible bench and lockers", &["Yes".to_owned(), "No".to_owned()], 0);
        let ok = f.show("Then click inside a room closed by walls, in a plan: lockers run along its long walls, benches down the middle, showers along one end.", "Choose Room");
        let num = |t: &TextCtrl| t.get_value().trim().parse::<f64>().ok().unwrap_or(0.0).max(0.0) as u32;
        let args = json!({ "command": "bath.locker_room", "lockers": num(&lockers), "tiers": if tiers.get_selection() == Some(1) { 2 } else { 1 }, "showers": num(&showers), "accessible": acc.get_selection() != Some(1), "replace": true });
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_room_tool();
        if let Some(tool) = self.room_tool.as_mut() {
            tool.layout = Some(args);
            frame.set_status_text("Locker Room: click inside the room; Esc ends", 0);
        }
    }

    /// Architecture › Build › Shower Falls (ARC-BATH-011): the fall and the drain, for the
    /// selected level-access showers.
    pub(crate) fn shower_falls(&mut self) {
        let frame = self.frame;
        let selected: Vec<serde_json::Value> = self.selection().into_iter().map(|id| json!(id.0)).collect();
        if selected.is_empty() {
            return error(&frame, "Select level-access showers first: a roll-in shower or a gang shower head standing on a floor.");
        }
        let f = super::reinforcement_ui::Form::new(&frame, "Shower Falls");
        let fall = f.text("Fall (%)", "", "Empty for the rule packs' default (2 % under the IPC, 1:60 under Approved Document M)");
        let drain = f.choice("Drain", &["Centre".to_owned(), "Linear, along the back".to_owned()], 0);
        let ok = f.show("A floor drain is placed and the floor under each shower warps to fall to it.", "Lay Falls");
        let mut args = json!({ "element_ids": selected, "drain": if drain.get_selection() == Some(1) { "linear" } else { "centre" } });
        if let Ok(p) = fall.get_value().trim().trim_end_matches('%').trim().parse::<f64>() {
            args["fall"] = json!(p / 100.0);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        match self.run("bath.shower_falls", args) {
            Ok(out) => {
                self.refresh_scene(&frame);
                frame.set_status_text(&format!("Shower falls: {} drain(s) placed, {} floor(s) warped", out["drains"].as_array().map_or(0, Vec::len), out["floors"].as_array().map_or(0, Vec::len)), 0);
            }
            Err(e) => error(&frame, &format!("The falls were not laid:\n{e}")),
        }
    }

    /// Architecture › Build › Waterproofing (ARC-BATH-012): the waterproofing schedule for the
    /// selected fixtures or all, offered as CSV.
    pub(crate) fn waterproofing(&mut self) {
        let frame = self.frame;
        let selected: Vec<serde_json::Value> = self.selection().into_iter().map(|id| json!(id.0)).collect();
        let s = match self.run("bath.waterproofing", json!({ "element_ids": selected })) {
            Ok(v) => v,
            Err(e) => return error(&frame, &e),
        };
        let extents = s["extents"].as_array().cloned().unwrap_or_default();
        if extents.is_empty() {
            return error(&frame, "No waterproofing: the bathroom rule packs in use ask for none at these fixtures (the IPC pack covers showers, tubs and urinals; Approved Document M wet rooms).");
        }
        let n = |v: &serde_json::Value| v.as_f64().unwrap_or(0.0);
        let mut csv = vec!["Fixture,Kind,Surface,On,Width (m),Height (m),Area (m²),Rule,Class".to_owned()];
        let mut lines = Vec::new();
        for e in &extents {
            let fixture = self.element_label(&e["element"]);
            let on = if e["host"].is_null() { String::new() } else { self.element_label(&e["host"]) };
            let (surface, rule, class) = (e["surface"].as_str().unwrap_or_default(), e["rule"].as_str().unwrap_or_default(), e["class"].as_str().unwrap_or_default());
            lines.push(format!("{fixture}: {surface} {:.2} × {:.2} m = {:.2} m² ({rule})", n(&e["width"]), n(&e["height"]), n(&e["area"])));
            csv.push(format!("\"{fixture}\",{},{surface},\"{on}\",{:.3},{:.3},{:.3},\"{rule}\",{class}", e["fixture"].as_str().unwrap_or_default(), n(&e["width"]), n(&e["height"]), n(&e["area"])));
        }
        lines.push(format!("\nWalls {:.2} m², floors {:.2} m². Copy the schedule as CSV?", n(&s["wall_area"]), n(&s["floor_area"])));
        let d = MessageDialog::builder(&frame, &lines.join("\n"), "Waterproofing").with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconInformation).build();
        if d.show_modal() == ID_YES {
            Clipboard::get().set_text(&csv.join("\n"));
            frame.set_status_text("The waterproofing schedule was copied as CSV", 0);
        }
    }

    /// Architecture › Build › Sauna (ARC-BATH-014): tiers, surfaces and heater, then a click
    /// inside a room closed by walls.
    pub(crate) fn sauna(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Sauna");
        let tiers = f.choice("Benches", &["Two tiers".to_owned(), "One tier".to_owned()], 0);
        let glass = f.text("Uninsulated surface (m²)", "0", "Glass door, tile, brick or glass block: each m² sizes as 1.2 m³ more");
        let logs = f.choice("Walls", &["Insulated, panelled".to_owned(), "Log (× 1.5)".to_owned()], 0);
        let kw = f.text("Heater (kW)", "", "Empty to size it from the room");
        let acc = f.choice("Lowest bench", &["Standard".to_owned(), "Accessible (ADA §903: 24 in deep, 18 in high)".to_owned()], 0);
        let vents = f.choice("Vents", &["Inlet low by the heater, outlet high on the far wall".to_owned(), "Inlet low by the heater, larger outlet low on the far wall".to_owned(), "None".to_owned()], 0);
        let door = f.choice("Door", &["Vent near its top; notice outside (IMC 914)".to_owned(), "Leave the door as it is".to_owned()], 0);
        let ok = f.show("Then click inside a room closed by walls, in a plan: benches run along its longer wall, the heater goes on an end wall. The notice goes outside the door only where the building is not a dwelling (Manage › Model Properties).", "Choose Room");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let vents = ["high_outlet", "low_outlet", "none"][vents.get_selection().unwrap_or(0) as usize];
        let mut args = json!({ "command": "bath.sauna", "tiers": if tiers.get_selection() == Some(1) { 1 } else { 2 }, "uninsulated_area": num(&glass).unwrap_or(0.0).max(0.0), "log_walls": logs.get_selection() == Some(1), "accessible": acc.get_selection() == Some(1), "vents": vents, "door_fittings": door.get_selection() != Some(1), "replace": true });
        if let Some(k) = num(&kw) {
            args["heater_kw"] = json!(k);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_room_tool();
        if let Some(tool) = self.room_tool.as_mut() {
            tool.layout = Some(args);
            frame.set_status_text("Sauna: click inside the room; Esc ends", 0);
        }
    }

    /// Architecture › Build › Steam Room (ARC-BATH-015): the ceiling, finish and outside wall,
    /// then a click inside a room closed by walls.
    pub(crate) fn steam_room(&mut self) {
        let frame = self.frame;
        let f = super::reinforcement_ui::Form::new(&frame, "Steam Room");
        let height = f.text("Ceiling, high side (mm)", "2130", "Over the bench wall; it falls 1:6 across the room");
        let materials = ["tile", "acrylic", "glass", "stone"];
        let labels: Vec<String> = ["Ceramic tile (× 1.0)", "Acrylic or fibreglass (× 0.65)", "Glass tile or block (× 1.35)", "Natural stone (× 1.65)"].iter().map(|s| (*s).to_owned()).collect();
        let material = f.choice("Walls' finish", &labels, 0);
        let outside = f.choice("Outside wall", &["No".to_owned(), "Yes (+10 %)".to_owned()], 0);
        let acc = f.choice("Bench", &["Standard (450 deep)".to_owned(), "Accessible (ADA §903: 24 in deep, 18 in high)".to_owned()], 0);
        let assembly = f.choice("Walls", &["Vapour-proof (TCNA SR614 assembly)".to_owned(), "As they are".to_owned()], 0);
        let generator = f.choice("Generator", &["Outside, behind the steam head's wall".to_owned(), "None (place one later)".to_owned()], 0);
        let ok = f.show("Then click inside a room closed by walls, in a plan: the bench runs along its longer wall, the steam head goes on an end wall, and a floor under it falls to a drain. Move the generator to a dry, ventilated closet within 50 ft (15 m) of the head.", "Choose Room");
        let mut args = json!({ "command": "bath.steam_room", "material": materials[material.get_selection().unwrap_or(0) as usize], "exterior_wall": outside.get_selection() == Some(1), "accessible": acc.get_selection() == Some(1), "wall_assembly": assembly.get_selection() != Some(1), "generator": generator.get_selection() != Some(1), "replace": true });
        if let Ok(mm) = height.get_value().trim().parse::<f64>() {
            args["height"] = json!(mm / 1000.0);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_room_tool();
        if let Some(tool) = self.room_tool.as_mut() {
            tool.layout = Some(args);
            frame.set_status_text("Steam Room: click inside the room; Esc ends", 0);
        }
    }

    /// View › Create › Bathroom Elevations (ARC-BATH-018): a click inside a room closed by walls.
    pub(crate) fn bathroom_elevations(&mut self) {
        let frame = self.frame;
        self.start_room_tool();
        if let Some(tool) = self.room_tool.as_mut() {
            tool.layout = Some(json!({ "command": "bath.elevations" }));
            frame.set_status_text("Bathroom Elevations: click inside the room in a plan; Esc ends", 0);
        }
    }

    /// View › Create › Bathroom Schedules (ARC-BATH-018).
    pub(crate) fn bathroom_schedules(&mut self) {
        let frame = self.frame;
        match self.run("bath.schedules", json!({})) {
            Ok(_) => {
                self.refresh_scene(&frame);
                frame.set_status_text("Plumbing Fixture Schedule and Bathroom Accessories made: see Project Browser › Schedules", 0);
            }
            Err(e) => error(&frame, &e),
        }
    }

    /// View › Coordination › Bathroom Report (ARC-BATH-019): the compliance report as HTML,
    /// opened in the browser.
    pub(crate) fn bath_report(&mut self) {
        let frame = self.frame;
        let Some(mut path) = self.save_target("Bathroom Compliance Report", "HTML (*.html)|*.html", "bathroom report.html") else { return };
        if !path.to_lowercase().ends_with(".html") {
            path.push_str(".html");
        }
        match self.run("bath.report", json!({ "path": path })) {
            Ok(r) => {
                frame.set_status_text(&format!("Bathroom report: {} error(s), {} note(s), written to {path}", r["errors"], r["notes"]), 0);
                if !path.starts_with("fileengine://") {
                    launch_default_browser(&format!("file://{path}"), BrowserLaunchFlags::Default);
                }
            }
            Err(e) => error(&frame, &e),
        }
    }

    /// View › Coordination › Issues from Bathrooms (ARC-BATH-019).
    pub(crate) fn bath_issues(&mut self) {
        let frame = self.frame;
        match self.run("bcf.from_bath_check", json!({})) {
            Ok(r) => {
                let (made, already, resolved) = (r["topic_ids"].as_array().map_or(0, Vec::len), r["already"].as_u64().unwrap_or(0), r["resolved"].as_array().map_or(0, Vec::len));
                self.refresh_scene(&frame);
                frame.set_status_text(&format!("{made} BCF issue(s) made from Check Bathrooms' errors{}{}; see the Issues pane", if already > 0 { format!(", {already} already had one") } else { String::new() }, if resolved > 0 { format!(", {resolved} resolved") } else { String::new() }), 0);
            }
            Err(e) => error(&frame, &e),
        }
    }

    /// Architecture › Build › Tiles (ARC-BATH-020): the tile and its set-out, then a click on
    /// the side of the selected wall to tile, or on a floor.
    pub(crate) fn tiles(&mut self) {
        let frame = self.frame;
        let wall = self.selection().into_iter().find(|id| self.store_now().and_then(|s| s.get(*id).and_then(riviera_core::walls::wall_recipe)).is_some());
        let f = super::reinforcement_ui::Form::new(&frame, "Tiles");
        let surface = f.choice("Surface", &[if wall.is_some() { "The selected wall".to_owned() } else { "A wall (select it first)".to_owned() }, "The floor".to_owned()], if wall.is_some() { 0 } else { 1 });
        let w = f.text("Tile width (mm)", "", "Default 300 on walls, 600 on floors");
        let h = f.text("Tile height (mm)", "", "Default 600");
        let joint = f.text("Joint (mm)", "", "Default 3 on walls, 5 on floors");
        let datum = f.choice("Set out from", &["The middle (equal cuts)".to_owned(), "The start corner (full tile)".to_owned(), "The end corner (full tile)".to_owned()], 0);
        let pattern = f.choice("Pattern", &["Stack".to_owned(), "Half bond".to_owned()], 0);
        let top = f.text("Up to (mm, walls)", "", "Default the wall's top");
        let ok = f.show("Then click in a plan: on the side of the wall to tile (its face between the walls meeting it), or on the floor.", "Pick");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok().map(|v| v / 1000.0);
        let datum = ["centred", "corner", "end_corner"][datum.get_selection().unwrap_or(0) as usize];
        let mut args = json!({ "command": "tile.create", "datum": datum, "pattern": if pattern.get_selection() == Some(1) { "half_bond" } else { "stack" } });
        let on_wall = surface.get_selection() == Some(0);
        if on_wall {
            let Some(wall) = wall else {
                f.dialog.destroy();
                return error(&frame, "Select the wall to tile first.");
            };
            args["wall_id"] = json!(wall.0);
            if let Some(t) = num(&top) {
                args["top"] = json!(t);
            }
        }
        if let (Some(a), Some(b)) = (num(&w), num(&h)) {
            args["tile"] = json!([a, b]);
        }
        if let Some(j) = num(&joint) {
            args["joint"] = json!(j);
        }
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_room_tool();
        if let Some(tool) = self.room_tool.as_mut() {
            tool.layout = Some(args);
            frame.set_status_text("Tiles: click on the side to tile, or on the floor; Esc ends", 0);
        }
    }
}
