// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! The View tab's discipline selector (DOC-GFX-021): the active view from one discipline's point
//! of view, the rest ghosted or hidden (`view.discipline`, temporary like Hide/Isolate).

use serde_json::json;

use super::Gui;

/// The selector's buttons: action, discipline (none for the whole model) and its label.
pub(crate) const DISCIPLINES: [(&str, Option<&str>, &str); 7] = [
    ("ui.discipline_all", None, "the whole model"),
    ("ui.discipline_architecture", Some("architecture"), "Architecture"),
    ("ui.discipline_structure", Some("structure"), "Structure"),
    ("ui.discipline_mechanical", Some("mechanical"), "Mechanical"),
    ("ui.discipline_plumbing", Some("plumbing"), "Plumbing"),
    ("ui.discipline_electrical", Some("electrical"), "Electrical"),
    ("ui.discipline_site", Some("site"), "Site"),
];

impl Gui {
    /// Shows `discipline` (none: everything) in the active view.
    pub(crate) fn show_discipline(&mut self, action: &str) {
        let Some((_, d, label)) = DISCIPLINES.iter().find(|(a, ..)| *a == action).copied() else { return };
        let mut args = json!({ "disciplines": d.map(|x| vec![x]).unwrap_or_default(), "others": if self.discipline_hide { "hidden" } else { "ghost" } });
        if let Some(v) = self.active_view().and_then(|v| v.element) {
            args["view_id"] = json!(v);
        }
        match self.run("view.discipline", args) {
            Ok(r) => {
                self.discipline = d;
                for (a, x, _) in DISCIPLINES {
                    self.ribbon.set_toggled(a, x == d);
                }
                let text = match d {
                    None => "Showing the whole model".to_owned(),
                    Some(_) => format!("Showing {label}: {} elements, the other {} {} (Reset Hide/Isolate, HR, shows all)", r["shown"], r["others"], if self.discipline_hide { "hidden" } else { "ghosted" }),
                };
                self.frame.set_status_text(&text, 0);
            }
            Err(e) => self.frame.set_status_text(&e, 0),
        }
        self.after_temp_change_pub();
    }

    /// Ghost or hide the other disciplines, and show the current one again that way.
    pub(crate) fn toggle_discipline_hide(&mut self) {
        self.discipline_hide = !self.discipline_hide;
        self.ribbon.set_toggled("ui.discipline_hide", self.discipline_hide);
        if let Some(d) = self.discipline
            && let Some((a, ..)) = DISCIPLINES.iter().find(|(_, x, _)| *x == Some(d))
        {
            self.show_discipline(a);
        }
    }
}
