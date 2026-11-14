// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Curtain grids in the view (ARC-CW-004, ARC-CW-006): with a curtain wall selected, Modify |
//! Curtain Walls › Add Vertical Line, Add Horizontal Line, Remove Line and Change Panel work by
//! clicking on the wall in a 3D, elevation or section view (a plan, for vertical lines). The
//! point under the cursor is where the camera's ray meets the wall's face; the line or cell it
//! would change is drawn as the cursor moves. Each click is one change; Esc ends.

use riviera_core::ElementId;
use riviera_core::curtain::{self, Dir, Panel};
use riviera_render::RenderBackend;
use riviera_render::plan::Lines;
use serde_json::{Value, json};
use wxdragon::prelude::*;

use super::curtain::PANEL_NAMES;
use super::{Gui, error};

const PREVIEW: [f32; 4] = [0.05, 0.35, 0.95, 1.0];
/// How near (metres) a click must be to a manual line to remove it.
const NEAR: f64 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridAction {
    Add(Dir),
    Remove,
    /// The panel to put in clicked cells; `None` the type's.
    Panel(Option<Panel>),
}

/// The pick: the wall, what a click does, and the point under the cursor (along the wall from
/// its start, and absolute height).
#[derive(Debug, Clone)]
pub struct CurtainPick {
    wall: ElementId,
    action: GridAction,
    cursor: Option<[f64; 2]>,
}

impl Gui {
    /// Starts one of the grid picks on the selected curtain wall.
    pub(crate) fn start_curtain_pick(&mut self, action: &str) {
        let frame = self.frame;
        let Some(wall) = self.selection().into_iter().find(|id| self.store_now().and_then(|s| s.get(*id).and_then(curtain::curtain_recipe)).is_some()) else {
            return frame.set_status_text("Select a curtain wall first", 0);
        };
        let action = match action {
            "u" => GridAction::Add(Dir::U),
            "v" => GridAction::Add(Dir::V),
            "remove" => GridAction::Remove,
            _ => {
                let mut names: Vec<&str> = PANEL_NAMES.iter().map(|p| p.1).collect();
                names.push("The type's panel");
                let d = SingleChoiceDialog::builder(&frame, "Put which panel in the cells you click?", "Change Panel", &names).build();
                if d.show_modal() != ID_OK {
                    return;
                }
                let k = usize::try_from(d.get_selection()).unwrap_or(0);
                GridAction::Panel(PANEL_NAMES.get(k).map(|p| p.0))
            }
        };
        self.end_tool(&frame);
        self.curtain_pick = Some(CurtainPick { wall, action, cursor: None });
        self.curtain_pick_prompt();
    }

    pub(crate) fn end_curtain_pick(&mut self) -> bool {
        if self.curtain_pick.take().is_none() {
            return false;
        }
        if let Some(v) = self.viewport() {
            v.set_overlay_lines(&[]);
        }
        true
    }

    fn curtain_pick_prompt(&mut self) {
        let Some(p) = self.curtain_pick.clone() else { return };
        // The position first: the prompt is long and the status bar cuts its end off.
        let at = p.cursor.map(|c| format!("{} along, {} up · ", self.format_length(c[0]), self.format_length(c[1]))).unwrap_or_default();
        let what = match p.action {
            GridAction::Add(Dir::U) => "click where a vertical grid line goes",
            GridAction::Add(Dir::V) => "click where a horizontal grid line goes (3D, elevation or section)",
            GridAction::Remove => "click a grid line you placed to remove it",
            GridAction::Panel(_) => "click the cells whose panel changes",
        };
        self.frame.set_status_text(&format!("{at}Curtain grid: {what}; Esc ends"), 0);
    }

    /// Where the cursor's ray meets the wall's face: along the wall from its start, and the
    /// absolute height (in a plan, the height is the wall's mid-height).
    fn curtain_hit(&self, x: i32, y: i32, wall: ElementId) -> Option<([f64; 2], curtain::CurtainGeometry)> {
        let store = self.store_now()?;
        let g = curtain::regenerate(&store, store.get(wall)?).ok()?;
        let v = self.active_view()?;
        let size = v.panel.get_client_size();
        let (o, d) = v.camera.ray(x as f32, y as f32, size.width as f32, size.height as f32);
        let (a, b) = (g.line.start, g.line.end);
        let l = g.length().max(1e-9);
        let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
        let n = [-u[1], u[0]];
        if v.plan.is_some() {
            // A plan: the point on the base line under the cursor.
            let p = self.plan_xy(x, y)?;
            let s = (p[0] - a[0]) * u[0] + (p[1] - a[1]) * u[1];
            return Some(([s.clamp(0.0, l), (g.z[0] + g.z[1]) / 2.0], g));
        }
        // The wall's own surface under the cursor, as the scene picks it; else the vertical
        // plane through the base line.
        let picked = self.scene().pick_within(o, d, f32::NEG_INFINITY, f32::INFINITY).filter(|(i, _)| self.scene().meshes[*i].element.is_some_and(|e| e == wall || store.get(e).and_then(|p| p.param(riviera_core::mesh_objects::PART_OF_KEY)).and_then(serde_json::Value::as_str) == Some(store.get(wall).map(|w| w.guid().to_string()).unwrap_or_default().as_str())));
        let p = match picked {
            Some((_, t)) => {
                let h = o + d * t;
                [f64::from(h.x), f64::from(h.y), f64::from(h.z)]
            }
            None => {
                let denom = f64::from(d.x) * n[0] + f64::from(d.y) * n[1];
                if denom.abs() < 1e-9 {
                    return None;
                }
                let t = ((a[0] - f64::from(o.x)) * n[0] + (a[1] - f64::from(o.y)) * n[1]) / denom;
                if t <= 0.0 {
                    return None;
                }
                [f64::from(o.x) + f64::from(d.x) * t, f64::from(o.y) + f64::from(d.y) * t, f64::from(o.z) + f64::from(d.z) * t]
            }
        };
        let s = (p[0] - a[0]) * u[0] + (p[1] - a[1]) * u[1];
        ((-0.05..=l + 0.05).contains(&s) && (g.z[0] - 0.05..=g.z[1] + 0.05).contains(&p[2])).then_some(([s.clamp(0.0, l), p[2]], g))
    }

