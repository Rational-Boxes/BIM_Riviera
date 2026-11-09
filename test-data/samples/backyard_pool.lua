-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A backyard pool
-- description: A 10 × 5 m residential pool in a 16 × 12 m patio: 1.0 m deep at its shallow end, falling 1:10 to 1.4 m, 1:8 to 5 ft and then to 2.0 m, with corner stairs, an underwater bench, a ladder at the deep end, two main drains and an underwater light; checked against the ISPSC's residential rules.
--
-- Riviera sample model (test-data/samples): a residential pool (ARC-POOL-001…004). A pool's class
-- decides its rules: a residential pool is held to the ISPSC's residential chapter (a 33–48 in
-- shallow end, 1:7 to the transition, 10 in stair treads), not the public-pool rules.

local l1 = riviera.level.list()[1].id

local function rect(x0, y0, x1, y1)
  local p = { { x0, y0 }, { x1, y0 }, { x1, y1 }, { x0, y1 } }
  local out = {}
  for i = 1, 4 do out[i] = { line = { start = p[i], ["end"] = p[i % 4 + 1] } } end
  return out
end
-- The patio, its top at the level.
riviera.floor.create{ loops = { rect(0, 0, 16, 12) }, level_id = l1 }

-- 10 × 5 m, its shallow end the west wall (edge 1): 1.0 m (39 in), 1.4 m at 4 m, 5 ft at 5 m
-- (1:8, inside the ISPSC's 1:7), then 2.0 m at 7 m.
local pool = riviera.pool.create{ level = l1, name = "Backyard pool", class = "residential",
  outline = { { 3, 8.5 }, { 3, 3.5 }, { 13, 3.5 }, { 13, 8.5 } },
  profile = { { at = 0, depth = 1.0 }, { at = 4, depth = 1.4 }, { at = 5, depth = 1.524 }, { at = 7, depth = 2.0 } } }

-- Stairs in the shallow end's north corner, and a bench along the south wall.
local s = riviera.pool.add_entry{ pool_id = pool, kind = "stairs", edge = 1, at = 1.0, width = 1.8 }
local b = riviera.pool.add_entry{ pool_id = pool, kind = "bench", edge = 2, at = 2.5 }
print(string.format("Stairs: %d risers of %.0f mm on %.0f mm treads; bench seat %.0f mm below the water", s.risers, s.riser * 1000, s.tread * 1000, b.seat * 1000))

-- From the Riviera Pools library: a ladder on the deep end's north wall, two main drains 1.5 m
-- apart on the deep floor, and a light in the deep end's east wall.
local t = riviera.furnishing.load{ names = { "Pool ladder, 3 steps", "Main drain cover 300 × 300", "Underwater light, large niche" } }
local function place(name, x, y, turn, offset)
  return riviera.mesh_object.place{ type_id = t[name], point = { x, y }, level = l1, rotation = turn, offset = offset or 0 }
end
place("Pool ladder, 3 steps", 11.5, 8.5, 0)
place("Main drain cover 300 × 300", 11.5, 5.25, 0, -2.15)
place("Main drain cover 300 × 300", 11.5, 6.75, 0, -2.15)
place("Underwater light, large niche", 13, 6, -math.pi / 2)

local i = riviera.pool.info{ pool_id = pool }
print(string.format("%.1f m² of water, %.1f m³ (less the stairs and bench), %.2f–%.2f m deep", i.area, i.volume, i.shallowest, i.deepest))
local c = riviera.pool.check{}
for _, f in ipairs(c.findings) do print(f.severity, f.rule, f.message) end
for _, f in ipairs(c.findings) do assert(f.severity ~= "error", f.rule .. ": " .. f.message) end
assert(#riviera.overlap.find{}.overlaps == 0, "nothing overlaps")
