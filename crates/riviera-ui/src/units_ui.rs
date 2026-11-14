// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Modular units (ARC-GRP-004): Create Unit Type makes the selection a unit type with its bay,
//! its corner clicked in a plan; Unit Array repeats a type along a corridor clicked start to end,
//! on a range of levels; Connect Units runs every copy's open connectors into the selected mains;
//! Edit Unit opens a copy in the group editor with the rest of the model hidden (or faded, Show
//! Context), and Finish or Cancel brings the model back.
//! Atrium (ARC-GLZ-018): a void clicked corner to corner, opened up to a chosen level and roofed.

use serde_json::json;
use wxdragon::prelude::*;

use riviera_core::ElementId;

use super::{Gui, error};

/// The unit being edited in isolation: the view isolating it and its members.
#[derive(Debug, Clone)]
pub(crate) struct UnitEdit {
    pub view: Option<ElementId>,
    pub members: Vec<ElementId>,
    pub faded: bool,
    pub copies: u64,
}

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

impl Gui {
    /// Architecture › Model › Edit Unit: the selected copy (or one of its members, or an array),
    /// or a unit type chosen from a list, opened in the group editor with everything else hidden.
    pub(crate) fn edit_unit(&mut self) {
        let frame = self.frame;
        let Some(store) = self.store_now() else { return };
        let selected = self.selection().into_iter().find(|id| {
            store.get(*id).is_some_and(|e| {
                riviera_core::modular_units::copy_info(e).is_some()
                    || riviera_core::modular_units::array_recipe(e).is_some()
                    || riviera_core::groups::instance_of(e).and_then(|g| store.find_by_guid(&riviera_core::GlobalId::parse(&g)?)).and_then(|g| store.get(g)).and_then(riviera_core::modular_units::copy_info).is_some()
            })
        });
        let args = match selected {
            Some(id) => json!({ "element_id": id }),
            None => {
                let types = riviera_core::modular_units::types(&store);
                if types.is_empty() {
                    return error(&frame, "There are no unit types: make one with Create Unit Type.");
                }
                let labels: Vec<&str> = types.iter().map(|t| t.name.as_str()).collect();
                let dialog = SingleChoiceDialog::builder(&frame, "Unit type to edit (its first copy opens; every copy follows):", "Edit Unit", &labels).build();
                if dialog.show_modal() != ID_OK {
                    return;
                }
                let Some(t) = usize::try_from(dialog.get_selection()).ok().and_then(|i| types.get(i)) else { return };
                json!({ "unit_type": t.id })
            }
        };
        let v = match self.run("unit.edit", args) {
            Ok(v) => v,
            Err(e) => return error(&frame, &format!("The unit was not opened:\n{e}")),
        };
        self.properties_stale = true;
        self.refresh_scene(&frame);
        let members: Vec<ElementId> = v["members"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(ElementId)).collect()).unwrap_or_default();
        let copies = v["copies"].as_u64().unwrap_or(0);
        self.unit_edit = Some(UnitEdit { view: self.active_view().and_then(|v| v.element), members: members.clone(), faded: false, copies });
        self.isolate_unit();
        self.select_ids(&members, super::select::Combine::Replace);
        self.zoom_to_fit();
        frame.set_status_text(
            &format!(
                "Editing unit {} ({}) alone: Finish updates all {copies} copies",
                v["number"].as_str().unwrap_or("?"),
                v["unit_type"].as_str().unwrap_or("?"),
            ),
            0,
        );
    }

    /// The unit being edited isolated in the active view: the rest hidden, or faded.
    fn isolate_unit(&mut self) {
        let Some(u) = self.unit_edit.clone() else { return };
        self.temp("view.temp_isolate", &u.members, json!({ "by": "element", "ghost_others": u.faded }));
    }

    /// Edit Group › Show Context: the building around the unit being edited faded in, or hidden
    /// again.
    pub(crate) fn unit_context(&mut self) {
        let Some(u) = &mut self.unit_edit else {
            return self.frame.set_status_text("Show Context: open a unit with Edit Unit first", 0);
        };
        u.faded = !u.faded;
        let faded = u.faded;
        self.isolate_unit();
        self.frame.set_status_text(if faded { "The building around the unit is shown faded" } else { "The unit alone" }, 0);
    }

    /// After Finish or Cancel in the group editor: a unit's isolation ends.
    pub(crate) fn unit_edit_closed(&mut self, finished: bool) {
        let Some(u) = self.unit_edit.take() else { return };
        let args = match u.view {
            Some(v) => json!({ "view_id": v }),
            None => json!({}),
        };
        let _ = self.run("view.temp_reset", args);
        self.clear_selection();
        self.after_temp_change_pub();
        if finished {
            self.frame.set_status_text(&format!("Unit edited: all {} copies follow it", u.copies), 0);
        }
    }
}

