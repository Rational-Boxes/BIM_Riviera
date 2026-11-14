# Edit operations audit (2026-11-13)

Owner (2026-11-13): "Please audit all edit operations. I attempted to change the types of roofs,
doors, windows, etc. and not all edit tools appear to be complete."

The audit looked at two levels.

- **Core.** Every typed element of the hotel sample (`test-data/samples/hotel.ifc`) was swapped by
  `element.set_type` to each other type of its family.
- **Window.** The Properties rows, the contextual Modify tabs and the ribbon actions were checked
  for every category. Doors, roofs and components were also checked by hand on the hotel in the GUI.

**Summary.** The core can change the type of almost everything; only railings, stairs and duct/pipe
runs are refused. The window is where it falls short:

1. **No type catalogues.** Every Type list shows only the types already in the model, so there is
   usually one choice besides *Duplicate Type…*. The hotel has two door types, one window type and
   one roof type. Only mesh components (`library.rs`), structural sections (Load Section) and MEP
   starters have anything to load from.
2. **Many categories have no Type selector at all**, though the core would accept the swap.
3. **Several categories return before the generic parameter rows**, so Mark, Comments, project
   parameters and Psets cannot be edited for them.
4. **A dozen existing core edit commands have no window path.**

## 1. Type selector and type values, by category

| Category | Type selector | Duplicate | Type values editable | Gap |
|---|---|---|---|---|
| Walls (basic, stacked) | yes | yes | name, width; layers (Edit Assembly), rows (Edit Structure) | catalogue only |
| Curtain walls / storefront | yes | yes | grids, mullions, panel | no rename; cell panel swap and manual grid lines (`curtain.set_panel_type`, `curtain.add_grid`/`move_grid`/`delete_grid`/`set_grid_segments`) not in the window |
| Doors, windows | yes | yes | every `InsertType` field, shape, operation, leaves, automatic | catalogue only |
| Floors / ceilings | yes, but **floor and ceiling types in one list** (`sketch.rs:1582`) | yes | thickness; layers by Edit Assembly | wrong-kind pick fails; no rename; Level read-only (`floor.set_level` exists) |
| Roofs (footprint, arched, dome) | yes | yes | **thickness only** | roof types have no layers or materials in core (`roofs.rs:1531`); no rename; Base Level read-only (`roof.set_params.level_id` exists); arch `radius`/`spring_edges` not editable |
| Roof by extrusion | footprint rows shown instead of its own | yes | thickness | `roof.set_extrusion` (profile, start, end) has no window path |
| Sloped glazing types | assigned only | – | none | `roof.set_glazing_type`, `glazing.set_type`, `frame.set_type` unused |
| Geodesic dome | none (own rows) | – | – | Roofs tab shown but its tools only act on roof recipes |
| Ramps | **none** (`ramps.rs:268`) | no | max slope, thickness: **edits the shared type in place** | core `set_type` supports ramps |
| Stairs | **none** | no | none | **no core support** (`set_type` refuses; `type.set_param` has no stair branch); Base/Top read-only (`stairs.rs:645,652`) though `stair.set_params` takes them; stair edits don't refresh Properties |
| Railings / gates | **no Properties rows at all** | – | – | no core type swap; `railing_type.create`, `railing.move_feature`, `railing.remove_feature` have no window path; the tool never offers a type |
| Wall openings / shafts | – | – | – | no Modify tab (`cat.openings`); no Edit Boundary sketch for shafts |
| Structural columns, beams, braces | yes | yes | section, sizes, material | **no Modify tab** for `cat.structural_columns` / `cat.structural_framing` (Attach/Detach Top only on `cat.columns`); generic rows hidden |
| Footings, wall foundations | **none**, no rows | no | none (core `type.set_param` takes width, length, thickness, toe, heel…) | core `set_type` supports them; placing offers no type |
| Ducts, pipes, cable trays, conduit | **none**, no rows | no | no | **size can't be changed** (`duct.set_size`, `pipe.set_size`, `cable_tray.set_size`, `conduit.set_size` unused); **type swap not in core**; `mep_segment_type.create`, `routing_preferences.*`, `mep_run.reapply_type`, `mep_run.lock_size` unused |
| Placed components (furniture, fixtures, terminals, equipment, appliances, hoods, gym, seating, cabinets, site components, lift cars) | **none**: "Type Name" is a text box that **renames the shared type** (`components.rs:1555`) | no | name, category, IFC type, scale, colours (all copies) | core `set_type` swaps within a category; library types can be loaded but not chosen here; Level read-only (`mesh_object.set` takes `level`) |
| Sprinklers | as components | – | – | `sprinkler_type.set` (K-factor, temperature, coverage) unused |
| Rebar | – | – | – | bar type and shape read-only (`rebar.set` takes them) |
| Analytical members | – | – | – | section type read-only, rotation hidden (`analytical_member.set` takes both) |
| Rooms | – | – | – | **Number has a handler but no row** (`rooms.rs:409`); Upper Limit read-only (`room.set` takes both) |
| Pools | no rows | – | – | `pool.set` (outline, deck, water level, profile, cove, coping, shell, class, entries) and `pool.remove_entry` unused |
| Model groups | – | – | – | `group.swap_type`, `group.duplicate_type`, `group.rename_type` unused |
| Elevators, escalators, stairlifts | car shows component rows: offset/rotation/scale **can move the car off its shaft** | – | – | `elevator.delete`, `escalator.delete`, `stairlift.delete` unused (Delete removes the car only) |
| Grab bars, toposolids, unit arrays | – | – | – | `grab_bar.set`, `toposolid.set_points`, `unit.set_array` unused |

