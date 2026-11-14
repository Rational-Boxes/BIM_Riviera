// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Ramps and Measure in the window (ARC-RAMP-001, ARC-MOD-023).
//!
//! - **Ramp** (Architecture › Ramp): in a plan, click the centreline's points from the bottom
//!   up; Enter makes the ramp (Esc drops the last point, then ends the tool). The tool's
//!   Properties set the height it climbs (a small one, 600 mm by default: a ramp to the next
//!   level would be very long) and its width; the status bar shows the run the type's maximum
//!   slope needs and what is drawn so far. Railings go on both sides when it rises over 150 mm
//!   (the tool's Railings row turns them off or on).
//! - **Modify | Ramps:** Edit Path draws a selected ramp's centreline again with the same tool
//!   (Enter replaces it); Flip Direction reverses which end is the bottom; Railings adds them to
//!   sides without one (`ramp.add_railings`). A selected ramp's Path row takes the points typed.
//! - **Measure** (Modify › Measure): click points; each segment and the running total show in
//!   the status bar. Esc clears the chain, a second Esc ends the tool.

use riviera_core::ElementId;
use riviera_render::RenderBackend;
use riviera_render::plan::Lines;
use serde_json::json;
use wxdragon::prelude::*;

use super::{Gui, error};

const PREVIEW: [f32; 4] = [0.05, 0.35, 0.95, 1.0];

/// The Ramp tool: its level, the points so far, the cursor, and the height and width it uses.
#[derive(Debug, Clone)]
pub struct RampTool {
    level: ElementId,
    points: Vec<[f64; 2]>,
    cursor: Option<[f64; 2]>,
    pub height: f64,
    pub width: f64,
    /// Railings on both sides (by default when it rises over 150 mm).
    pub railings: bool,
    /// Edit Path: the ramp whose centreline is drawn again.
    pub edit: Option<ElementId>,
}

/// The Measure tool: the chain clicked so far and the cursor.
#[derive(Debug, Clone, Default)]
pub struct MeasureTool {
    points: Vec<[f64; 2]>,
    cursor: Option<[f64; 2]>,
}

