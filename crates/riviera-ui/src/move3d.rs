// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Move and Copy outside plans (ARC-MOD-001, ARC-MOD-002): in a 3D view, a section or an
//! elevation, the selection gets a handle per axis at its centre (x red, y green, z blue).
//! Click a handle, move the mouse along it (the selection's box follows) and click to place, or
//! type a distance and press Enter. The tool stays on for further moves; Esc drops a handle in
//! use, then ends it.
//!
//! What a selection may do decides its handles: doors, windows and wall openings move along their
//! wall and up it only (the core keeps them in their wall), floor openings and shafts across
//! the floor only, and rooms and grids, which keep to their levels, have no height handle.

use riviera_core::ElementId;
use riviera_render::RenderBackend;
use riviera_render::plan::Lines;
use serde_json::{Value, json};
use wxdragon::prelude::*;

use super::Gui;

const PREVIEW: [f32; 4] = [0.05, 0.35, 0.95, 1.0];
/// A handle's length on screen, and how near the cursor must be to take it (pixels).
const HANDLE_PX: f32 = 90.0;
const GRAB_PX: f32 = 9.0;

/// One handle: its label and direction (a unit vector) and colour.
#[derive(Debug, Clone)]
pub struct Axis {
    label: &'static str,
    dir: [f64; 3],
    color: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct Move3d {
    copy: bool,
    targets: Vec<ElementId>,
    /// The selection's box (min, max), and its centre, where the handles stand.
    lo: [f64; 3],
    hi: [f64; 3],
    axes: Vec<Axis>,
    hover: Option<usize>,
    /// The handle in use, where along it the grab was, and how far the cursor has taken it.
    active: Option<usize>,
    start: f64,
    delta: f64,
    typed: String,
}

impl Move3d {
    fn center(&self) -> [f64; 3] {
        [(self.lo[0] + self.hi[0]) / 2.0, (self.lo[1] + self.hi[1]) / 2.0, (self.lo[2] + self.hi[2]) / 2.0]
    }
}

fn add(a: [f64; 3], b: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

fn v3(p: [f64; 3]) -> glam::Vec3 {
    glam::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)
}

fn f3(p: [f64; 3]) -> [f32; 3] {
    [p[0] as f32, p[1] as f32, p[2] as f32]
}

/// The distance in pixels from `p` to the segment `a`–`b`.
fn to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = (dx * dx + dy * dy).max(1e-6);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    (p.0 - a.0 - dx * t).hypot(p.1 - a.1 - dy * t)
}

impl Gui {
    /// Move or Copy started outside a plan.
    pub(crate) fn start_move3d(&mut self, copy: bool) {
        let frame = self.frame;
        let what = if copy { "Copy" } else { "Move" };
        let targets = self.selection();
        if targets.is_empty() {
            return frame.set_status_text(&format!("Select the elements first, then start {what}"), 0);
        }
        let Some((lo, hi)) = self.box_of(&targets) else {
            return frame.set_status_text(&format!("{what}: the selection is not shown in this view"), 0);
        };
        let axes = self.axes_for(&targets);
        if axes.is_empty() {
            return frame.set_status_text(&format!("{what}: the selection can't be moved here"), 0);
        }
        self.end_tool(&frame);
        self.move3d = Some(Move3d { copy, targets, lo, hi, axes, hover: None, active: None, start: 0.0, delta: 0.0, typed: String::new() });
        self.move3d_draw();
        self.move3d_prompt();
        self.refresh_active();
    }

    pub(crate) fn end_move3d(&mut self) -> bool {
        if self.move3d.take().is_none() {
            return false;
        }
        if let Some(v) = self.viewport() {
            v.set_overlay_lines(&[]);
        }
        true
    }

    /// The box round what the view draws of `ids`.
    fn box_of(&self, ids: &[ElementId]) -> Option<([f64; 3], [f64; 3])> {
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for m in self.scene().meshes.iter().filter(|m| m.element.is_some_and(|e| ids.contains(&e))) {
            for p in &m.positions {
                for k in 0..3 {
                    lo[k] = lo[k].min(f64::from(p[k]));
                    hi[k] = hi[k].max(f64::from(p[k]));
                }
            }
        }
        lo[0].is_finite().then_some((lo, hi))
    }

