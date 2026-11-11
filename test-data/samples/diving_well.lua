-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A diving well
-- description: A 14 × 10 m diving well, 3.8 m deep for 8 m from its west wall, with two 1 m springboards 2.8 m apart, ladders at the east end and two main drains; each board's plummet checked against the Model Aquatic Health Code's Table 4.8.2.3 for distances, depths and clear overhead.
--
-- Riviera sample model (test-data/samples): diving envelopes (ARC-POOL-006). pool.check finds each
-- diving board at a pool, takes its plummet (the tip's middle) and its height above the water, and
-- checks the table's column for it: A back to the wall, B to the side walls, C to the next
-- plummet, D to the wall ahead, E clear overhead, and the depths H, K and M.

local l1 = riviera.level.list()[1].id
riviera.rules.set_packs{ domain = "pool", packs = { "us_mahc_2024", "us_vgb" } }

local function rect(x0, y0, x1, y1)
  local p = { { x0, y0 }, { x1, y0 }, { x1, y1 }, { x0, y1 } }
  local out = {}
  for i = 1, 4 do out[i] = { line = { start = p[i], ["end"] = p[i % 4 + 1] } } end
  return out
end
riviera.floor.create{ loops = { rect(-6, -5, 20, 15) }, level_id = l1 }

-- 14 × 10 m, its west wall (edge 1) under the boards: 3.8 m deep to 8 m out, rising to 2.5 m.
local pool = riviera.pool.create{ level = l1, name = "Diving well", outline = { { 0, 10 }, { 0, 0 }, { 14, 0 }, { 14, 10 } },
  profile = { { at = 0, depth = 3.8 }, { at = 8, depth = 3.8 }, { at = 14, depth = 2.5 } } }
riviera.pool.depth_markings{ pool_id = pool }

local t = riviera.furnishing.load{ names = { "Diving board 1 m, on stand", "Pool ladder, 3 steps", "Main drain cover 300 × 300" } }
local function place(name, x, y, turn, offset)
  return riviera.mesh_object.place{ type_id = t[name], point = { x, y }, level = l1, rotation = turn, offset = offset or 0 }
end
-- Two 1 m boards on the west deck pointing east, their tips 1.9 m over the water (A: 6 ft), 3.6 m
-- from the side walls (B: 10 ft) and 2.8 m apart (C: 8.83 ft). A board is 16 ft long, its tip
-- 2.44 m ahead of its middle.
for _, y in ipairs({ 3.6, 6.4 }) do
  place("Diving board 1 m, on stand", 1.9 - 2.4384, y, math.pi / 2)
end
-- Ladders at the east end, away from the boards; drains on the deep floor 1.5 m apart.
place("Pool ladder, 3 steps", 13, 10, 0)
place("Pool ladder, 3 steps", 13, 0, math.pi)
place("Main drain cover 300 × 300", 7, 4.25, 0, -3.95)
place("Main drain cover 300 × 300", 7, 5.75, 0, -3.95)

local c = riviera.pool.check{}
for _, f in ipairs(c.findings) do print(f.severity, f.rule, f.message) end
for _, f in ipairs(c.findings) do assert(f.severity ~= "error", f.rule .. ": " .. f.message) end
assert(#riviera.overlap.find{}.overlaps == 0, "nothing overlaps")
