-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: An atrium hotel with a roof pool
-- description: A 60 × 40 m hotel on a steel frame round a five-storey atrium, over a concrete basement on strip and pad footings with its plant, laundry, storage and a sump for groundwater: guest and service lifts in its four corners; a lobby with a covered drop-off, a ballroom, three conference rooms and a restaurant on the ground floor, each set out for its use; a gym and a spa on the second floor; 88 guest rooms on four floors laid out from two room units, merged into the façade and corridor walls with their balconies, every eighth accessible, their diffusers connected to the corridor mains; and on the roof a pool on a raised podium under the atrium's glazed roof, which reaches over the pool and the roof patio, with sun-control baffles over both, and solar panels on the rest of the roof round its air handling units; on its site, a drive from the street under the porte-cochère and back, a car park and a lawn.
--
-- Riviera sample model (test-data/samples): the hotel scenario (owner, 2026-11-12). Modular
-- units (ARC-GRP-004): `unit.create_type` makes a guest room a type with its 4.0 × 7.5 m bay,
-- `unit.array` repeats it down each corridor on four floors, mirrored in pairs, numbered by
-- floor, with an accessible variant every eighth room; each copy's corridor and façade walls
-- merge into the building's, its door and window cut into them, its balcony joined to the
-- floor; `unit.connect` runs every room's diffuser into its corridor's supply main. Atriums
-- (ARC-GLZ-018), sun-control baffles (ARC-GLZ-019) and event and dining seating (ARC-INT-006).

local FT = 0.3048
local l1 = riviera.level.list()[1].id
riviera.level.rename{ level_id = l1, name = "Ground" }
-- Floors: the ground floor 5 m high, four guest floors at 3.5 m, the roof terrace, and the pool
-- podium raised 1.8 m on the roof so the pool's shell stays clear of the roof slab.
local floors = { l1 }
for k = 2, 5 do
  floors[k] = riviera.level.create{ elevation = 5.0 + 3.5 * (k - 2), name = "Level " .. k }
end
local roof = riviera.level.create{ elevation = 19.0, name = "Roof" }
-- A basement 4 m down for the plant, the laundry and storage.
local basement = riviera.level.create{ elevation = -4.0, name = "Basement" }
local podium = riviera.level.create{ elevation = 20.8, name = "Pool podium" }

local function rect(x0, y0, x1, y1)
  local p = { { x0, y0 }, { x1, y0 }, { x1, y1 }, { x0, y1 } }
  local out = {}
  for i = 1, 4 do out[i] = { line = { start = p[i], ["end"] = p[i % 4 + 1] } } end
  return out
end
local function wall(level, a, b, h)
  return riviera.wall.create{ curve = { line = { start = a, ["end"] = b } }, base_level = level, top_constraint = { unconnected = h } }
end

-- Slabs and the façade on every floor; the roof slab is the terrace.
for k, l in ipairs(floors) do
  riviera.floor.create{ loops = { rect(0, 0, 60, 40) }, level_id = l }
end
riviera.floor.create{ loops = { rect(0, 0, 60, 40) }, level_id = roof }
local facade = {}
for k, l in ipairs(floors) do
  -- To the underside of the 300 mm slab above.
  local h = k == 1 and 4.7 or 3.2
  facade[k] = {
    south = wall(l, { 0, 0 }, { 60, 0 }, h), east = wall(l, { 60, 0 }, { 60, 40 }, h),
    north = wall(l, { 60, 40 }, { 0, 40 }, h), west = wall(l, { 0, 40 }, { 0, 0 }, h),
  }
end

-- The frame: W14 columns along the atrium's long sides (one a storey, standing between the
-- slabs) and outside the façades at the room dividers (between the balconies, full height), W24
-- girders along the atrium lines under every floor (none across the atrium; the ballroom and the
-- conference rooms are column-free, 19.6 m spans), and heavier W24X84 girders under the roof,
-- which carries the pool. The girders hang under the 300 mm slabs.
local column = riviera.column.load_section{ catalog = "aisc_w", designation = "W14X90" }
local girder = riviera.beam.load_section{ catalog = "aisc_w", designation = "W24X76" }
local transfer = riviera.beam.load_section{ catalog = "aisc_w", designation = "W24X84" }
local storeys = { basement, l1, floors[2], floors[3], floors[4], floors[5], roof }
for k = 1, 6 do
  for _, p in ipairs({ { 0.4, 10 }, { 20, 10 }, { 32, 10 }, { 40, 10 }, { 59.6, 10 }, { 0.4, 30 }, { 20, 30 }, { 32, 30 }, { 40, 30 }, { 59.6, 30 } }) do
    riviera.column.place{ type_id = column, point = p, base_level = storeys[k], top_constraint = { level = storeys[k + 1] }, top_offset = -0.3 }
  end
