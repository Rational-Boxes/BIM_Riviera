-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A swimming pool hall
-- description: A pool hall's deck with a six-lane 25 m competition pool, 2.0 m deep throughout, with starting platforms, ladders, a pool lift, a lifeguard chair and main drains from the Riviera Pools library, and a 12 × 7 m teaching pool beside it whose floor falls from 0.6 m to 0.9 m, with stairs and an ADA sloped entry built in; each pool's shell walls, base slab, coping and water are generated, and the deck is cut round them.
--
-- Riviera sample model (test-data/samples): swimming pools (ARC-POOL-001…004). pool.create makes a
-- pool from a template or an outline with its depth profile; pool.info gives its water area,
-- volume, depths and slopes; in IFC the pool is an IfcElementAssembly of walls, a base slab and
-- coping, with its water an IfcSpace (WATER).

local l1 = riviera.level.list()[1].id

-- The hall's deck, 0.3 m thick, its top at the level.
local function rect(x0, y0, x1, y1)
  local p = { { x0, y0 }, { x1, y0 }, { x1, y1 }, { x0, y1 } }
  local out = {}
  for i = 1, 4 do out[i] = { line = { start = p[i], ["end"] = p[i % 4 + 1] } } end
  return out
end
riviera.floor.create{ loops = { rect(0, 0, 44, 30) }, level_id = l1 }

-- A 25 m pool, six 2.5 m lanes, its long axis east–west.
local main = riviera.pool.create{ level = l1, name = "Main pool", template = { kind = "competition_25m", origin = { 16, 12 } } }
-- A teaching pool to the east, 0.6 m deep at its west end falling to 0.9 m.
local teach = riviera.pool.create{ level = l1, name = "Teaching pool", template = { kind = "teaching", origin = { 36, 20 } },
  profile = { { at = 0, depth = 0.6 }, { at = 12, depth = 0.9 } } }

for _, p in ipairs(riviera.pool.list{}) do
  local i = riviera.pool.info{ pool_id = p.id }
  local slope = i.segments[1] and i.segments[1].ratio or "level"
  print(string.format("%s: %.1f m² of water, %.1f m³, %.2f–%.2f m deep, floor %s", p.name, i.area, i.volume, i.shallowest, i.deepest, slope))
end
local m = riviera.pool.info{ pool_id = main }
assert(math.abs(m.volume - 25.03 * 15 * 2.0) < 1, "the main pool holds about 751 m³")

-- Equipment from the Riviera Pools library, each facing the water at its front.
local t = riviera.furnishing.load{ names = { "Starting platform 24 × 32 in", "Pool ladder, 3 steps", "Pool lift, 300 lb", "Lifeguard chair 6 ft", "Main drain cover 300 × 300" } }
local function place(name, x, y, turn, offset)
  return riviera.mesh_object.place{ type_id = t[name], point = { x, y }, level = l1, rotation = turn, offset = offset or 0 }
end
-- A starting platform at the east end of each of the six lanes (lane centres 1.25 m in from
-- the side walls, 2.5 m apart), facing west; the touch pads' 30 mm is in the length.
for k = 0, 5 do
  place("Starting platform 24 × 32 in", 28.515 + 0.406, 4.5 + 1.25 + 2.5 * k, -math.pi / 2)
end
-- Ladders at each end of both long sides, on the walls' faces.
place("Pool ladder, 3 steps", 5, 19.5, 0)
place("Pool ladder, 3 steps", 27, 19.5, 0)
place("Pool ladder, 3 steps", 5, 4.5, math.pi)
place("Pool ladder, 3 steps", 27, 4.5, math.pi)
-- A pool lift on the south deck and a lifeguard chair on the north deck, both facing the pool.
place("Pool lift, 300 lb", 8, 3.6, math.pi)
place("Lifeguard chair 6 ft", 16, 21.6, 0)
-- Two main drains on the floor, 2 m apart (the VGB Act's 3 ft at least), 2.15 m down.
place("Main drain cover 300 × 300", 16, 11, 0, -2.15)
place("Main drain cover 300 × 300", 16, 13, 0, -2.15)

-- The teaching pool's entries, built into its west wall (edge 1, from its north-west corner):
-- stairs, and a sloped entry falling 1:12 to 27 in below the water; and stairs at its east end
-- (edge 3), as the ISPSC wants an entry at each end and does not count the sloped entry.
local s = riviera.pool.add_entry{ pool_id = teach, kind = "stairs", edge = 1, at = 5 }
local r = riviera.pool.add_entry{ pool_id = teach, kind = "sloped_entry", edge = 1, at = 2 }
riviera.pool.add_entry{ pool_id = teach, kind = "stairs", edge = 3, at = 3.5 }
print(string.format("Teaching pool: stairs of %d risers of %.0f mm; a sloped entry %.2f m long", s.risers, s.riser * 1000, r.run))

for _, p in ipairs(riviera.pool.check{}.pools) do
  print(string.format("%s (%s): %d entries, %d accessible, %d drains", p.name, p.class, p.entries, p.accessible_entries, p.drains))
end
assert(#riviera.overlap.find{}.overlaps == 0, "the equipment stands clear of the pools' shells")
