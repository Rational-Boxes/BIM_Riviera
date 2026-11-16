// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Move and Copy outside plans (ARC-MOD-004, ARC-MOD-005): in a 3D view, a section or an
//! elevation the selection turns translucent and gets a handle per axis at its centre: a 3D
//! arrow with a cone for its head (x red, y green, z blue; yellow when the cursor is on it).
//! Press on a handle and drag: the selection's copy follows along that axis, solid, and the
//! selection moves there when the button is let go. Beside the handles a box of fields, one per
//! axis, shows the distance dragged and takes exact values (Enter moves by them). The tool
//! stays on for further moves; Esc ends it.
//!
//! What a selection may do decides its handles: doors, windows and wall openings move in the
//! plane of their wall (along it and up it; the core keeps them in their wall), floor openings
//! and shafts in the plane of the floor, and rooms and grids, which keep to their levels, have no
//! height handle.

use riviera_core::ElementId;
use riviera_render::{SceneMesh, Visibility};
use serde_json::{Value, json};
use wxdragon::prelude::*;

use super::Gui;

/// A handle's length on screen, its shaft's and cone's radii and the cone's length (pixels), and
/// how near the cursor must be to take it.
const HANDLE_PX: f64 = 95.0;
const SHAFT_PX: f64 = 2.5;
const CONE_R_PX: f64 = 8.0;
const CONE_L_PX: f64 = 24.0;
const GRAB_PX: f32 = 10.0;
/// The selection's opacity while it is being moved.
const GHOST: f32 = 0.3;
const HOVER: [f32; 4] = [1.0, 0.82, 0.1, 1.0];

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
    /// The selection's box (min, max); the handles stand at its centre.
    lo: [f64; 3],
    hi: [f64; 3],
    axes: Vec<Axis>,
    hover: Option<usize>,
    /// The handle being dragged, where along it the grab was, and how far it has gone.
    drag: Option<usize>,
    start: f64,
    delta: f64,
}

/// The fields beside the handles: a box over the view with one field per axis.
pub struct MoveFields {
    panel: Panel,
    fields: Vec<TextCtrl>,
}

impl Move3d {
    fn center(&self) -> [f64; 3] {
        [(self.lo[0] + self.hi[0]) / 2.0, (self.lo[1] + self.hi[1]) / 2.0, (self.lo[2] + self.hi[2]) / 2.0]
    }

    /// How far the drag has taken the selection.
    fn shift(&self) -> [f64; 3] {
        self.drag.map_or([0.0; 3], |k| add([0.0; 3], self.axes[k].dir, self.delta))
    }
}

fn add(a: [f64; 3], b: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn v3(p: [f64; 3]) -> glam::Vec3 {
    glam::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)
}

/// The distance in pixels from `p` to the segment `a`–`b`.
fn to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = (dx * dx + dy * dy).max(1e-6);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    (p.0 - a.0 - dx * t).hypot(p.1 - a.1 - dy * t)
}

