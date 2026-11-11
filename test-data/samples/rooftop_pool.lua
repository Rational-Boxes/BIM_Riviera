-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A rooftop pool
-- description: A hotel's 10 × 4 m rooftop pool sunk into its roof terrace over a plant floor: its water, shell and tiles as loads on the slab's analytical panel, and the terrace's receptacles, switch, luminaires and underwater lights checked against NEC Article 680, with the bonding grid it needs.
--
-- Riviera sample model (test-data/samples): a pool's structure and electrical bonding
-- (ARC-POOL-010). `pool.structure` weighs the water (9.81 kN/m³), the concrete shell
-- (24.5 kN/m³) and the tiling (1.25 kN/m²) and, since a level is below it, gives the slab's
-- span/360 deflection limit and the rooftop notes; `pool.apply_loads` puts the weight on the
-- slab's analytical panel in a dead case and a fluid case. `pool.bonding` checks each electrical
-- element round the pool against NEC 680's distances and heights, and sizes the 8 AWG bonding
-- grid under the 3 ft perimeter surface.

local plant = riviera.level.list()[1].id
riviera.level.set_elevation{ level_id = plant, elevation = 8.0 }
riviera.level.rename{ level_id = plant, name = "Plant" }
local roof = riviera.level.create{ elevation = 12.0, name = "Roof" }

local function rect(x0, y0, x1, y1)
  local p = { { x0, y0 }, { x1, y0 }, { x1, y1 }, { x0, y1 } }
  local out = {}
  for i = 1, 4 do out[i] = { line = { start = p[i], ["end"] = p[i % 4 + 1] } } end
  return out
end
-- The roof terrace, 18 × 12 m, its top at the roof level.
riviera.floor.create{ loops = { rect(0, 0, 18, 12) }, level_id = roof }

-- The ISPSC's barrier round the terrace (ARC-POOL-012), its gate by the stair opening outward.
local fence_type = riviera.railing_type.create{ name = "Pool fence 4 ft", height = 1.25, bottom_rail_height = 0.05, baluster = 0.025, baluster_spacing = 0.12 }
local fence = riviera.railing.create{ type_id = fence_type, path = { { 0.1, 0.1 }, { 17.9, 0.1 }, { 17.9, 11.9 }, { 0.1, 11.9 } }, closed = true, base_level = roof }
local gate = riviera.railing.add_gate{ railing_id = fence, point = { 2, 0.1 }, swing_out = true }
-- Self-closing, self-latching and lockable (ARC-POOL-016), its latch 53 in up, inside the ISPSC's 52–54 in for public pools.
riviera.railing.gate_hardware{ gate_id = gate, latch_height = 1.35 }

-- 10 × 4 m, 1.2 m deep throughout (a hotel pool, no diving), a 250 mm concrete shell, tiled.
local pool = riviera.pool.create{ level = roof, name = "Rooftop pool",
  outline = { { 4, 8 }, { 4, 4 }, { 14, 4 }, { 14, 8 } },
  profile = { { at = 0, depth = 1.2 }, { at = 10, depth = 1.2 } } }
riviera.pool.set{ pool_id = pool, shell = { kind = "cast_concrete", thickness = 0.25, tile = 0.012, membrane = true } }
riviera.pool.depth_markings{ pool_id = pool }

-- From the Riviera Pools library: a lift on the north deck, a ladder at the east end and two
-- underwater lights in the south wall, their lenses 18 in under the water.
local t = riviera.furnishing.load{ names = { "Pool lift, 300 lb", "Pool ladder, 3 steps", "Underwater light, large niche" } }
local function place(name, x, y, turn)
  return riviera.mesh_object.place{ type_id = t[name], point = { x, y }, level = roof, rotation = turn }
end
place("Pool lift, 300 lb", 6.5, 8.6, 0)
place("Pool ladder, 3 steps", 12.5, 8, 0)
place("Underwater light, large niche", 7, 4, math.pi)
place("Underwater light, large niche", 11, 4, math.pi)

-- On the terrace: two receptacles 2.5 m out (more than 6 ft, so GFCI only), the lights' switch
-- 2 m out, two deck luminaires 3.5 m from the water, and the terrace's board well away.
local m = riviera.mep_type.load_starters{ names = { "Receptacle", "Light Switch", "Ceiling Luminaire", "Distribution Board" } }
local function device(name, x, y, offset)
  return riviera.device.place{ type_id = m[name], point = { x, y }, level = roof, offset = offset, work_plane = true }
end
device("Receptacle", 6, 10.5, 0.45)
device("Receptacle", 12, 10.5, 0.45)
device("Light Switch", 16, 6, 1.1)
for _, x in ipairs({ 6, 12 }) do
  riviera.lighting_fixture.place{ type_id = m["Ceiling Luminaire"], point = { x, 0.5 }, level = roof, offset = 3.0, work_plane = true }
end
riviera.electrical_equipment.place{ type_id = m["Distribution Board"], point = { 17.5, 11.5 }, level = roof, work_plane = true }

-- Its loads.
local s = riviera.pool.structure{ pool_id = pool }[1]
print(string.format("Water %.0f kN, shell %.0f kN, tiles %.0f kN: %.1f kN/m² full over %.1f m² on average; %.1f kN/m² on the floor, the walls %.1f kN/m", s.water_weight, s.shell_weight, s.finishes, s.full, s.footprint, s.peak, s.wall_line_load))
print(string.format("Walls: %.1f kPa at the base, %.1f kN·m/m; the slab's deflection kept under %.0f mm", s.wall.pressure, s.wall.moment, s.deflection_limit))
for _, p in ipairs(s.point_loads) do print(string.format("%s: %.1f kN at its anchorage", p.object_type, p.load)) end
assert(s.elevated, "a level is below it")

-- The slab under the shell (its underside 1.6 m below the terrace) as an analytical panel, and
-- the pool's weight on it: the shell and tiles as dead load, the water as a fluid load.
local z = 12.0 - 0.15 - 1.2 - 0.25
local panel = riviera.analytical_panel.create_by_boundary{
  boundary = { { 3, 3, z }, { 15, 3, z }, { 15, 9, z }, { 3, 9, z } }, thickness = 0.35, material = "Concrete f'c 4000 psi" }
riviera.load_nature.create{ name = "Fluid", symbol = "F", action_type = "permanent", action_source = "USERDEFINED" }
local dead = riviera.load_case.create{ name = "D", nature = "D", self_weight = true }
local fluid = riviera.load_case.create{ name = "F", nature = "F" }
local a = riviera.pool.apply_loads{ pool_id = pool, panel_id = panel, dead_case_id = dead, water_case_id = fluid }
print(string.format("On the %.0f m² panel: %.1f kN/m² dead and %.1f kN/m² of water", a.panel_area, a.dead_pressure / 1000, a.water_pressure / 1000))

-- Its electrical zones and bonding.
local b = riviera.pool.bonding{ pool_id = pool }[1]
print(string.format("Bonding grid: %.1f m² of perimeter surface, %.0f m of 8 AWG copper, tied at %d points", b.grid.perimeter_surface, b.grid.grid_conductor, #b.grid.ties))
for _, x in ipairs(b.bond) do print("Bond: " .. x.what) end
for _, f in ipairs(b.findings) do print(f.severity, f.source, f.message) end
for _, f in ipairs(b.findings) do assert(f.severity ~= "error", f.rule .. ": " .. f.message) end