    /// The handles a selection gets.
    fn axes_for(&self, ids: &[ElementId]) -> Vec<Axis> {
        let x = Axis { label: "x", dir: [1.0, 0.0, 0.0], color: [0.85, 0.15, 0.15, 1.0] };
        let y = Axis { label: "y", dir: [0.0, 1.0, 0.0], color: [0.15, 0.65, 0.15, 1.0] };
        let z = Axis { label: "height", dir: [0.0, 0.0, 1.0], color: [0.15, 0.35, 0.95, 1.0] };
        let Some(store) = self.active_view().and_then(|v| self.app.document(v.doc)).map(|d| d.doc.store().clone()) else { return Vec::new() };
        let elements: Vec<&riviera_core::Element> = ids.iter().filter_map(|id| store.get(*id)).collect();
        // Doors, windows and wall openings: along their wall and up it.
        let host = |e: &riviera_core::Element| riviera_core::inserts::insert_recipe(e).map(|r| r.host).or_else(|| riviera_core::wall_openings::wall_opening_recipe(e).map(|r| r.host));
        if !elements.is_empty() && elements.iter().all(|e| host(e).is_some()) {
            let wall = host(elements[0]).and_then(|g| riviera_core::GlobalId::parse(&g)).and_then(|g| store.find_by_guid(&g)).and_then(|w| store.get(w));
            let Some(g) = wall.and_then(|w| riviera_core::walls::regenerate(&store, w).ok()) else { return vec![z] };
            let u = g.direction();
            return vec![Axis { label: "along the wall", dir: [u[0], u[1], 0.0], color: [0.85, 0.15, 0.15, 1.0] }, z];
        }
        // Floor openings and shafts: across the floor.
        if elements.iter().all(|e| riviera_core::slab_openings::shaft_recipe(e).is_some() || riviera_core::slab_openings::opening_recipe(e).is_some()) {
            return vec![x, y];
        }
        // Rooms and grids keep to their levels.
        let flat = elements.iter().any(|e| riviera_core::rooms::room_recipe(e).is_some() || riviera_core::grids::grid_data(e).is_some() || e.class == "IfcBuildingStorey");
        if flat { vec![x, y] } else { vec![x, y, z] }
    }

    /// A handle's length in metres, so it stands about [`HANDLE_PX`] long on screen.
    fn handle_length(&self, c: [f64; 3], dir: [f64; 3]) -> f64 {
        let Some(v) = self.active_view() else { return 1.0 };
        let size = v.panel.get_client_size();
        let (w, h) = (size.width as f32, size.height as f32);
        let px = match (v.camera.project(v3(c), w, h), v.camera.project(v3(add(c, dir, 1.0)), w, h)) {
            (Some(a), Some(b)) => (b.0 - a.0).hypot(b.1 - a.1),
            _ => 0.0,
        };
        if px > 1e-3 { f64::from(HANDLE_PX / px).clamp(0.05, 1e4) } else { 1.0 }
    }

    /// Where along `axis` (from the centre) the cursor at (`x`, `y`) is: the nearest point of the
    /// axis to the cursor's ray.
    fn along_axis(&self, x: i32, y: i32, c: [f64; 3], a: [f64; 3]) -> Option<f64> {
        let v = self.active_view()?;
        let size = v.panel.get_client_size();
        let (o, d) = v.camera.ray(x as f32, y as f32, size.width as f32, size.height as f32);
        let (o, d) = ([f64::from(o.x), f64::from(o.y), f64::from(o.z)], [f64::from(d.x), f64::from(d.y), f64::from(d.z)]);
        let dot = |p: [f64; 3], q: [f64; 3]| p[0] * q[0] + p[1] * q[1] + p[2] * q[2];
        let w0 = [o[0] - c[0], o[1] - c[1], o[2] - c[2]];
        let b = dot(d, a);
        let den = 1.0 - b * b;
        // Looking straight down the axis: no position along it.
        (den > 1e-4).then(|| (dot(a, w0) - b * dot(d, w0)) / den)
    }