end
for _, x in ipairs({ 8, 20, 32, 40, 52 }) do
  for _, y in ipairs({ -0.35, 40.35 }) do
    riviera.column.place{ type_id = column, point = { x, y }, base_level = basement, base_offset = -0.3, top_constraint = { level = roof } }
  end
end
for k = 2, 7 do
  local level = storeys[k]
  local ty = level == roof and transfer or girder
  local xs = { 0.4, 20, 32, 40, 59.6 }
  for _, y in ipairs({ 10, 30 }) do
    for i = 1, #xs - 1 do
      -- Level 3's end girders would cross the gym's and the spa's walls below them.
      if not (level == floors[3] and (i == 1 or i == #xs - 1)) then
        riviera.beam.create{ type_id = ty, reference_level = level, curve = { start = { xs[i], y }, ["end"] = { xs[i + 1], y } }, start_offset = -0.3, end_offset = -0.3 }
      end
    end
  end
end

-- The lifts in the four corners: a guest lift in each, and a service lift beside it in the
-- south-east and north-west corners, from the basement to the roof.
local function lift(x, y, turn, kind)
  return riviera.elevator.create{ point = { x, y }, rotation = turn, base_level = basement, top_level = roof, kind = kind, equipment = false }
end
lift(2.0, 4.0, math.rad(-90), "passenger")
lift(58.0, 4.0, math.rad(90), "passenger")
lift(54.85, 4.0, math.rad(90), "freight")
lift(2.0, 36.0, math.rad(-90), "passenger")
lift(5.15, 36.0, math.rad(-90), "freight")
lift(58.0, 36.0, math.rad(90), "passenger")

-- The atrium: a 23 × 19 m void from the ground floor up through the roof, 42 in guards round
-- every opened floor, and a glazed ridge roof 6 m above the terrace that reaches east and north
-- over the pool podium and the roof patio.
local atrium = riviera.atrium.create{
  outline = { { 18.5, 10.5 }, { 41.5, 10.5 }, { 41.5, 29.5 }, { 18.5, 29.5 } },
  base_level = l1, top_level = roof, form = "ridge", roof_offset = 6.0,
  roof_outline = { { 18.2, 10.2 }, { 58.6, 10.2 }, { 58.6, 37.8 }, { 18.2, 37.8 } },
  name = "Atrium",
}
for _, r in ipairs(riviera.atrium.check{}) do
  print(string.format("Atrium: %d storeys, %.1f m to the glazing, %.0f m²; the smoke layer at %.1f m, %.0f kg/s, exhaust %.0f m³/s", r.storeys, r.height, r.area, r.smoke.clear_height, r.smoke.mass_flow, r.smoke.exhaust))
  for _, f in ipairs(r.findings) do print("  " .. f.rule .. ": " .. f.message) end
end

-- ---- The basement ----
-- Mass concrete below ground: a 300 mm concrete slab to the inner faces of 300 mm concrete
-- retaining walls, the walls on strip footings 0.3 m below it (they meet once at the corners),
-- and pad footings under the basement's columns, their tops at the slab's underside.
local function type_named(name, class)
  for _, t in ipairs(riviera.type.list()) do
    if (name == nil or t.name == name) and (class == nil or t.class == class) then return t.id end
  end
end
local bslab = riviera.floor.create{ loops = { rect(0.15, 0.15, 59.85, 39.85) }, level_id = basement, structural = true }
local slab_type = riviera.type.duplicate{ type_id = type_named("Generic Floor - 300mm"), new_name = "Concrete Slab 300" }
riviera.compound.set_layers{ type_id = slab_type, layers = { { ["function"] = "structure", material = "Concrete", thickness = "300 mm" } } }
riviera.element.set_type{ element_ids = { bslab }, type_id = slab_type }
local retaining = {}
for _, w in ipairs({ { { 0, 0 }, { 60, 0 } }, { { 60, 0 }, { 60, 40 } }, { { 60, 40 }, { 0, 40 } }, { { 0, 40 }, { 0, 0 } } }) do
  retaining[#retaining + 1] = riviera.wall.create{ curve = { line = { start = w[1], ["end"] = w[2] } }, base_level = basement, base_offset = -0.3, top_constraint = { unconnected = 4.0 }, structural = true }
end
local wall_type = riviera.type.duplicate{ type_id = type_named(nil, "IfcWallType"), new_name = "Concrete Retaining Wall 300" }
riviera.compound.set_layers{ type_id = wall_type, layers = { { ["function"] = "structure", material = "Concrete", thickness = "300 mm" } } }
riviera.element.set_type{ element_ids = retaining, type_id = wall_type }
riviera.wall_foundation.create{ wall_ids = retaining }
for _, p in ipairs({ { 20, 10 }, { 32, 10 }, { 40, 10 }, { 20, 30 }, { 32, 30 }, { 40, 30 } }) do
  riviera.footing.place{ point = p, level = basement, offset = -0.3 }
end
-- Partitions 3.0 m high under the girders: the mechanical plant (west), the laundry and storage
-- (middle), the electrical and water room (east); a chiller, a boiler and a water heater in the
-- plant room.
local bw = wall(basement, { 14, 0.15 }, { 14, 39.85 }, 3.0)
local be = wall(basement, { 46, 0.15 }, { 46, 39.85 }, 3.0)
local bm = wall(basement, { 14, 20 }, { 46, 20 }, 3.0)
riviera.door.place{ host_wall_id = bw, point_along = 12 }
riviera.door.place{ host_wall_id = be, point_along = 28 }
riviera.door.place{ host_wall_id = bm, point_along = 4 }
riviera.room.place{ level_id = basement, point = { 10, 20 }, name = "Mechanical plant", number = "B01" }
riviera.room.place{ level_id = basement, point = { 30, 10 }, name = "Laundry", number = "B02" }
riviera.room.place{ level_id = basement, point = { 30, 30 }, name = "Storage", number = "B03" }
riviera.room.place{ level_id = basement, point = { 53, 20 }, name = "Electrical and water", number = "B04" }
local plant = riviera.mep_type.load_starters{ names = { "Chiller", "Boiler", "Water Heater" } }
riviera.mech_equipment.place{ type_id = plant["Chiller"], point = { 7, 15 }, level = basement }
riviera.mech_equipment.place{ type_id = plant["Boiler"], point = { 7, 23 }, level = basement }
riviera.mech_equipment.place{ type_id = plant["Water Heater"], point = { 11.5, 20 }, level = basement }
-- Groundwater control (MEP-PIPE-011): a perimeter drain outside the retaining walls beside their
-- footings, a sump pit in the plant room's floor with duty and standby submersible pumps, and a
-- rising main up under the ground floor and out to the storm drain.
local sump = riviera.sump.create{ level = basement, point = { 11.5, 27 } }
print(string.format("Sump: %.0f m of drain, %.2f L/s (design %.2f L/s); a %.1f m pit %.1f m deep, %.2f m³ working; pumps %.2f kW at %.1f m", sump.drain_length, sump.inflow, sump.design_flow, sump.pit_size, sump.pit_depth, sump.working_volume, sump.motor, sump.head))

-- ---- The ground floor ----
-- Partitions 4.0 m high, under the girders.
local G = 4.0
-- The ballroom (west), the conference rooms (east), the restaurant (north), the lobby and atrium
-- court (south and middle), the front office and the administration either side of the lobby.
local ball = {
  s = wall(l1, { 0, 7.5 }, { 18, 7.5 }, G), n = wall(l1, { 0, 32.5 }, { 18, 32.5 }, G),
  e = wall(l1, { 18, 0 }, { 18, 40 }, G),
}
wall(l1, { 8, 0 }, { 8, 7.5 }, G)
wall(l1, { 52, 0 }, { 52, 7.5 }, G)
local conf_w = wall(l1, { 42, 0 }, { 42, 40 }, G)
wall(l1, { 42, 7.5 }, { 60, 7.5 }, G)
wall(l1, { 42, 15.8 }, { 60, 15.8 }, G)
wall(l1, { 42, 24.2 }, { 60, 24.2 }, G)
wall(l1, { 42, 32.5 }, { 60, 32.5 }, G)
local rest_s = wall(l1, { 18, 30.35 }, { 42, 30.35 }, G)
-- Doors: the lobby's entrance pair under the canopy, the ballroom's three, each conference
-- room's two, the restaurant's two into the atrium court.
for _, x in ipairs({ 28.0, 30.0 }) do riviera.door.place{ host_wall_id = facade[1].south, point_along = x } end
for _, a in ipairs({ 14, 20, 26 }) do riviera.door.place{ host_wall_id = ball.e, point_along = a } end
for _, a in ipairs({ 9.5, 14, 18, 22.5, 26.5, 31 }) do riviera.door.place{ host_wall_id = conf_w, point_along = a } end
for _, a in ipairs({ 6, 18 }) do riviera.door.place{ host_wall_id = rest_s, point_along = a } end
local function room(level, p, name, number) return riviera.room.place{ level_id = level, point = p, name = name, number = number } end
local ballroom = room(l1, { 9, 20 }, "Ballroom", "G01")
local conf = { room(l1, { 51, 11.5 }, "Conference A", "G02"), room(l1, { 51, 20 }, "Conference B", "G03"), room(l1, { 51, 28.4 }, "Conference C", "G04") }
local restaurant = room(l1, { 30, 35 }, "Restaurant", "G05")
room(l1, { 30, 5 }, "Lobby and atrium court", "G06")
room(l1, { 13, 4 }, "Front office", "G07")
room(l1, { 47, 4 }, "Administration", "G08")

-- Each room set out for its use, checked against the IBC's occupant load, exits and egress.
local function setup(r, style, more)
  local args = { room_id = r, style = style }
  for k, v in pairs(more or {}) do args[k] = v end
  local s = riviera.seating.layout(args)
  print(string.format("%s: %d tables, %d seats (%.1f ft² each); occupant load %d, %d exits needed, %d doors; %.0f in of %.0f in egress", style, s.tables, s.seats, s.per_seat, s.design_load, s.exits_needed, s.doors, s.width_needed, s.width))
  for _, f in ipairs(s.findings) do print("  " .. f.rule .. ": " .. f.message) end
  for _, f in ipairs(s.findings) do assert(f.severity ~= "error", f.rule .. ": " .. f.message) end
  return s
end
setup(ballroom, "banquet")
setup(conf[1], "theatre")
setup(conf[2], "classroom")
setup(conf[3], "boardroom")
setup(restaurant, "dining")

-- The drop-off: a steel porte-cochère 20 × 11 m in front of the lobby, its flat roof 4.3 m up
-- (14 ft clear for coaches) under the first floor's balconies, on two columns standing on the
-- forecourt 0.3 m below the ground floor, over the drive (the site, below).
local post = riviera.column.load_section{ catalog = "aisc_w", designation = "W10X49" }
for _, p in ipairs({ { 21, -10.5 }, { 39, -10.5 } }) do
  riviera.column.place{ type_id = post, point = p, base_level = l1, base_offset = -0.3, top_constraint = { unconnected = 4.3 } }
end
riviera.roof.create_footprint{ curves = { { line = { start = { 20, -11.5 }, ["end"] = { 40, -11.5 } }, defines_slope = false }, { line = { start = { 40, -11.5 }, ["end"] = { 40, -0.65 } }, defines_slope = false }, { line = { start = { 40, -0.65 }, ["end"] = { 20, -0.65 } }, defines_slope = false }, { line = { start = { 20, -0.65 }, ["end"] = { 20, -11.5 } }, defines_slope = false } }, level_id = l1, base_offset = 4.3 }

-- ---- Level 2: the gym (west wing) and the spa (east wing) ----
local l2 = floors[2]
wall(l2, { 0, 7.5 }, { 7.5, 7.5 }, 3.2)
wall(l2, { 0, 32.5 }, { 7.5, 32.5 }, 3.2)
local gym_e = wall(l2, { 7.5, 7.5 }, { 7.5, 32.5 }, 3.2)
riviera.door.place{ host_wall_id = gym_e, point_along = 12.5 }
local gym = room(l2, { 4, 20 }, "Gym", "201G")
local g = riviera.gym.layout{ room_id = gym, head = { 7.5, 20 }, pieces = {
  { item = "Treadmill, commercial", count = 4 }, { item = "Elliptical" }, { item = "Upright bike" }, { item = "Rower" },
} }
print(string.format("Gym: %d pieces in %d rows", #g.placed, g.rows))
wall(l2, { 52.5, 7.5 }, { 60, 7.5 }, 3.2)
wall(l2, { 52.5, 32.5 }, { 60, 32.5 }, 3.2)
local spa_w = wall(l2, { 52.5, 7.5 }, { 52.5, 32.5 }, 3.2)
riviera.door.place{ host_wall_id = spa_w, point_along = 12.5 }
room(l2, { 56, 12 }, "Spa", "201S")
-- A sauna and a steam room in the spa's north end, their doors opening out.
local function box(level, x, y, w, d, h)
  local c = { { x, y }, { x + w, y }, { x + w, y + d }, { x, y + d } }
  local ws = {}
  for i = 1, 4 do ws[i] = wall(level, c[i], c[i % 4 + 1], h) end
  return ws
end
local sauna = box(l2, 55, 28, 2.4, 2.0, 2.2)
riviera.door.place{ host_wall_id = sauna[1], point_along = 1.2, flip_facing = true }
riviera.bath.sauna{ point = { 56.2, 29.0 }, level = l2, bench_wall_id = sauna[3] }
local steam = box(l2, 55, 24.5, 2.4, 1.8, 2.4)
riviera.door.place{ host_wall_id = steam[1], point_along = 1.2, flip_facing = true }
riviera.bath.steam_room{ point = { 56.2, 25.4 }, level = l2, bench_wall_id = steam[3], material = "tile" }

-- ---- The guest floors: two room units ----
-- Corridor walls on each guest floor, 2.5 m corridors either side of the atrium's galleries.
for k = 2, 5 do
  wall(floors[k], { 8, 7.5 }, { 52, 7.5 }, 3.2)
  wall(floors[k], { 8, 32.5 }, { 52, 32.5 }, 3.2)
end
-- The room, drawn once off to the side on level 2: its corridor wall with the entrance door, its
-- façade wall with a window and a balcony door, two side walls, the room, a king bed, a ceiling
-- diffuser and a 3 × 1.5 m balcony with its railing.
local t = riviera.mep_type.load_starters{ names = { "Supply Diffuser 600" } }
local beds = riviera.furnishing.load{ names = { "Bed, king 76 × 80 in", "Desk 60 × 30 in", "Task chair" } }
local function guest_room(x0, accessible)
  local c = wall(l2, { x0, 100 }, { x0 + 4, 100 }, 3.2)
  local f = wall(l2, { x0, 107.5 }, { x0 + 4, 107.5 }, 3.2)
  local a = wall(l2, { x0, 100 }, { x0, 107.5 }, 3.2)
  local b = wall(l2, { x0 + 4, 100 }, { x0 + 4, 107.5 }, 3.2)
  riviera.door.place{ host_wall_id = c, point_along = accessible and 1.2 or 0.9 }
  riviera.window.place{ host_wall_id = f, point_along = 1.0 }
  riviera.door.place{ host_wall_id = f, point_along = 2.9 }
  local r = riviera.room.place{ level_id = l2, point = { x0 + 2, 104 }, name = accessible and "Accessible king" or "King" }
  local d = riviera.air_terminal.place{ type_id = t["Supply Diffuser 600"], point = { x0 + 2.5, 102.5 }, level = l2, offset = 2.3, flow = 0.06 }.id
  local bed = riviera.mesh_object.place{ type_id = beds["Bed, king 76 × 80 in"], point = { x0 + 2, 105.2 }, level = l2, rotation = math.pi }
  local desk = riviera.mesh_object.place{ type_id = beds["Desk 60 × 30 in"], point = { x0 + 3.3, 102.2 }, level = l2, rotation = math.rad(90) }
  local balcony = riviera.floor.create{ loops = { rect(x0 + 0.5, 107.6, x0 + 3.5, 109.1) }, level_id = l2 }
  local rail = riviera.railing.create{ path = { { x0 + 0.55, 107.65 }, { x0 + 0.55, 109.05 }, { x0 + 3.45, 109.05 }, { x0 + 3.45, 107.65 } }, base_level = l2 }
  return { c, f, a, b, r, d, bed, desk, balcony, rail }
end
local king = riviera.unit.create_type{ element_ids = guest_room(100, false), name = "King room", origin = { 100, 100 }, width = 4.0, depth = 7.5 }
riviera.unit.create_type{ element_ids = guest_room(110, true), name = "Accessible king", origin = { 110, 100 }, width = 4.0, depth = 7.5 }

-- Down the south corridor (rooms to the south, facing the street) and the north corridor
-- (facing the garden), on levels 2 to 5, from 8 m to 52 m between the corner cores: eleven a
-- side, mirrored in pairs, every eighth accessible. Level 2's wings hold the gym and the spa.
local guest_levels = { floors[2], floors[3], floors[4], floors[5] }
local south = riviera.unit.array{ unit_type = "King room", path = { { 0, 8.75 }, { 60, 8.75 } }, levels = guest_levels, sides = "right", corridor = 2.5, start = 8, exclusions = { { 0, 8 }, { 52, 60 } },
  variants = { { unit_type = "Accessible king", every = 8 } }, name = "South rooms" }
local north = riviera.unit.array{ unit_type = "King room", path = { { 0, 31.25 }, { 60, 31.25 } }, levels = guest_levels, sides = "left", corridor = 2.5, start = 8, exclusions = { { 0, 8 }, { 52, 60 } },
  variants = { { unit_type = "Accessible king", every = 8 } }, numbering = { start = 51 }, name = "North rooms" }
for _, a in ipairs({ south, north }) do
  local types = {}
  for k, n in pairs(a.per_type) do types[#types + 1] = n .. " " .. k end
  print(string.format("%d rooms: %s; %d faces found nothing to merge with", a.copies, table.concat(types, ", "), #a.unmerged))
  for _, u in ipairs(a.unmerged) do print("  " .. u) end
  assert(a.copies == 44 and #a.unmerged == 0, "eleven rooms a side on four floors, every wall merged")
end

-- Supply air: a main down each corridor on every guest floor, 2.9 m up under the slab, and every room's
-- diffuser connected into its corridor's main.
local mains = {}
for _, l in ipairs(guest_levels) do
  for _, y in ipairs({ 8.75, 31.25 }) do
    mains[#mains + 1] = riviera.duct.create{ points = { { 8.5, y, 2.9 }, { 51.5, y, 2.9 } }, level = l, shape = "round", diameter = 0.4, system_type_id = "supply_air" }.segments[1]
  end
end
for _, a in ipairs({ south, north }) do
  local c = riviera.unit.connect{ array_id = a.array_id, spines = mains }
  print(string.format("%d branches; %d not connected", c.branches, #c.unconnected))
  for _, u in ipairs(c.unconnected) do print("  " .. u.number .. ": " .. u.reason) end
  assert(c.branches == 44 and #c.unconnected == 0, "every room's diffuser is on its corridor main")
end

-- ---- The roof: the pool podium, the patio and the baffles ----
-- The podium, 15 × 18 m, its deck 1.8 m above the terrace, a stair up to it, and the pool
-- fence round its edge with a self-closing gate at the stair.
riviera.floor.create{ loops = { rect(43, 11, 58, 29) }, level_id = podium }
local pool = riviera.pool.create{ level = podium, name = "Roof pool", outline = { { 45, 22 }, { 45, 16 }, { 57, 16 }, { 57, 22 } },
  profile = { { at = 0, depth = 1.2 }, { at = 12, depth = 1.35 } } }
riviera.pool.set{ pool_id = pool, shell = { kind = "cast_concrete", thickness = 0.25, tile = 0.012, membrane = true } }
riviera.pool.add_entry{ pool_id = pool, kind = "stairs", edge = 1, at = 3.0 }
riviera.pool.depth_markings{ pool_id = pool }
riviera.stair.create{ base_level = roof, width = 1.4, runs = { { start = { 44.4, 32.4 }, ["end"] = { 44.4, 29.3 } } } }
local fence_type = riviera.railing_type.create{ name = "Pool fence 4 ft", height = 1.25, bottom_rail_height = 0.05, baluster = 0.025, baluster_spacing = 0.12 }
local fence = riviera.railing.create{ type_id = fence_type, path = { { 43.1, 11.1 }, { 57.9, 11.1 }, { 57.9, 28.9 }, { 43.1, 28.9 } }, closed = true, base_level = podium }
local gate = riviera.railing.add_gate{ railing_id = fence, point = { 44.5, 28.9 }, swing_out = true }
riviera.railing.gate_hardware{ gate_id = gate, latch_height = 1.3716 }

-- Its weight on the roof's structure: the pool's loads, and the roof slab under it as an
-- analytical panel carrying them.
local s = riviera.pool.structure{ pool_id = pool }[1]
print(string.format("Roof pool: water %.0f kN, shell %.0f kN; %.1f kN/m² full", s.water_weight, s.shell_weight, s.full))
local panel = riviera.analytical_panel.create_by_boundary{
  boundary = { { 43, 11, 19.0 }, { 58, 11, 19.0 }, { 58, 29, 19.0 }, { 43, 29, 19.0 } }, thickness = 0.3, material = "Concrete f'c 4000 psi" }
local dead = riviera.load_case.create{ name = "D", nature = "D", self_weight = true }
riviera.load_nature.create{ name = "Fluid", symbol = "F", action_type = "permanent", action_source = "USERDEFINED" }
local fluid = riviera.load_case.create{ name = "F", nature = "F" }
local loads = riviera.pool.apply_loads{ pool_id = pool, panel_id = panel, dead_case_id = dead, water_case_id = fluid }
print(string.format("On the roof panel: %.1f kN/m² dead and %.1f kN/m² of water", loads.dead_pressure / 1000, loads.water_pressure / 1000))

-- Sun control: tilted aluminium baffles 0.25 m apart under the whole glazed roof, and the direct
-- sun they let onto the pool and the patio at Miami's latitude.
riviera.location.set{ lat = 25.8, lon = -80.2, time_zone = -5.0, place = "Miami" }
riviera.shading.baffles{ outline = { { 18.5, 10.5 }, { 58.3, 10.5 }, { 58.3, 37.5 }, { 18.5, 37.5 } }, level = roof, height = 5.6, spacing = 0.25, depth = 0.5, tilt = "45°", direction = "90°", name = "Roof baffles" }
local sun = riviera.shading.sun_check{ elements = { pool }, areas = { { name = "Roof patio", outline = { { 30, 30.5 }, { 42, 30.5 }, { 42, 37.5 }, { 30, 37.5 } }, z = 19.0 } } }
for _, a in ipairs(sun.areas) do
  print(string.format("%s: %.0f %% of the direct sun (the most %.0f %%)", a.name, a.share * 100, a.peak * 100))
  assert(a.passes, a.name .. " gets more than a quarter of the direct sun")
end

-- The roof terrace's guard round its edge.
riviera.railing.create{ path = { { 0.15, 0.15 }, { 59.85, 0.15 }, { 59.85, 39.85 }, { 0.15, 39.85 } }, closed = true, base_level = roof }

-- Plant on the open roof west of the glazing: two air handling units for the guest floors.
local ahu = riviera.mep_type.load_starters{ names = { "Air Handling Unit" } }
for _, y in ipairs({ 16, 24 }) do
  riviera.mech_equipment.place{ type_id = ahu["Air Handling Unit"], point = { 9, y }, level = roof }
end
-- Solar panels on the rest of the open roof (MEP-ELEC-022): 400 W modules tilted 10° to the south,
-- 4 ft from the edge, 1.2 m clear of the glazed roof, the air handling units and the lift
-- overruns, rows spaced clear of each other's winter shadow.
local pv = riviera.pv.array{ level = roof, name = "Roof PV" }
print(string.format("PV: %d modules in %d rows, %.1f kWp, kept clear of %d things on the roof", pv.modules, pv.rows, pv.kwp, #pv.kept_clear_of))
assert(pv.modules > 50, "the open roof takes the array")

-- ---- The site ----
-- The ground round the building, in four strips round its footprint (the building and its lift
-- pits stand in the hole): level with the underside of the ground floor's slab, 0.3 m below it,
-- for 16 m in front, falling 0.7 m to the street on the south.
local function ground(b, pts)
  return riviera.toposolid.create{ level_id = l1, boundary = b, points = pts }
end
local function box2(x0, y0, x1, y1) return { { x0, y0 }, { x1, y0 }, { x1, y1 }, { x0, y1 } } end
local south = ground(box2(-40, -50, 100, -1), { { -40, -1, -0.3 }, { 100, -1, -0.3 }, { -40, -16, -0.3 }, { 100, -16, -0.3 }, { -40, -50, -1.0 }, { 100, -50, -1.0 } })
local north = ground(box2(-40, 41, 100, 60), { { -40, 41, -0.3 }, { 100, 41, -0.3 }, { -40, 60, -0.3 }, { 100, 60, -0.3 } })
local west = ground(box2(-40, -1, -1, 41), { { -40, -1, -0.3 }, { -1, -1, -0.3 }, { -40, 41, -0.3 }, { -1, 41, -0.3 } })
local east = ground(box2(61, -1, 100, 41), { { 61, -1, -0.3 }, { 100, -1, -0.3 }, { 61, 41, -0.3 }, { 100, 41, -0.3 } })

-- The drive: an alignment from the street, north to the lobby, east under the porte-cochère and
-- back south to the street, with 10 m curves; its 7 m surface a sub-division of the ground
-- 50 mm up, traced from the alignment's offsets.
local drive = riviera.alignment.create{ level = l1, name = "Hotel drive", points = { { 6, -49.5 }, { 6, -6 }, { 54, -6 }, { 54, -49.5 } }, radii = { 10, 10 } }
local left, right = {}, {}
local st = 0
while st <= drive.length + 1e-9 do
  left[#left + 1] = riviera.alignment.point_at{ alignment_id = drive.alignment, station = st, offset = 3.5 }.point
  right[#right + 1] = riviera.alignment.point_at{ alignment_id = drive.alignment, station = st, offset = -3.5 }.point
  st = st + math.min(2.0, drive.length - st > 1e-9 and drive.length - st or 2.0)
  if st > drive.length then st = drive.length + 1 end
end
local surface = {}
for _, p in ipairs(left) do surface[#surface + 1] = { p[1], p[2] } end
for k = #right, 1, -1 do surface[#surface + 1] = { right[k][1], right[k][2] } end
riviera.toposolid.subdivide{ toposolid_id = south, region = surface, offset = 0.05 }
print(string.format("Drive: %.1f m, %d curves", drive.length, #riviera.alignment.list()[1].curves))
-- A car park east of the drive, a lawn north of the building, trees either side of it.
riviera.toposolid.subdivide{ toposolid_id = south, region = box2(64, -45, 95, -20), offset = 0.05 }
riviera.toposolid.subdivide{ toposolid_id = north, region = box2(-30, 45, 90, 57), offset = 0.05 }
local tree = riviera.mep_type.create{ name = "Tree 6 m", category = "planting", size = { 4.0, 4.0, 6.0 }, color = { 0.25, 0.50, 0.22, 1.0 } }
for _, y in ipairs({ 2, 14, 26, 38 }) do
  riviera.sitecomponent.place{ type_id = tree, point = { -20, y }, host_id = west, level = l1 }
  riviera.sitecomponent.place{ type_id = tree, point = { 80, y }, host_id = east, level = l1 }
end
-- The property line.
riviera.propertyline.create{ origin = { -40, -50 }, table = {
  { distance = 140.0, bearing = "N 90°E" }, { distance = 110.0, bearing = "N 0°E" },
  { distance = 140.0, bearing = "S 90°W" }, { distance = 110.0, bearing = "S 0°E" },
} }

print(string.format("%d guest rooms", #riviera.unit.list{}))