    pub(crate) fn curtain_pick_motion(&mut self, x: i32, y: i32) {
        let Some(p) = self.curtain_pick.clone() else { return };
        let hit = self.curtain_hit(x, y, p.wall);
        let mut segments = Vec::new();
        if let Some((c, g)) = &hit {
            let (a, b) = (g.line.start, g.line.end);
            let l = g.length().max(1e-9);
            let at = |s: f64, z: f64| [(a[0] + (b[0] - a[0]) * s / l) as f32, (a[1] + (b[1] - a[1]) * s / l) as f32, z as f32];
            match p.action {
                GridAction::Add(Dir::U) => segments.push([at(c[0], g.z[0]), at(c[0], g.z[1])]),
                GridAction::Add(Dir::V) => segments.push([at(0.0, c[1]), at(l, c[1])]),
                GridAction::Remove => {
                    if let Some((dir, at_, _)) = self.nearest_manual(p.wall, *c) {
                        segments.push(match dir {
                            Dir::U => [at(at_, g.z[0]), at(at_, g.z[1])],
                            Dir::V => [at(0.0, g.z[0] + at_), at(l, g.z[0] + at_)],
                        });
                    }
                }
                GridAction::Panel(_) => {
                    if let Some(part) = cell_at(g, *c) {
                        let (x0, x1, z0, z1) = (part.x[0], part.x[1], part.z[0], part.z[1]);
                        segments.extend([[at(x0, z0), at(x1, z0)], [at(x1, z0), at(x1, z1)], [at(x1, z1), at(x0, z1)], [at(x0, z1), at(x0, z0)]]);
                    }
                }
            }
        }
        if let Some(t) = &mut self.curtain_pick {
            t.cursor = hit.map(|h| h.0);
        }
        if let Some(v) = self.viewport() {
            v.set_overlay_lines(&[Lines { color: PREVIEW, width: 1.0, segments }]);
        }
        self.curtain_pick_prompt();
    }

    /// The manual line nearest a point on the wall (its direction, position and key).
    fn nearest_manual(&self, wall: ElementId, c: [f64; 2]) -> Option<(Dir, f64, u32)> {
        let store = self.store_now()?;
        let e = store.get(wall)?;
        let r = curtain::curtain_recipe(e)?;
        let g = curtain::regenerate(&store, e).ok()?;
        r.grids
            .iter()
            .map(|m| {
                let d = match m.dir {
                    Dir::U => (m.at - c[0]).abs(),
                    Dir::V => (g.z[0] + m.at - c[1]).abs(),
                };
                (d, m)
            })
            .filter(|(d, _)| *d < NEAR)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, m)| (m.dir, m.at, m.key))
    }

    pub(crate) fn curtain_pick_click(&mut self, x: i32, y: i32) {
        let frame = self.frame;
        let Some(p) = self.curtain_pick.clone() else { return };
        let Some((c, g)) = self.curtain_hit(x, y, p.wall) else {
            return frame.set_status_text("Curtain grid: click on the selected curtain wall", 0);
        };
        let wall = p.wall;
        let result = match p.action {
            GridAction::Add(Dir::U) => self.run("curtain.add_grid", json!({ "curtain_id": wall, "direction": "u", "position": round(c[0]) })),
            GridAction::Add(Dir::V) => {
                if self.active_view().is_some_and(|v| v.plan.is_some()) {
                    return frame.set_status_text("Curtain grid: horizontal lines are placed in a 3D, elevation or section view", 0);
                }
                self.run("curtain.add_grid", json!({ "curtain_id": wall, "direction": "v", "position": round(c[1] - g.z[0]) }))
            }
            GridAction::Remove => match self.nearest_manual(wall, c) {
                Some((_, _, key)) => self.run("curtain.delete_grid", json!({ "curtain_id": wall, "grid_key": key })),
                None => return frame.set_status_text("Curtain grid: no grid line you placed is there (lines of the type's layout change in its type)", 0),
            },
            GridAction::Panel(panel) => match cell_at(&g, c) {
                Some(part) => self.run("curtain.set_panel_type", json!({ "curtain_id": wall, "cells": [part.cell], "panel": panel.map_or(Value::Null, |p| json!(p)) })),
                None => return,
            },
        };
        if let Err(e) = result {
            error(&frame, &format!("The curtain wall was not changed:\n{e}"));
        }
        self.properties_stale = true;
        self.refresh_scene(&frame);
        self.curtain_pick_motion(x, y);
    }

    /// Esc ends the pick.
    pub(crate) fn curtain_pick_escape(&mut self) -> bool {
        let done = self.end_curtain_pick();
        if done {
            self.frame.set_status_text("Ready", 0);
        }
        done
    }
}

/// To the millimetre.
fn round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// The panel whose cell holds a point on the wall (along, absolute height).
fn cell_at(g: &curtain::CurtainGeometry, c: [f64; 2]) -> Option<&curtain::CurtainPart> {
    g.parts.iter().find(|p| !p.mullion && (p.x[0]..=p.x[1]).contains(&c[0]) && (p.z[0]..=p.z[1]).contains(&c[1]))
}
