// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! A selected grab bar's rows (ARC-BATH-003): its diameter, its clear gap to the wall and the
//! height of its top, changed with `grab_bar.set`; the rule it was placed by, if any.

use riviera_core::ElementId;
use riviera_core::grab_bars;
use serde_json::json;
use wxdragon::prelude::*;

use super::{Gui, error};

impl Gui {
    pub(crate) fn grab_bar_rows(&mut self, grid: &PropertyGrid, id: ElementId) {
        let Some(r) = self.store_now().and_then(|s| s.get(id).and_then(grab_bars::recipe)) else { return };
        grid.append(Property::category("Grab Bar", "c_grab_bar"));
        grid.append(Property::string("Diameter", "gb:diameter", self.format_length(r.diameter)));
        grid.append(Property::string("Gap to Wall", "gb:gap", self.format_length(r.gap)));
        let top = r.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max) + r.diameter / 2.0;
        grid.append(Property::string("Top Height", "gb:top", self.format_length(top)));
        grid.set_help_string("gb:top", "The top of the bar above its level; the whole bar moves");
        if let Some(rule) = r.rule {
            grid.append(Property::string("Placed by Rule", "gb_rule", rule));
            grid.set_property_read_only("gb_rule", true);
        }
    }

    /// An edit of a grab bar row; `false` if `property` is not one.
    pub(crate) fn grab_bar_edit(&mut self, property: &str, text: Option<String>) -> bool {
        let Some(row) = property.strip_prefix("gb:") else { return false };
        let frame = self.frame;
        let Some(v) = self.parse_length_text(&text.unwrap_or_default()) else { return true };
        let ids: Vec<ElementId> = match self.store_now() {
            Some(s) => self.selection().into_iter().filter(|i| s.get(*i).and_then(grab_bars::recipe).is_some()).collect(),
            None => return true,
        };
        let mut args = json!({ "element_ids": ids });
        args[row] = json!(v);
        if let Err(e) = self.run("grab_bar.set", args) {
            error(&frame, &format!("The grab bar was not changed:\n{e}"));
        }
        self.properties_stale = true;
        self.refresh_scene(&frame);
        true
    }
}