## 2. Generic parameters hidden

`fill_properties` (`riviera-ui/src/lib.rs`) returns before the `param.list` rows for:

- floors, roofs and rooms;
- columns, beams, analytical elements, rebar and components;
- curtain walls and ramps.

So Mark, Comments, project parameters and Psets can't be edited on those categories. Lift car
capacity and size are among the values hidden this way.

## 3. Silent no-ops

- *Duplicate Type…* does nothing when the element has no current type: roofs (`roofs.rs:500`),
  floors (`sketch.rs:1665`) and curtain walls.
- A roof type name that isn't found does nothing either (`roofs.rs:545`).
- A bad scale entry on a component silently becomes 1.0 (`components.rs:1663`).

## 4. Layout tools that can't be re-edited

- **Re-run with Replace:** bathroom layout, sauna, steam room and toilet core.
- **Seating:** re-runs, but its form starts from the defaults, not the last values.
- **Delete piece by piece:** gym, cabinet runs, kitchen line-ups, closets, tile, baffles and
  atriums need a core set or replace command before their results can be changed as a whole.

## 5. Fix plan

**Phase 1: type changes.**

- A Type selector and *Duplicate Type…* for:
  - components, listing the model's types of the category plus the component library's;
  - ramps, footings and wall foundations.
- Separate the floor and ceiling lists.
- Add rename rows for roof, floor, ceiling, curtain and ramp types.
- Fix the no-op paths.
- Ship **starter type catalogues** for doors, windows, roofs, floors and ceilings:
  - doors: single, double, sliding, glass storefront, fire-rated;
  - windows: fixed, casement and awning, in common sizes;
  - roofs, floors and ceilings: common build-ups.

  They load from the Type list ("Load more types…").

**Phase 2: MEP runs.**

- Size and type rows for ducts, pipes, trays and conduit.
- Core `element.set_type` for segments, re-applying routing preferences.

**Phase 3: instance values the core already takes.**

- Stair base and top, plus stair types in core.
- Roof and floor level.
- Room number and upper limit.
- Component level.
- Roof by extrusion profile.
- Pool settings.
- Rebar bar type and shape.
- Analytical section.
- Sprinkler types.
- Railing rows and railing types (plus the core type swap).

**Phase 4: the generic parameter rows and Modify tabs.**

- Generic parameter rows after every category's own rows.
- Modify tabs for:
  - structural framing and columns;
  - ducts, pipes and fittings;
  - ramps and openings;
  - pools and vertical transport.
- Lift deletes as whole assemblies.
- Group type swap.

**Phase 5: re-editable layouts.** Core replace commands for line-ups, cabinet runs, gym layouts,
closets, baffles and atriums, then their window forms filled from the last values.

## 6. Progress

**2026-11-13: phases 1 and 2 done, phase 3 begun.**

- **Type catalogues** (DM-TYPE-002): the Type row with Load, Duplicate and Type Name for every
  kind listed in §1 except stairs, curtain walls, columns, beams and grids (those keep their own
  selectors).
  - Starter types: doors, windows, walls, floors, ceilings, roofs, ramps and railings.
  - Component libraries load by category.
  - Floor and ceiling lists are separate.
  - The Door, Window and Wall tools can pick catalogue types before placing.
  - The no-op Duplicate paths are gone wherever the Type row replaced them.
