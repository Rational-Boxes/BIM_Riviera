// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! The Type row of a selected element (DM-TYPE-002): the types of its kind in the model, then
//! those it could load (Riviera's starter types and the component libraries, `type.catalogue`),
//! then Duplicate Type…. Choosing a type not yet loaded loads it (`type.load`) and gives it to the
//! selection (`element.set_type`), as one step each.

use riviera_core::ElementId;
use serde_json::{Value, json};
use wxdragon::prelude::*;

use super::{Gui, error};

const DUPLICATE: &str = "Duplicate Type…";

/// The Type row shown: its entries by label.
#[derive(Debug, Clone)]
pub(crate) struct TypePicker {
    pub element: ElementId,
    pub current: Option<ElementId>,
    /// A placing tool's row instead of a selection's (`element` is then the tool's type): the
    /// tool's own type row key, which takes the chosen type's name.
    pub tool: Option<&'static str>,
    /// Label, catalogue name, type when loaded.
    pub entries: Vec<(String, String, Option<ElementId>)>,
}

impl Gui {
    /// Appends the Type row for the selected `element` of type `current`; `false` (nothing
    /// appended) when its kind has no catalogue, so the caller shows its own.
    pub(crate) fn type_picker_row(&mut self, grid: &PropertyGrid, element: ElementId, current: Option<ElementId>) -> bool {
        self.type_picker_rows(grid, element, current, None)
    }

    /// The Type row of a placing tool whose type is `current`: choosing one sets the tool's type
    /// through its own row `tool` (`it_type`, `wt_type`).
    pub(crate) fn type_picker_tool_row(&mut self, grid: &PropertyGrid, current: ElementId, tool: &'static str) -> bool {
        self.type_picker_rows(grid, current, Some(current), Some(tool))
    }

    fn type_picker_rows(&mut self, grid: &PropertyGrid, element: ElementId, current: Option<ElementId>, tool: Option<&'static str>) -> bool {
        let Ok(list) = self.run("type.catalogue", json!({ "element_id": element })) else {
            return false;
        };
        let mut entries = Vec::new();
        for e in list.as_array().into_iter().flatten() {
            let name = e["name"].as_str().unwrap_or_default().to_owned();
            let id = e["type_id"].as_u64().map(ElementId);
            let label = match id {
                Some(_) => name.clone(),
                None => format!("{name} (from {})", e["source"].as_str().unwrap_or("the catalogue")),
            };
            entries.push((label, name, id));
        }
        let mut choices: Vec<(String, i32)> = entries.iter().enumerate().map(|(i, e)| (e.0.clone(), i as i32)).collect();
        if current.is_some() {
            choices.push((DUPLICATE.to_owned(), choices.len() as i32));
        }
        let index = current.and_then(|c| entries.iter().position(|e| e.2 == Some(c))).map_or(-1, |i| i as i32);
        grid.append(Property::enumeration("Type", "tp_type", choices, index));
        grid.set_help_string("tp_type", "The type: those in the model, then those that can be loaded (starter types and component libraries); Duplicate Type… makes a new one from this one");
        self.type_picker = Some(TypePicker { element, current, tool, entries });
        true
    }

    /// A Type Name row renaming the current type (`type.rename`), for kinds without their own.
    pub(crate) fn type_name_row(&mut self, grid: &PropertyGrid, current: Option<ElementId>) {
        let Some(t) = current else { return };
        if self.type_picker.is_none() {
            self.type_picker = Some(TypePicker { element: t, current, tool: None, entries: Vec::new() });
        }
        let name = self.store_now().and_then(|s| s.get(t).and_then(|e| e.name.clone())).unwrap_or_default();
        grid.append(Property::string("Type Name", "tp_name", name));
        grid.set_help_string("tp_name", "Renames the type (every element of it shows the new name)");
    }

    /// An edit of the Type or Type Name row; `false` if `property` is neither.
    pub(crate) fn type_picker_edit(&mut self, property: &str, text: Option<String>) -> bool {
        if property != "tp_type" && property != "tp_name" {
            return false;
        }
        let frame = self.frame;
        let text = text.unwrap_or_default();
        let Some(p) = self.type_picker.clone() else { return true };
        let result = if property == "tp_name" {
            match p.current {
                Some(t) => self.run("type.rename", json!({ "type_id": t, "name": text.trim() })).map(|_| ()),
                None => Ok(()),
            }
        } else {
            self.pick_type(&p, &text)
        };
        if let Err(e) = result {
            error(&frame, &format!("The type was not changed:\n{e}"));
        }
        self.properties_stale = true;
        self.refresh_scene(&frame);
        true
    }

    fn pick_type(&mut self, p: &TypePicker, label: &str) -> Result<(), String> {
        let frame = self.frame;
        let chosen = if label == DUPLICATE {
            let Some(current) = p.current else { return Ok(()) };
            let proposed = self.store_now().map(|s| riviera_core::types::duplicate_name(&s, current)).unwrap_or_default();
            let dialog = TextEntryDialog::builder(&frame, "Name of the new type:", "Duplicate Type").with_default_value(&proposed).build();
            if dialog.show_modal() != ID_OK {
                return Ok(());
            }
            let name = dialog.get_value().unwrap_or(proposed);
            self.run("type.duplicate", json!({ "type_id": current, "new_name": name.trim() }))?
        } else {
            let Some((_, name, id)) = p.entries.iter().find(|e| e.0 == label) else {
                return Err(format!("{label} is not in the list"));
            };
            match id {
                Some(id) => json!(id),
                None => self.run("type.load", json!({ "element_id": p.element, "name": name }))?,
            }
        };
        let Some(t) = chosen.as_u64() else { return Ok(()) };
        if let Some(tool) = p.tool {
            let name = self.store_now().and_then(|s| s.get(ElementId(t)).and_then(|e| e.name.clone())).unwrap_or_default();
            let done = match tool {
                "it_type" => self.choose_insert_type(&name, &[]),
                _ => self.choose_wall_type(&name, &[]),
            };
            return done.unwrap_or(Ok(()));
        }
        if Some(ElementId(t)) == p.current {
            return Ok(());
        }
        // Every selected element of the same kind as the one shown takes it.
        let kind = |s: &riviera_core::Store, id: ElementId| s.get(id).map(|e| (e.class.clone(), e.param(riviera_core::walls::RECIPE_KEY).and_then(|r| r.get("kind")).cloned().unwrap_or(Value::Null)));
        let ids: Vec<ElementId> = match self.store_now() {
            Some(s) => {
                let k = kind(&s, p.element);
                self.selection().into_iter().filter(|id| kind(&s, *id) == k).collect()
            }
            None => vec![p.element],
        };
        self.run("element.set_type", json!({ "element_ids": ids, "type_id": t })).map(|_| ())
    }
}
