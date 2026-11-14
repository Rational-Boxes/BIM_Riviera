// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A selected pool's rows (ARC-POOL-001 … 003): its deck, water level, depths, cove, class,
//! coping and shell, each changed with `pool.set` (its shell, water and openings follow); its
//! entries listed, the selected one removable with Remove Entry.

use riviera_core::ElementId;
use riviera_core::pools::{self, PoolClass, ShellKind};
use serde_json::{Value, json};
use wxdragon::prelude::*;

use super::{Gui, error};

const CLASSES: [(PoolClass, &str); 4] = [(PoolClass::Public, "Public"), (PoolClass::Competition, "Competition"), (PoolClass::Residential, "Residential"), (PoolClass::Wading, "Wading")];
const SHELLS: [(ShellKind, &str); 4] = [(ShellKind::CastConcrete, "Cast concrete"), (ShellKind::Shotcrete, "Shotcrete"), (ShellKind::Panel, "Panel"), (ShellKind::OnePiece, "One-piece")];

fn choices<T>(list: &[(T, &str)]) -> Vec<(String, i32)> {
    list.iter().enumerate().map(|(i, c)| (c.1.to_owned(), i as i32)).collect()
}

impl Gui {
    /// The pool a selected element is, if it is one.
    fn pool_of_selection(&mut self) -> Option<(ElementId, pools::PoolRecipe)> {
        let store = self.store_now()?;
        let id = *self.selection().first()?;
        Some((id, pools::recipe(store.get(id)?)?))
    }

    /// A selected pool's rows; `false` if `id` is not a pool.
    pub(crate) fn pool_rows(&mut self, grid: &PropertyGrid, id: ElementId) -> bool {
        let Some(r) = self.store_now().and_then(|s| s.get(id).and_then(pools::recipe)) else { return false };
        grid.append(Property::category("Pool", "c_pool"));
        let len = |g: &mut Self, v: f64| g.format_length(v);
        grid.append(Property::string("Deck Height", "pl:deck", len(self, r.deck)));
        grid.set_help_string("pl:deck", "The deck's height above the level");
        grid.append(Property::string("Water Level Below Deck", "pl:water_level", len(self, r.water_level)));
        let shallow = r.profile.first().map_or(0.0, |s| s.depth);
        let deep = r.profile.iter().map(|s| s.depth).fold(0.0, f64::max);
        grid.append(Property::string("Shallow Depth", "pl:shallow", len(self, shallow)));
        grid.append(Property::string("Deepest Depth", "pl:deep", len(self, deep)));
        grid.set_help_string("pl:deep", "Every profile station at the deepest depth takes the new one");
        grid.append(Property::string("Cove Radius", "pl:cove", len(self, r.cove)));
        let class = CLASSES.iter().position(|c| c.0 == r.class()).map_or(0, |i| i as i32);
        grid.append(Property::enumeration("Class", "pl:class", choices(&CLASSES), class));
        grid.append(Property::category("Coping", "c_pool_coping"));
        grid.append(Property::string("Coping Width", "pl:coping_width", len(self, r.coping.width)));
        grid.append(Property::string("Coping Overhang", "pl:coping_overhang", len(self, r.coping.overhang)));
        grid.append(Property::string("Coping Thickness", "pl:coping_thickness", len(self, r.coping.thickness)));
        grid.append(Property::category("Shell", "c_pool_shell"));
        let kind = SHELLS.iter().position(|c| c.0 == r.shell.kind).map_or(0, |i| i as i32);
        grid.append(Property::enumeration("Construction", "pl:shell_kind", choices(&SHELLS), kind));
        grid.append(Property::string("Shell Thickness", "pl:shell_thickness", len(self, r.shell.thickness)));
        grid.append(Property::string("Tile Thickness", "pl:tile", r.shell.tile.map(|t| len(self, t)).unwrap_or_default()));
        grid.set_help_string("pl:tile", "Empty for an untiled shell");
        grid.append(Property::boolean("Membrane", "pl:membrane", r.shell.membrane));
        if !r.entries.is_empty() {
            grid.append(Property::category(format!("Entries ({})", r.entries.len()), "c_pool_entries"));
            for (k, e) in r.entries.iter().enumerate() {
                let what = serde_json::to_value(e.kind).ok().and_then(|v| v.as_str().map(|s| s.replace('_', " "))).unwrap_or_default();
                let key = format!("pl_entry{k}");
                grid.append(Property::string(format!("Entry {}", k + 1), &key, what));
                grid.set_property_read_only(&key, true);
            }
        }
        true
    }

    /// An edit of a pool row; `false` if `property` is not one.
    pub(crate) fn pool_edit(&mut self, property: &str, text: Option<String>) -> bool {
        let Some(row) = property.strip_prefix("pl:") else { return false };
        let frame = self.frame;
        let text = text.unwrap_or_default();
        let Some((id, r)) = self.pool_of_selection() else { return true };
        let length = |g: &mut Self| g.parse_length_text(&text);
        let mut args = json!({ "pool_id": id });
        let set: Option<(&str, Value)> = match row {
            "deck" | "water_level" | "cove" => length(self).map(|v| (row, json!(v))),
            "shallow" | "deep" => length(self).map(|v| {
                let deepest = r.profile.iter().map(|s| s.depth).fold(0.0, f64::max);
                let mut profile = r.profile.clone();
                if row == "shallow" {
                    if let Some(s) = profile.first_mut() {
                        s.depth = v;
                    }
                } else {
                    for s in profile.iter_mut().filter(|s| (s.depth - deepest).abs() < 1e-9) {
                        s.depth = v;
                    }
                }
                ("profile", json!(profile))
            }),
            "class" => CLASSES.iter().find(|c| c.1 == text).map(|c| ("class", json!(c.0))),
            "coping_width" | "coping_overhang" | "coping_thickness" => length(self).map(|v| {
                let mut c = r.coping;
                match row {
                    "coping_width" => c.width = v,
                    "coping_overhang" => c.overhang = v,
                    _ => c.thickness = v,
                }
                ("coping", json!(c))
            }),
            "shell_kind" | "shell_thickness" | "tile" | "membrane" => {
                let mut s = r.shell;
                let ok = match row {
                    "shell_kind" => SHELLS.iter().find(|c| c.1 == text).map(|c| s.kind = c.0).is_some(),
                    "shell_thickness" => length(self).map(|v| s.thickness = v).is_some(),
                    "tile" if text.trim().is_empty() => {
                        s.tile = None;
                        true
                    }
                    "tile" => length(self).map(|v| s.tile = Some(v)).is_some(),
                    _ => {
                        s.membrane = text == "true" || text == "1";
                        true
                    }
                };
                ok.then(|| ("shell", json!(s)))
            }
            _ => None,
        };
        let Some((key, value)) = set else {
            error(&frame, &format!("{text} is not a value for this row"));
            return true;
        };
        args[key] = value;
        if let Err(e) = self.run("pool.set", args) {
            error(&frame, &format!("The pool was not changed:\n{e}"));
        }
        self.properties_stale = true;
        self.refresh_scene(&frame);
        true
    }
}