impl Gui {
    /// Architecture › Build › Atrium (ARC-GLZ-018): the top level and roof, then the void's two
    /// opposite corners on the atrium floor's level.
    pub(crate) fn atrium(&mut self) {
        let frame = self.frame;
        let levels: Vec<(String, serde_json::Value)> = self.run("level.list", json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().map(|l| (l["name"].as_str().unwrap_or_default().to_owned(), l["id"].clone())).collect();
        if levels.len() < 2 {
            return error(&frame, "An atrium needs at least two levels.");
        }
        let names: Vec<String> = levels.iter().map(|(n, _)| n.clone()).collect();
        let f = super::reinforcement_ui::Form::new(&frame, "Atrium");
        let top = f.choice("Top level", &names, names.len() - 1);
        let form = f.choice("Roof", &["Ridge".to_owned(), "Pyramid".to_owned(), "Barrel vault".to_owned(), "Flat".to_owned()], 0);
        let offset = f.text("Roof above the top level (m)", "4.0", "The glazed roof's plate");
        let reach = f.text("Roof reaches past the void (m)", "0.3", "On every side; more covers a terrace or a pool");
        let ok = f.show("Then click two opposite corners of the void on the atrium floor's level. Its floors above are opened, guarded 42 in high, and roofed in glass.", "Pick the Void");
        let num = |t: &TextCtrl| t.get_value().trim().replace(',', ".").parse::<f64>().ok();
        let roof = ["ridge", "pyramid", "barrel", "flat"][form.get_selection().unwrap_or(0).min(3) as usize];
        let args = json!({
            "top_level": levels[top.get_selection().unwrap_or(0) as usize].1,
            "form": roof,
            "roof_offset": num(&offset).unwrap_or(4.0),
            "overhang": num(&reach).unwrap_or(0.3),
        });
        f.dialog.destroy();
        if !ok {
            return;
        }
        self.start_picking("atrium.create", vec![json!({ "key": "from", "kind": "at", "prompt": "Atrium: click a corner of the void" }), json!({ "key": "to", "kind": "point", "prompt": "Atrium: click the opposite corner" })], args);
    }

    pub(crate) fn atrium_finish(&mut self, mut args: serde_json::Value) {
        let frame = self.frame;
        let from = args["from"].take();
        let to = args["to"].take();
        let (a, b) = ([from["point"][0].as_f64().unwrap_or(0.0), from["point"][1].as_f64().unwrap_or(0.0)], [to[0].as_f64().unwrap_or(0.0), to[1].as_f64().unwrap_or(0.0)]);
        let o = args.as_object_mut().expect("an object");
        o.remove("from");
        o.remove("to");
        args["base_level"] = from["level"].clone();
        args["outline"] = json!([[a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])], [a[0].min(b[0]), a[1].max(b[1])]]);
        match self.run("atrium.create", args) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let mut text = format!("Atrium through {}", v["opened"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default());
                if let Ok(r) = self.run("atrium.check", json!({ "atrium_id": v["atrium"] })) {
                    let r = &r[0];
                    text.push_str(&format!("\n{} storeys, {:.1} m high; smoke exhaust about {:.0} m³/s", r["storeys"], r["height"].as_f64().unwrap_or(0.0), r["smoke"]["exhaust"].as_f64().unwrap_or(0.0)));
                    for f in r["findings"].as_array().into_iter().flatten() {
                        text.push_str(&format!("\n• {}: {}", f["rule"].as_str().unwrap_or(""), f["message"].as_str().unwrap_or("")));
                    }
                }
                MessageDialog::builder(&frame, &text, "Atrium").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build().show_modal();
            }
            Err(e) => error(&frame, &format!("The atrium was not made:\n{e}")),
        }
    }
}
