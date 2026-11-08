-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A fitness room
-- description: A fitness suite. A fitness room under a 3 m ceiling: a row of treadmills with their side and rear zones, bikes, an elliptical, a stair climber and a rower, and a strength area with a cable crossover, a power rack, a leg press, a bench and a dumbbell rack, a mirror on the strength wall and a ceiling fan; one of each type accessible with its clear floor space (ADA §236.1), every use zone clear. A cycling studio beside it filled with bikes facing the instructor, and a circuit room laid out in rows by gym.layout.
--
-- Riviera sample model (test-data/samples): gyms (ARC-GYM-001). The equipment comes from the
-- Riviera Gym Equipment library; the gym_equipment pack's use zones are checked by rules.check;
-- gym.mirror places a wall mirror (IfcCovering MIRROR); gym.studio sizes a studio and places its
-- bikes; gym.layout fills a room in rows (ARC-GYM-002); gym.check checks the types, the ceiling
-- over each piece, the mirrors and each room's occupant load.

local l1 = riviera.level.list()[1].id
riviera.clearance.set_rule_set{ standard = "ada_2010" }

-- A 26 × 17 m suite, its walls 4 m tall: an 18 × 10 m fitness room in its south-west corner
-- under a 3 m ceiling, a cycling studio east of it and a circuit room along the north. The
-- partitions stop at the other walls' faces (0.1 m off their lines).
local function wall(a, b)
  return riviera.wall.create{ curve = { line = { start = a, ["end"] = b } }, base_level = l1, top_constraint = { unconnected = 4.0 } }
end
wall({ 0, 0 }, { 26, 0 }); wall({ 26, 0 }, { 26, 17 }); wall({ 26, 17 }, { 0, 17 }); wall({ 0, 17 }, { 0, 0 })
wall({ 0.1, 10 }, { 25.9, 10 }); wall({ 18, 0.1 }, { 18, 9.9 })
riviera.room.place{ level_id = l1, point = { 9, 5 }, name = "Fitness", number = "1" }
riviera.ceiling.create_auto{ level_id = l1, point = { 9, 5 }, offset = 3.0 }

local names = { "Treadmill, commercial", "Upright bike", "Recumbent bike", "Elliptical", "Stair climber", "Rower",
  "Cable crossover", "Power rack", "Leg press, selectorised", "Flat bench", "Dumbbell rack, two-tier", "Ceiling fan 52 in" }
local t = riviera.furnishing.load{ names = names }
local function put(name, x, y, rotation, offset)
  return riviera.mesh_object.place{ type_id = t[name], point = { x, y }, level = l1, rotation = rotation or 0, offset = offset or 0 }
end

-- Cardio: four treadmills along the north wall, 2 m apart, stepped on from the south with 2 m
-- clear behind; a row of bikes, an elliptical, a stair climber and a rower to the south.
for k = 0, 3 do put("Treadmill, commercial", 1.5 + 2 * k, 8.6) end
put("Upright bike", 1.2, 2.5); put("Recumbent bike", 2.7, 2.5); put("Elliptical", 4.5, 2.5)
put("Stair climber", 6.3, 2.5); put("Rower", 8.2, 2.5)
-- Strength: a cable crossover by the south wall, a power rack, a leg press, a bench, and a
-- dumbbell rack against the north wall facing south; 0.9 m round each station.
put("Cable crossover", 13.5, 2.0); put("Power rack", 12.0, 6.0); put("Leg press, selectorised", 15.5, 7.0)
put("Flat bench", 11.0, 4.0); put("Dumbbell rack, two-tier", 12.0, 9.3)
-- A ceiling fan over the aisle between the cardio rows, its canopy just under the ceiling.
put("Ceiling fan 52 in", 4.5, 4.5, 0, 2.5)

-- A mirror on the strength area's east wall, facing west.
riviera.gym.mirror{ start = { 17.8, 4.5 }, ["end"] = { 17.8, 9.0 }, level = l1, toward = { 12, 6 } }

-- A cycling studio to the east, 8 × 10 m: its bikes in rows facing the instructor's east wall.
local studio_room = riviera.room.place{ level_id = l1, point = { 22, 5 }, name = "Cycle studio", number = "2" }
local studio = riviera.gym.studio{ room_id = studio_room, kind = "cycle", head = { 26, 5 } }
print(string.format("cycle studio: %.0f m², about %d places, %d bikes placed", studio.area, studio.capacity, #studio.placed))

-- A circuit room along the north, 26 × 7 m, laid out in rows from its north wall.
local circuit_room = riviera.room.place{ level_id = l1, point = { 13, 13.5 }, name = "Circuit", number = "3" }
local circuit = riviera.gym.layout{ room_id = circuit_room, head = { 13, 17 }, pieces = {
  { item = "Treadmill, commercial", count = 3 }, { item = "Recumbent bike", count = 2 }, { item = "Rower" },
  { item = "Leg press, selectorised" }, { item = "Smith machine" } } }
assert(next(circuit.unplaced) == nil, "every circuit piece fits")
print(string.format("circuit room: %d pieces in %d rows", #circuit.placed, circuit.rows))

local c = riviera.gym.check{}
for _, r in ipairs(c.rooms) do
  print(string.format("%s: %.0f m², %d pieces, %d occupants (50 ft² gross)", r.name, r.area, r.pieces, r.occupants))
end
for _, ty in ipairs(c.types) do
  print(string.format("%s: %d units, %d accessible with a clear space", ty.kind, ty.units, ty.with_space))
end
assert(#c.findings == 0, "one of each type is accessible, the ceiling is high enough, no mirror faces a treadmill")
assert(#riviera.rules.check{} == 0, "every use zone is clear")
assert(#riviera.overlap.find{}.overlaps == 0, "nothing overlaps")