    /// The handle under the cursor.
    fn handle_at(&self, x: i32, y: i32) -> Option<usize> {
        let t = self.move3d.as_ref()?;
        let v = self.active_view()?;
        let size = v.panel.get_client_size();
        let (w, h) = (size.width as f32, size.height as f32);
        let c = t.center();
        t.axes
            .iter()
            .enumerate()
            .filter_map(|(k, a)| {
                let tip = add(c, a.dir, self.handle_length(c, a.dir));
                let (p, q) = (v.camera.project(v3(c), w, h)?, v.camera.project(v3(tip), w, h)?);
                Some((to_segment((x as f32, y as f32), p, q), k))
            })
            .filter(|(d, _)| *d <= GRAB_PX)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, k)| k)
    }

    fn move3d_draw(&mut self) {
        let Some(t) = self.move3d.clone() else { return };
        let shift = t.active.map_or([0.0; 3], |k| add([0.0; 3], t.axes[k].dir, t.delta));
        let c = add(t.center(), shift, 1.0);
        let mut lines = Vec::new();
        for (k, a) in t.axes.iter().enumerate() {
            let len = self.handle_length(c, a.dir);
            let tip = add(c, a.dir, len);
            // An arrowhead across the handle.
            let side = if a.dir[2].abs() < 0.9 { [-a.dir[1], a.dir[0], 0.0] } else { [1.0, 0.0, 0.0] };
            let back = add(tip, a.dir, -0.18 * len);
            let segments = vec![[f3(c), f3(tip)], [f3(tip), f3(add(back, side, 0.08 * len))], [f3(tip), f3(add(back, side, -0.08 * len))]];
            let width = if t.active == Some(k) || t.hover == Some(k) { 4.0 } else { 2.5 };
            lines.push(Lines { color: a.color, width, segments });
            if t.active == Some(k) {
                // The line it slides along, thin, a few box sizes either way.
                let size = (0..3).map(|i| t.hi[i] - t.lo[i]).fold(1.0, f64::max);
                let reach = 3.0 * size + t.delta.abs();
                lines.push(Lines { color: a.color, width: 1.0, segments: vec![[f3(add(c, a.dir, -reach)), f3(add(c, a.dir, reach))]] });
            }
        }
        if t.active.is_some() {
            // The selection's box where it would go.
            let (lo, hi) = (add(t.lo, shift, 1.0), add(t.hi, shift, 1.0));
            let p = |i: usize| f3([if i & 1 == 0 { lo[0] } else { hi[0] }, if i & 2 == 0 { lo[1] } else { hi[1] }, if i & 4 == 0 { lo[2] } else { hi[2] }]);
            let edges = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];
            lines.push(Lines { color: PREVIEW, width: 1.5, segments: edges.iter().map(|(a, b)| [p(*a), p(*b)]).collect() });
        }
        if let Some(v) = self.viewport() {
            v.set_overlay_lines(&lines);
        }
    }

    fn move3d_prompt(&mut self) {
        let Some(t) = self.move3d.clone() else { return };
        let what = if t.copy { "Copy" } else { "Move" };
        let text = match t.active {
            Some(k) => {
                let d = if t.typed.is_empty() { self.format_length(t.delta) } else { t.typed.clone() };
                format!("{d} {} · {what}: click to place, or type a distance and Enter; Esc lets go", t.axes[k].label)
            }
            None => {
                let names: Vec<&str> = t.axes.iter().map(|a| a.label).collect();
                format!("{what}: click a handle ({}) and move along it; Esc ends", names.join(", "))
            }
        };
        self.frame.set_status_text(&text, 0);
    }

    pub(crate) fn move3d_motion(&mut self, x: i32, y: i32) {
        let Some(t) = self.move3d.clone() else { return };
        match t.active {
            Some(k) if t.typed.is_empty() => {
                if let Some(s) = self.along_axis(x, y, t.center(), t.axes[k].dir)
                    && let Some(m) = &mut self.move3d
                {
                    m.delta = ((s - t.start) * 1000.0).round() / 1000.0;
                }
            }
            Some(_) => {}
            None => {
                let hover = self.handle_at(x, y);
                if let Some(m) = &mut self.move3d {
                    m.hover = hover;
                }
            }
        }
        self.move3d_draw();
        self.move3d_prompt();
    }

    pub(crate) fn move3d_click(&mut self, x: i32, y: i32) {
        let Some(t) = self.move3d.clone() else { return };
        match t.active {
            None => {
                let Some(k) = self.handle_at(x, y) else {
                    return self.frame.set_status_text("Click one of the handles at the selection", 0);
                };
                let start = self.along_axis(x, y, t.center(), t.axes[k].dir).unwrap_or(0.0);
                if let Some(m) = &mut self.move3d {
                    m.active = Some(k);
                    m.start = start;
                    m.delta = 0.0;
                }
                self.move3d_draw();
                self.move3d_prompt();
            }
            Some(_) => self.move3d_commit(),
        }
    }

    /// Moves (or copies) the selection by the handle's distance, and stays on for the next move.
    fn move3d_commit(&mut self) {
        let frame = self.frame;
        let Some(t) = self.move3d.clone() else { return };
        let Some(k) = t.active else { return };
        let distance = if t.typed.is_empty() {
            t.delta
        } else {
            match self.run("units.parse", json!({ "text": t.typed, "data_type": "length" })) {
                Ok(v) => v.as_f64().unwrap_or_default(),
                Err(e) => return frame.set_status_text(&e, 0),
            }
        };
        if distance.abs() < 1e-9 {
            return self.move3d_release();
        }
        let v = add([0.0; 3], t.axes[k].dir, distance);
        let result = if t.copy {
            self.run("element.copy", json!({ "element_ids": t.targets, "vectors": [v] }))
        } else {
            self.run("element.move", json!({ "element_ids": t.targets, "vector": v }))
        };
        match result {
            Ok(r) => {
                self.refresh_scene(&frame);
                // The moved elements (or the copies) stay selected for the next move.
                let ids: Vec<ElementId> = if t.copy { r[0].as_array() } else { r.as_array() }.into_iter().flatten().filter_map(Value::as_u64).map(ElementId).collect();
                let ids = if ids.is_empty() { t.targets.clone() } else { ids };
                self.select_ids(&ids, super::select::Combine::Replace);
                let bounds = self.box_of(&ids);
                if let Some(m) = &mut self.move3d {
                    m.targets = ids;
                    if let Some((lo, hi)) = bounds {
                        m.lo = lo;
                        m.hi = hi;
                    }
                }
                self.move3d_release();
            }
            Err(e) => super::error(&frame, &format!("Nothing was moved:\n{e}")),
        }
    }

    /// Lets go of the handle in use.
    fn move3d_release(&mut self) {
        if let Some(m) = &mut self.move3d {
            m.active = None;
            m.delta = 0.0;
            m.typed.clear();
        }
        self.move3d_draw();
        self.move3d_prompt();
    }

    /// Keys: a typed distance and Enter, Backspace, and Esc (lets go of the handle, then ends).
    pub(crate) fn move3d_key(&mut self, key: i32) -> bool {
        let Some(t) = self.move3d.clone() else { return false };
        match key {
            WXK_ESCAPE => {
                if t.active.is_some() {
                    self.move3d_release();
                } else {
                    self.end_move3d();
                    self.frame.set_status_text("Ready", 0);
                }
            }
            WXK_RETURN | WXK_NUMPAD_ENTER if t.active.is_some() => self.move3d_commit(),
            WXK_BACK if !t.typed.is_empty() => {
                if let Some(m) = &mut self.move3d {
                    m.typed.pop();
                }
                self.move3d_prompt();
            }
            _ => {
                let Some(c) = u32::try_from(key).ok().and_then(char::from_u32) else { return false };
                let starts = c.is_ascii_digit() || c == '.' || c == '-';
                let continues = !t.typed.is_empty() && (c.is_ascii_graphic() || c == ' ');
                if t.active.is_none() || !(starts || continues) {
                    return false;
                }
                if let Some(m) = &mut self.move3d {
                    m.typed.push(c);
                }
                self.move3d_prompt();
            }
        }
        true
    }
}