fn len(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

impl Gui {
    fn chain_lines(&mut self, points: &[[f64; 2]], cursor: Option<[f64; 2]>) {
        let z = self
            .active_view()
            .and_then(|v| v.plan.as_ref())
            .map_or(0.0, |p| p.range.cut) as f32
            + 0.004;
        let mut pts: Vec<[f64; 2]> = points.to_vec();
        if let (Some(c), false) = (cursor, points.is_empty()) {
            pts.push(c);
        }
        let lines = Lines {
            color: PREVIEW,
            width: 0.5,
            segments: pts
                .windows(2)
                .map(|w| {
                    [
                        [w[0][0] as f32, w[0][1] as f32, z],
                        [w[1][0] as f32, w[1][1] as f32, z],
                    ]
                })
                .collect(),
        };
        if let Some(v) = self.viewport() {
            v.set_overlay_lines(&[lines]);
        }
    }

    fn chain_point(&self, x: i32, y: i32, from: Option<[f64; 2]>) -> Option<[f64; 2]> {
        let lines = self.snap_lines();
        self.snapped_at(x, y, from, &lines).map(|s| s.point)
    }

    // ---- Ramp ----

    /// Architecture › Ramp.
    pub(crate) fn start_ramp_tool(&mut self) {
        let frame = self.frame;
        if !self.ensure_plan(false) {
            return;
        }
        let Some(level) = self
            .active_view()
            .and_then(|v| v.plan.as_ref())
            .map(|p| p.level)
        else {
            return frame.set_status_text("Ramps are drawn in plan views", 0);
        };
        let (height, width, railings) = self
            .ramp_tool
            .as_ref()
            .filter(|t| t.edit.is_none())
            .map_or((0.6, 1.2, true), |t| (t.height, t.width, t.railings));
        self.end_tool(&frame);
        self.end_insert_tool();
        self.end_modify();
        self.clear_selection();
        self.ramp_tool = Some(RampTool {
            level,
            points: Vec::new(),
            cursor: None,
            height,
            width,
            railings,
            edit: None,
        });
        self.properties_stale = true;
        self.ramp_prompt();
    }

    /// The selected Riviera ramps.
    fn selected_ramps(&mut self) -> Vec<ElementId> {
        let ids = self.selection();
        let Some(store) = self.store_now() else { return Vec::new() };
        ids.into_iter().filter(|i| store.get(*i).and_then(riviera_core::ramps::ramp_recipe).is_some()).collect()
    }

    /// Modify | Ramps › Edit Path: the Ramp tool draws the selected ramp's centreline again, in a
    /// plan of its base level; Enter replaces it.
    pub(crate) fn edit_ramp_path(&mut self) {
        let frame = self.frame;
        let Some(id) = self.selected_ramps().first().copied() else {
            return frame.set_status_text("Select a ramp to draw its path again", 0);
        };
        if !self.ensure_plan(false) {
            return;
        }
        let Some(store) = self.store_now() else { return };
        let Some(r) = store.get(id).and_then(riviera_core::ramps::ramp_recipe) else { return };
        let level = riviera_core::GlobalId::parse(&r.extent.base_level).and_then(|g| store.find_by_guid(&g));
        let Some(level) = level.or_else(|| self.active_view().and_then(|v| v.plan.as_ref()).map(|p| p.level)) else { return };
        let height = r.extent.z(&store).map_or(0.6, |(a, b)| b - a);
        self.end_tool(&frame);
        self.end_insert_tool();
        self.end_modify();
        self.ramp_tool = Some(RampTool { level, points: Vec::new(), cursor: None, height, width: r.width, railings: false, edit: Some(id) });
        // The path as it is, for reference until the first click.
        self.chain_lines(&r.path, None);
        self.properties_stale = true;
        self.ramp_prompt();
    }

    /// Modify | Ramps › Flip Direction: the top end becomes the bottom.
    pub(crate) fn flip_ramps(&mut self) {
        let frame = self.frame;
        let ids = self.selected_ramps();
        if ids.is_empty() {
            return frame.set_status_text("Select the ramps to flip", 0);
        }
        let Some(store) = self.store_now() else { return };
        for id in ids {
            let Some(mut path) = store.get(id).and_then(riviera_core::ramps::ramp_recipe).map(|r| r.path) else { continue };
            path.reverse();
            if let Err(e) = self.run("ramp.set", json!({ "ramp_ids": [id], "path": path })) {
                return error(&frame, &format!("The ramp was not flipped:\n{e}"));
            }
        }
        self.properties_stale = true;
        self.refresh_scene(&frame);
    }

    /// Modify | Ramps › Railings: railings on the selected ramps' sides that have none.
    pub(crate) fn rail_ramps(&mut self) {
        let frame = self.frame;
        let ids = self.selected_ramps();
        if ids.is_empty() {
            return frame.set_status_text("Select the ramps to put railings on", 0);
        }
        match self.run("ramp.add_railings", json!({ "ramp_ids": ids })) {
            Ok(v) => {
                self.refresh_scene(&frame);
                let n = v.as_array().map_or(0, Vec::len);
                frame.set_status_text(&if n == 0 { "Every side already has a railing".to_owned() } else { format!("{n} railings placed") }, 0);
            }
            Err(e) => error(&frame, &format!("No railings were placed:\n{e}")),
        }
    }

    pub(crate) fn end_ramp_tool(&mut self) -> bool {
        if self.ramp_tool.take().is_none() {
            return false;
        }
        if let Some(v) = self.viewport() {
            v.set_overlay_lines(&[]);
        }
        self.properties_stale = true;
        true
    }

    /// The run the default type's slope needs for the tool's height.
    fn ramp_needed(&self) -> f64 {
        let slope = self
            .store_now()
            .and_then(|s| {
                s.iter()
                    .filter_map(riviera_core::ramps::ramp_type)
                    .next()
                    .map(|t| t.max_slope)
            })
            .unwrap_or(12.0);
        self.ramp_tool.as_ref().map_or(0.0, |t| t.height * slope)
    }

    fn ramp_prompt(&mut self) {
        let Some(t) = self.ramp_tool.clone() else {
            return;
        };
        let mut drawn: f64 = t.points.windows(2).map(|w| len(w[0], w[1])).sum();
        if let (Some(c), Some(last)) = (t.cursor, t.points.last()) {
            drawn += len(*last, c);
        }
        let need = self.ramp_needed();
        let (n, d) = (self.format_length(need), self.format_length(drawn));
        let left = self.format_length((need - drawn).max(0.0));
        let text = if t.points.is_empty() && t.edit.is_some() {
            format!("Edit Path: click the bottom of the ramp's new centreline (it needs {n} of run); Esc keeps the path as it is")
        } else if t.points.is_empty() {
            format!("Ramp: click the bottom of its centreline (it needs {n} of run)")
        } else {
            format!(
                "Ramp: click the next point; Enter makes it, Esc drops the last point · drawn {d} of {n} ({left} to go)"
            )
        };
        self.frame.set_status_text(&text, 0);
    }

    pub(crate) fn ramp_motion(&mut self, x: i32, y: i32) {
        let from = self
            .ramp_tool
            .as_ref()
            .and_then(|t| t.points.last().copied());
        let p = self.chain_point(x, y, from);
        let Some(t) = &mut self.ramp_tool else { return };
        t.cursor = p;
        let (points, cursor) = (t.points.clone(), t.cursor);
        self.chain_lines(&points, cursor);
        self.ramp_prompt();
    }

    pub(crate) fn ramp_click(&mut self, x: i32, y: i32) {
        self.ramp_motion(x, y);
        if let Some(t) = &mut self.ramp_tool
            && let Some(c) = t.cursor
            && t.points.last().is_none_or(|l| len(*l, c) > 0.05)
        {
            t.points.push(c);
        }
        self.ramp_prompt();
    }

    /// Enter makes the ramp; Esc drops the last point, then ends the tool.
    pub(crate) fn ramp_key(&mut self, key: i32) -> bool {
        let Some(t) = self.ramp_tool.clone() else {
            return false;
        };
        let frame = self.frame;
        match key {
            WXK_RETURN | WXK_NUMPAD_ENTER => {
                if t.points.len() < 2 {
                    frame.set_status_text("Ramp: click two points or more first", 0);
                    return true;
                }
                if let Some(id) = t.edit {
                    match self.app.execute("ramp.set", json!({ "ramp_ids": [id], "path": t.points }), &super::user()) {
                        Ok(out) => {
                            self.end_ramp_tool();
                            self.refresh_scene(&frame);
                            self.select_ids(&[id], super::select::Combine::Replace);
                            let warning = out.warnings.first().map(|w| w.message.clone());
                            frame.set_status_text(&warning.unwrap_or_else(|| "Ramp path changed".into()), 0);
                        }
                        Err(e) => error(&frame, &format!("The path was not changed:\n{}", e.message)),
                    }
                    return true;
                }
                let args = json!({
                    "path": t.points, "width": t.width, "base_level": t.level,
                    "top_constraint": { "unconnected": t.height }, "railings": t.railings,
                });
                match self.app.execute("ramp.create", args, &super::user()) {
                    Ok(out) => {
                        if let Some(t) = &mut self.ramp_tool {
                            t.points.clear();
                        }
                        self.refresh_scene(&frame);
                        let warning = out.warnings.first().map(|w| w.message.clone());
                        frame.set_status_text(
                            &warning.unwrap_or_else(|| "Ramp made; draw another, or Esc".into()),
                            0,
                        );
                    }
                    Err(e) => error(&frame, &format!("The ramp was not made:\n{}", e.message)),
                }
                true
            }
            WXK_ESCAPE => {
                let empty = t.points.is_empty();
                if empty {
                    self.end_ramp_tool();
                    frame.set_status_text("Ready", 0);
                } else if let Some(t) = &mut self.ramp_tool {
                    t.points.pop();
                    let (points, cursor) = (t.points.clone(), t.cursor);
                    self.chain_lines(&points, cursor);
                    self.ramp_prompt();
                }
                true
            }
            _ => false,
        }
    }

    /// The palette while the Ramp tool runs: the height it climbs and its width.
    pub(crate) fn ramp_tool_properties(&mut self, grid: &PropertyGrid) -> bool {
        let Some(t) = self.ramp_tool.clone() else {
            return false;
        };
        grid.append(Property::category("Ramp tool", "c_ramptool"));
        let (h, w) = (self.format_length(t.height), self.format_length(t.width));
        grid.append(Property::string("Height", "rpt_height", h));
        grid.set_help_string(
            "rpt_height",
            "How high the ramp climbs from the plan's level",
        );
        grid.append(Property::string("Width", "rpt_width", w));
        if t.edit.is_none() {
            grid.append(Property::boolean("Railings", "rpt_railings", t.railings));
            grid.set_help_string("rpt_railings", "Railings on both sides, following the flights and landings (ADA 405.8 asks for them above a 150 mm rise)");
        }
        true
    }

    /// The rows of a selected ramp; `false` if `id` is not one.
    pub(crate) fn ramp_rows(&mut self, grid: &PropertyGrid, id: ElementId) -> bool {
        let Some(store) = self.store_now() else {
            return false;
        };
        let Some(e) = store.get(id) else { return false };
        let Some(r) = riviera_core::ramps::ramp_recipe(e) else {
            return false;
        };
        let z = r.extent.z(&store).unwrap_or((0.0, 0.0));
        grid.append(Property::category("Constraints", "c_ramp"));
        self.extent_rows(grid, "rp_", &r.extent, z);
        grid.append(Property::category("Dimensions", "c_ramp_dims"));
        let w = self.format_length(r.width);
        grid.append(Property::string("Width", "rp_width", w));
        let path = r.path.iter().map(|p| format!("{}, {}", self.format_length(p[0]), self.format_length(p[1]))).collect::<Vec<_>>().join("; ");
        grid.append(Property::string("Path (x, y; …)", "rp_path", path));
        grid.set_help_string("rp_path", "The centreline's points from the bottom up, x, y each, separated by semicolons; or Modify | Ramps › Edit Path to draw it");
        if let Ok(g) = riviera_core::ramps::regenerate(&store, e) {
            let slope = format!("1:{:.1}", 1.0 / g.slope.max(1e-9));
            grid.append(Property::string("Slope", "rp_slope", slope));
            grid.set_property_read_only("rp_slope", true);
            let need = self.format_length(g.required_length);
            grid.append(Property::string("Run Needed", "rp_needed", need));
            grid.set_property_read_only("rp_needed", true);
            grid.append(Property::category("Type", "c_ramp_type"));
            let current = e
                .param("@type")
                .and_then(serde_json::Value::as_str)
                .and_then(riviera_core::GlobalId::parse)
                .and_then(|g| store.find_by_guid(&g));
            self.type_picker_row(grid, e.id(), current);
            self.type_name_row(grid, current);
            let rows = [
                (
                    "Maximum Slope 1: (type)",
                    "rpt_max_slope",
                    format!("{}", g.ty.max_slope),
                ),
                (
                    "Maximum Incline Length (type)",
                    "rpt_max_incline",
                    self.format_length(g.ty.max_incline_length),
                ),
                (
                    "Thickness (type)",
                    "rpt_thickness",
                    self.format_length(g.ty.thickness),
                ),
            ];
            for (label, name, value) in rows {
                grid.append(Property::string(label, name, value));
            }
            let solid = g.ty.shape == riviera_core::ramps::RampShape::Solid;
            grid.append(Property::enumeration("Shape (type)", "rpt_shape", vec![("Thick".to_owned(), 0), ("Solid".to_owned(), 1)], solid as i32));
            grid.set_help_string("rpt_shape", "Thick: a slab of the type's thickness under the slope; Solid: filled down to the base");
            self.type_rows = e
                .param("@type")
                .and_then(serde_json::Value::as_str)
                .and_then(riviera_core::GlobalId::parse)
                .and_then(|g| store.find_by_guid(&g));
        }
        true
    }

    /// An edit of a ramp or Ramp tool row; `false` if `property` is not one.
    pub(crate) fn ramp_edit(&mut self, property: &str, text: Option<String>) -> bool {
        if !property.starts_with("rp_") && !property.starts_with("rpt_") {
            return false;
        }
        let frame = self.frame;
        let text = text.unwrap_or_default();
        let parse = |g: &mut Gui, t: &str| -> Option<f64> {
            g.run("units.parse", json!({ "text": t, "data_type": "length" }))
                .ok()
                .and_then(|v| v.as_f64())
        };
        if property == "rpt_railings" {
            if let Some(t) = &mut self.ramp_tool {
                t.railings = text == "true" || text == "1";
            }
            return true;
        }
        if property == "rpt_height" || property == "rpt_width" {
            if let Some(v) = parse(self, &text).filter(|v| *v > 0.0)
                && let Some(t) = &mut self.ramp_tool
            {
                if property == "rpt_height" {
                    t.height = v;
                } else {
                    t.width = v;
                }
            }
            self.properties_stale = true;
            self.ramp_prompt();
            return true;
        }
        let ids: Vec<ElementId> = {
            let ids = self.selection();
            let store = self.store_now();
            ids.into_iter()
                .filter(|i| {
                    store
                        .as_ref()
                        .and_then(|s| s.get(*i))
                        .and_then(riviera_core::ramps::ramp_recipe)
                        .is_some()
                })
                .collect()
        };
        let Some(store) = self.store_now() else {
            return true;
        };
        let level = |name: &str| {
            riviera_core::levels::list(&store)
                .into_iter()
                .find(|l| l.name == name)
                .map(|l| l.id)
        };
        let set = |g: &mut Gui, mut a: serde_json::Value| {
            a["ramp_ids"] = json!(ids);
            g.run("ramp.set", a).map(|_| ())
        };
        let type_param = |g: &mut Gui, param: &str, value: serde_json::Value| {
            let t = g.type_rows?;
            Some(
                g.run(
                    "type.set_param",
                    json!({ "type_id": t, "param": param, "value": value }),
                )
                .map(|_| ()),
            )
        };
        let result: Option<Result<(), String>> = match property {
            "rp_base" => level(&text).map(|l| set(self, json!({ "base_level": l }))),
            "rp_base_off" => Some(set(self, json!({ "base_offset": text }))),
            "rp_top" if text == "Unconnected" => Some(set(
                self,
                json!({ "top_constraint": { "unconnected": 0.6 } }),
            )),
            "rp_top" => {
                level(&text).map(|l| set(self, json!({ "top_constraint": { "level": l } })))
            }
            "rp_height" => Some(set(
                self,
                json!({ "top_constraint": { "unconnected": text } }),
            )),
            "rp_top_off" => Some(set(self, json!({ "top_offset": text }))),
            "rp_width" => Some(set(self, json!({ "width": text }))),
            "rp_path" => {
                let pts: Option<Vec<[f64; 2]>> = text
                    .split(';')
                    .filter(|p| !p.trim().is_empty())
                    .map(|p| {
                        let v: Vec<f64> = p.split(',').filter_map(|c| self.parse_length_text(c.trim())).collect();
                        (v.len() == 2).then(|| [v[0], v[1]])
                    })
                    .collect();
                match pts.filter(|p| p.len() >= 2) {
                    Some(p) => Some(set(self, json!({ "path": p }))),
                    None => Some(Err("Type two points or more as x, y; x, y".to_owned())),
                }
            }
            "rpt_shape" => type_param(self, "shape", json!(text.to_lowercase())),
            "rpt_max_slope" => type_param(self, "max_slope", json!(text)),
            "rpt_max_incline" => type_param(self, "max_incline_length", json!(text)),
            "rpt_thickness" => type_param(self, "thickness", json!(text)),
            _ => None,
        };
        if let Some(Err(e)) = result {
            error(&frame, &format!("The change was not made:\n{e}"));
        }
        self.properties_stale = true;
        self.refresh_scene(&frame);
        true
    }

    // ---- Measure ----

    /// Modify › Measure.
    pub(crate) fn start_measure_tool(&mut self) {
        let frame = self.frame;
        if !self.in_plan() {
            return frame.set_status_text("Measure works in plan views", 0);
        }
        self.end_tool(&frame);
        self.end_insert_tool();
        self.end_modify();
        self.measure_tool = Some(MeasureTool::default());
        frame.set_status_text("Measure: click the first point", 0);
    }

    pub(crate) fn end_measure_tool(&mut self) -> bool {
        if self.measure_tool.take().is_none() {
            return false;
        }
        if let Some(v) = self.viewport() {
            v.set_overlay_lines(&[]);
        }
        true
    }

    fn measure_prompt(&mut self) {
        let Some(t) = self.measure_tool.clone() else {
            return;
        };
        let mut pts = t.points.clone();
        if let (Some(c), false) = (t.cursor, t.points.is_empty()) {
            pts.push(c);
        }
        if pts.len() < 2 {
            return self
                .frame
                .set_status_text("Measure: click the first point", 0);
        }
        let (a, b) = (pts[pts.len() - 2], pts[pts.len() - 1]);
        let seg = len(a, b);
        let total: f64 = pts.windows(2).map(|w| len(w[0], w[1])).sum();
        let angle = (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees();
        let (s, tt) = (self.format_length(seg), self.format_length(total));
        let (dx, dy) = (
            self.format_length((b[0] - a[0]).abs()),
            self.format_length((b[1] - a[1]).abs()),
        );
        self.frame.set_status_text(
            &format!(
                "Measure: {s} at {angle:.1}° (Δx {dx}, Δy {dy}) · chain total {tt} · click to add a point, Esc clears"
            ),
            0,
        );
    }

    pub(crate) fn measure_motion(&mut self, x: i32, y: i32) {
        let from = self
            .measure_tool
            .as_ref()
            .and_then(|t| t.points.last().copied());
        let p = self.chain_point(x, y, from);
        let Some(t) = &mut self.measure_tool else {
            return;
        };
        t.cursor = p;
        let (points, cursor) = (t.points.clone(), t.cursor);
        self.chain_lines(&points, cursor);
        self.measure_prompt();
    }

    pub(crate) fn measure_click(&mut self, x: i32, y: i32) {
        self.measure_motion(x, y);
        if let Some(t) = &mut self.measure_tool
            && let Some(c) = t.cursor
        {
            t.points.push(c);
        }
        self.measure_prompt();
    }

    /// Esc clears the chain, then ends the tool.
    pub(crate) fn measure_escape(&mut self) -> bool {
        let Some(t) = &mut self.measure_tool else {
            return false;
        };
        if t.points.is_empty() {
            self.end_measure_tool();
            self.frame.set_status_text("Ready", 0);
        } else {
            t.points.clear();
            if let Some(v) = self.viewport() {
                v.set_overlay_lines(&[]);
            }
            self.frame
                .set_status_text("Measure: click the first point", 0);
        }
        true
    }
}