/// An arrow from `from` along `dir`: a shaft of radius `r` and length `shaft`, then a cone of
/// radius `cr` and length `cl`.
fn arrow(from: [f64; 3], dir: [f64; 3], r: f64, shaft: f64, cr: f64, cl: f64, color: [f32; 4]) -> SceneMesh {
    const N: usize = 16;
    let side = if dir[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let u = unit(cross(dir, side));
    let w = cross(dir, u);
    let ring = |c: [f64; 3], rad: f64| -> Vec<[f32; 3]> {
        (0..N)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / N as f64;
                let p = add(add(c, u, rad * a.cos()), w, rad * a.sin());
                [p[0] as f32, p[1] as f32, p[2] as f32]
            })
            .collect()
    };
    let base = ring(from, r);
    let top = add(from, dir, shaft);
    let neck = ring(top, r);
    let rim = ring(top, cr);
    let tip = add(top, dir, cl);
    let mut positions = Vec::new();
    positions.extend(&base); // 0..N
    positions.extend(&neck); // N..2N
    positions.extend(&rim); // 2N..3N
    positions.push([from[0] as f32, from[1] as f32, from[2] as f32]); // 3N: base centre
    positions.push([top[0] as f32, top[1] as f32, top[2] as f32]); // 3N+1: rim centre
    positions.push([tip[0] as f32, tip[1] as f32, tip[2] as f32]); // 3N+2: tip
    let n = N as u32;
    let (bc, rc, tp) = (3 * n, 3 * n + 1, 3 * n + 2);
    let mut indices = Vec::new();
    for k in 0..n {
        let j = (k + 1) % n;
        // The shaft's side, its base, the cone's base and its side.
        indices.extend([k, j, n + j, k, n + j, n + k]);
        indices.extend([bc, j, k]);
        indices.extend([rc, 2 * n + k, 2 * n + j]);
        indices.extend([2 * n + k, tp, 2 * n + j]);
    }
    SceneMesh { element: None, class: "Move handle".into(), positions, indices, color }
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
        self.move3d = Some(Move3d { copy, targets, lo, hi, axes, hover: None, drag: None, start: 0.0, delta: 0.0 });
        self.move3d_fields_open();
        self.move3d_redraw();
        self.move3d_prompt();
    }

    pub(crate) fn end_move3d(&mut self) -> bool {
        if self.move3d.take().is_none() {
            return false;
        }
        if let Some(f) = self.move3d_fields.take() {
            f.panel.destroy();
        }
        self.move3d_redraw();
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
        let x = Axis { label: "X", dir: [1.0, 0.0, 0.0], color: [0.86, 0.16, 0.16, 1.0] };
        let y = Axis { label: "Y", dir: [0.0, 1.0, 0.0], color: [0.16, 0.66, 0.16, 1.0] };
        let z = Axis { label: "Z", dir: [0.0, 0.0, 1.0], color: [0.16, 0.36, 0.95, 1.0] };
        let Some(store) = self.active_view().and_then(|v| self.app.document(v.doc)).map(|d| d.doc.store().clone()) else { return Vec::new() };
        let elements: Vec<&riviera_core::Element> = ids.iter().filter_map(|id| store.get(*id)).collect();
        // Doors, windows and wall openings: in the plane of their wall.
        let host = |e: &riviera_core::Element| riviera_core::inserts::insert_recipe(e).map(|r| r.host).or_else(|| riviera_core::wall_openings::wall_opening_recipe(e).map(|r| r.host));
        if !elements.is_empty() && elements.iter().all(|e| host(e).is_some()) {
            let up = Axis { label: "Up", ..z };
            let wall = host(elements[0]).and_then(|g| riviera_core::GlobalId::parse(&g)).and_then(|g| store.find_by_guid(&g)).and_then(|w| store.get(w));
            let Some(g) = wall.and_then(|w| riviera_core::walls::regenerate(&store, w).ok()) else { return vec![up] };
            let u = g.direction();
            return vec![Axis { label: "Along", dir: [u[0], u[1], 0.0], color: x.color }, up];
        }
        // Floor openings and shafts: in the plane of the floor.
        if elements.iter().all(|e| riviera_core::slab_openings::shaft_recipe(e).is_some() || riviera_core::slab_openings::opening_recipe(e).is_some()) {
            return vec![x, y];
        }
        // Rooms and grids keep to their levels.
        let flat = elements.iter().any(|e| riviera_core::rooms::room_recipe(e).is_some() || riviera_core::grids::grid_data(e).is_some() || e.class == "IfcBuildingStorey");
        if flat { vec![x, y] } else { vec![x, y, z] }
    }

    /// Metres per pixel at `p` in the active view.
    fn move3d_scale(&self, p: [f64; 3]) -> f64 {
        let Some(v) = self.active_view() else { return 0.01 };
        let size = v.panel.get_client_size();
        let (w, h) = (size.width as f32, size.height as f32);
        // Across the screen at `p`: two points a metre apart square to the view.
        let right = v.camera.ray(w / 2.0 + 1.0, h / 2.0, w, h).1 - v.camera.ray(w / 2.0, h / 2.0, w, h).1;
        let side = if right.length() > 1e-9 { right.normalize() } else { glam::Vec3::X };
        match (v.camera.project(v3(p), w, h), v.camera.project(v3(p) + side, w, h)) {
            (Some(a), Some(b)) => {
                let px = f64::from((b.0 - a.0).hypot(b.1 - a.1));
                if px > 1e-3 { 1.0 / px } else { 0.01 }
            }
            _ => 0.01,
        }
    }

    /// Where along `a` (from `c`) the cursor at (`x`, `y`) is: the nearest point of the axis to
    /// the cursor's ray.
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

    /// The handle under the cursor: the nearest on screen within reach of its shaft and cone.
    fn handle_at(&self, x: i32, y: i32) -> Option<usize> {
        let t = self.move3d.as_ref()?;
        let v = self.active_view()?;
        let size = v.panel.get_client_size();
        let (w, h) = (size.width as f32, size.height as f32);
        let c = t.center();
        let len = HANDLE_PX * self.move3d_scale(c);
        t.axes
            .iter()
            .enumerate()
            .filter_map(|(k, a)| {
                let (p, q) = (v.camera.project(v3(c), w, h)?, v.camera.project(v3(add(c, a.dir, len)), w, h)?);
                Some((to_segment((x as f32, y as f32), p, q), k))
            })
            .filter(|(d, _)| *d <= GRAB_PX)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, k)| k)
    }

    /// Adds the tool's look to the active view's visibility: the selection translucent, the
    /// handles, and while dragging, the selection where it would go.
    pub(crate) fn move3d_decorate(&self, index: usize, vis: &mut Visibility) {
        let Some(t) = &self.move3d else { return };
        if self.views.get(index).is_none_or(|v| Some(v.id) != self.active) {
            return;
        }
        for id in &t.targets {
            vis.translucent.insert(*id, GHOST);
        }
        let shift = t.shift();
        if t.drag.is_some() {
            for m in self.scene().meshes.iter().filter(|m| m.element.is_some_and(|e| t.targets.contains(&e))) {
                let mut c = m.clone();
                c.element = None;
                for p in &mut c.positions {
                    p[0] += shift[0] as f32;
                    p[1] += shift[1] as f32;
                    p[2] += shift[2] as f32;
                }
                vis.extra.push(c);
            }
        }
        let c = add(t.center(), shift, 1.0);
        let s = self.move3d_scale(c);
        for (k, a) in t.axes.iter().enumerate() {
            let color = if t.drag == Some(k) || (t.drag.is_none() && t.hover == Some(k)) { HOVER } else { a.color };
            vis.extra.push(arrow(c, a.dir, SHAFT_PX * s, (HANDLE_PX - CONE_L_PX) * s, CONE_R_PX * s, CONE_L_PX * s, color));
        }
    }

    /// Redraws the active view with the tool's look, and places the fields beside the handles.
    fn move3d_redraw(&mut self) {
        if let Some(i) = self.views.iter().position(|v| Some(v.id) == self.active) {
            self.update_inspection(i);
        }
        self.move3d_fields_place();
        self.refresh_active();
    }

    fn move3d_prompt(&mut self) {
        let Some(t) = self.move3d.clone() else { return };
        let what = if t.copy { "Copy" } else { "Move" };
        let text = match t.drag {
            Some(k) => format!("{} {} · {what}: let go to place it; Esc cancels", self.format_length(t.delta), t.axes[k].label),
            None => format!("{what}: drag a handle, or type distances in the box and press Enter; Esc ends"),
        };
        self.frame.set_status_text(&text, 0);
    }

    // ---- The fields ----

    /// The box of fields, one per axis, over the view beside the handles.
    fn move3d_fields_open(&mut self) {
        let Some(t) = self.move3d.clone() else { return };
        let Some(v) = self.active_view() else { return };
        let panel = Panel::builder(&v.panel).build();
        let sizer = FlexGridSizer::builder(0, 2).with_vgap(2).with_hgap(4).build();
        let mut fields = Vec::new();
        for a in &t.axes {
            let label = StaticText::builder(&panel).with_label(a.label).build();
            let field = TextCtrl::builder(&panel).with_value("0").with_size(Size::new(90, -1)).with_style(TextCtrlStyle::ProcessEnter).build();
            field.set_tooltip(&format!("Distance along {} (Enter moves)", a.label));
            sizer.add(&label, 0, SizerFlag::AlignCenterVertical | SizerFlag::All, 2);
            sizer.add(&field, 0, SizerFlag::All, 2);
            let gui = self.this.clone();
            field.on_text_enter(move |_| {
                if let Some(g) = gui.upgrade() {
                    super::with(&g, Gui::move3d_fields_enter);
                }
            });
            fields.push(field);
        }
        panel.set_sizer_and_fit(sizer, true);
        self.move3d_fields = Some(MoveFields { panel, fields });
    }

    /// Keeps the box beside the handles' centre, inside the view.
    fn move3d_fields_place(&self) {
        let (Some(t), Some(f), Some(v)) = (&self.move3d, &self.move3d_fields, self.active_view()) else { return };
        let size = v.panel.get_client_size();
        let c = add(t.center(), t.shift(), 1.0);
        let Some((px, py)) = v.camera.project(v3(c), size.width as f32, size.height as f32) else { return };
        let box_size = f.panel.get_size();
        let x = (px as i32 + 40).clamp(0, (size.width - box_size.width).max(0));
        let y = (py as i32 + 30).clamp(0, (size.height - box_size.height).max(0));
        f.panel.move_window(x, y);
    }

    /// Shows the distance dragged in its axis's field.
    fn move3d_fields_show(&mut self) {
        let Some(t) = self.move3d.clone() else { return };
        let dragged = self.format_length(t.delta);
        let texts: Vec<String> = (0..t.axes.len()).map(|k| if t.drag == Some(k) { dragged.clone() } else { "0".to_owned() }).collect();
        let Some(f) = &self.move3d_fields else { return };
        for (field, text) in f.fields.iter().zip(texts) {
            field.set_value(&text);
        }
    }

    /// Enter in a field: moves by the fields' distances.
    pub(crate) fn move3d_fields_enter(&mut self) {
        let frame = self.frame;
        let Some(t) = self.move3d.clone() else { return };
        let Some(f) = &self.move3d_fields else { return };
        let texts: Vec<String> = f.fields.iter().map(TextCtrl::get_value).collect();
        let mut v = [0.0; 3];
        for (a, text) in t.axes.iter().zip(texts) {
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            match self.run("units.parse", json!({ "text": text, "data_type": "length" })) {
                Ok(d) => v = add(v, a.dir, d.as_f64().unwrap_or_default()),
                Err(e) => return frame.set_status_text(&format!("{}: {e}", a.label), 0),
            }
        }
        self.move3d_by(v);
    }

    // ---- Dragging ----

    pub(crate) fn move3d_motion(&mut self, x: i32, y: i32) {
        let hover = self.handle_at(x, y);
        let Some(t) = &mut self.move3d else { return };
        if t.drag.is_none() && t.hover != hover {
            t.hover = hover;
            self.move3d_redraw();
        }
    }

    /// A press on a handle starts dragging it; `false` when there is none under the cursor.
    pub(crate) fn move3d_press(&mut self, x: i32, y: i32) -> bool {
        let Some(k) = self.move3d.as_ref().and(self.handle_at(x, y)) else { return false };
        let t = self.move3d.clone().expect("checked");
        let start = self.along_axis(x, y, t.center(), t.axes[k].dir).unwrap_or(0.0);
        if let Some(m) = &mut self.move3d {
            m.drag = Some(k);
            m.start = start;
            m.delta = 0.0;
        }
        self.move3d_redraw();
        self.move3d_prompt();
        true
    }

    pub(crate) fn move3d_drag_motion(&mut self, x: i32, y: i32) {
        let Some(t) = self.move3d.clone() else { return };
        let Some(k) = t.drag else { return };
        if let Some(s) = self.along_axis(x, y, t.center(), t.axes[k].dir)
            && let Some(m) = &mut self.move3d
        {
            m.delta = ((s - t.start) * 1000.0).round() / 1000.0;
        }
        self.move3d_fields_show();
        self.move3d_redraw();
        self.move3d_prompt();
    }

    /// Letting go of a handle: the selection moves (or is copied) by the distance dragged.
    pub(crate) fn move3d_drag_end(&mut self) {
        let Some(t) = self.move3d.clone() else { return };
        let v = t.shift();
        if let Some(m) = &mut self.move3d {
            m.drag = None;
            m.delta = 0.0;
            m.hover = None;
        }
        self.move3d_by(v);
    }

    /// Moves (or copies) the selection by `v` and stays on, the handles at its new centre.
    fn move3d_by(&mut self, v: [f64; 3]) {
        let frame = self.frame;
        let Some(t) = self.move3d.clone() else { return };
        if v.iter().all(|c| c.abs() < 1e-9) {
            self.move3d_fields_show();
            self.move3d_redraw();
            return self.move3d_prompt();
        }
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
            }
            Err(e) => super::error(&frame, &format!("Nothing was moved:\n{e}")),
        }
        self.move3d_fields_show();
        self.move3d_redraw();
        self.move3d_prompt();
    }

    /// Esc: cancels a drag, else ends the tool.
    pub(crate) fn move3d_key(&mut self, key: i32) -> bool {
        let Some(t) = self.move3d.clone() else { return false };
        if key != WXK_ESCAPE {
            return false;
        }
        if t.drag.is_some() {
            if let Some(m) = &mut self.move3d {
                m.drag = None;
                m.delta = 0.0;
            }
            self.move3d_fields_show();
            self.move3d_redraw();
            self.move3d_prompt();
        } else {
            self.end_move3d();
            self.frame.set_status_text("Ready", 0);
        }
        true
    }
}