- **Footings and wall foundations:** a Type row and their type values.
- **Duct, pipe, tray and conduit segments:**
  - a Type row (core `element.set_type` now handles segments and refits fittings);
  - Diameter or Width/Height;
  - Size Locked.
- **Railings:**
  - core type swap and `type.set_param` for railing types;
  - Properties rows: Type, type sizes, and each opening's or gate's position and width (clear
    the position to remove it).
- **Phase 3, done so far:**
  - stair Base Level, Base Offset, Top (a level or Unconnected), Top Offset or Unconnected
    Height, and Auto-Extend; stair edits now refresh Properties;
  - roof Base Level and floor/ceiling Level as level choices;
  - room Number and Upper Limit;
  - component Level (bad scale input is refused rather than becoming 1);
  - roof by extrusion: Extrusion Start and End (no footprint rows);
  - pools: deck, water level, shallow and deepest depth, cove, class, coping, shell, tile and
    membrane, and entries listed.

**2026-11-13, continued (phase 3 and 4):**
- **Stair types:** core type swap and `type.set_param`; Properties Type row and the type's
  rules; four starter stairs.
- **Rebar:** Shape and Bar type are choices (`rebar.set`).
- **Analytical members:** Section Type is a choice and Rotation is editable.
- **Sprinkler types:** the head's mounting, K-factor, temperature, response, activation,
  coverage and connection size rows (`sprinkler_type.set`).
- **Generic parameter rows** (Identity Data, project parameters, Psets) now follow every
  element's own rows, not only walls'.
- **New Modify tabs:** ducts, pipes, duct and pipe fittings, cable trays, conduits, structural
  columns, structural framing, pools and lifts.
- **Lifts:** deleting a lift's car deletes the lift (`elevator.delete`, `escalator.delete`,
  `stairlift.delete`); the car's offset, rotation, scale and level rows are read-only.
- **Model groups:** Change, Duplicate and Rename Group Type on Modify | Model Groups.

**Still to do:**
- **Phase 3:**
  - arch radius and spring edges;
  - curtain grid and panel tools;
  - sloped glazing types;
  - grab bars, toposolid points, unit arrays.
- **Phase 4:**
  - Modify tabs for ramps and openings (no tools for them yet).
- **Phase 5:** re-editable layouts.

**2026-11-13, continued (phases 3 and 5):**
- **Arched roofs:** a Spring Edges choice (the opposite footprint edges the arch springs from).
- **Curtain walls:**
  - manual grid lines listed (move one, or clear it to delete it), with Add Vertical and Add
    Horizontal Line At rows;
  - panel overrides listed, and a Change Panels row (`2,0 3,0 door`);
  - Duplicate Type is offered only when there is a type.
- **Sloped glazing types:** grid 1 and grid 2 layouts and spacings, mullion size, and panel
  (`roof.set_glazing_type`).
- **Grab bars:** diameter, gap to wall and top height (`grab_bar.set`).
- **Toposolids:** each point's height (clear it to remove the point), and an Add Point row
  (`toposolid.set_points`).
- **Change Unit Array** (`unit.set_array`): levels, sides, corridor, pitch, count, start, skips
  and handing.
- **Phase 5, Edit Layout** (DM-TYPE-003): every layout tool's pieces remember the command and its
  arguments. `layout.get` and `layout.redo` re-make the layout with changed arguments in one undo
  step. The window has an Edit Layout form on Modify | Components and on Kitchen & Casework.

- **Atriums** are made again as a whole by Edit Layout (`atrium.create` is a layout command); Edit
  Layout is also on Glazed Spaces and the Roofs and Railings Modify tabs.

- **The user's type catalogue** (DM-TYPE-004): Save Type to My Catalogue… and Remove from My
  Catalogue… in every Type list; saved types are listed and loaded in every project.

**2026-11-14:**
- **The hotel sample regenerated**, so its layouts and atrium carry what made them and Edit
  Layout works on them.
- **Ramps:** Modify | Ramps with Edit Path, Flip Direction and Railings; the tool's Railings
  row (on above a 150 mm rise); a typed Path row; the type's Shape row. Railings follow the ramp
  (`ramp.add_railings`).
- **Curtain grids in the view:** Vertical Line, Horizontal Line, Remove Line and Change Panel on
  Modify | Curtain Walls, by clicking on the wall in 3D, elevation, section or plan.

**Remaining:**
- A Modify tab for openings (shafts have Edit Boundary through Properties only).
