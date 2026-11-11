-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A community pool in England
-- description: A 25 × 12.5 m community pool with a 2 m surround: Sport England's typical profile (1.0 m, 1 in 17 to 1.35 m, on to 2.0 m), six 2.0 m lanes, shallow-end steps with 0.14 m risers, a pool hoist, ladders and main drains; checked against Sport England's guidance and HSE's HSG179.
--
-- Riviera sample model (test-data/samples): the UK pool packs (ARC-POOL-007). Under the UK code
-- setting the pool packs are Sport England's design guidance and HSE's HSG179, whose findings
-- are notes.

local l1 = riviera.level.list()[1].id
riviera.code.set{ code = "uk_adk", occupancy = "other" }

local function rect(x0, y0, x1, y1)
  local p = { { x0, y0 }, { x1, y0 }, { x1, y1 }, { x0, y1 } }
  local out = {}
  for i = 1, 4 do out[i] = { line = { start = p[i], ["end"] = p[i % 4 + 1] } } end
  return out
end
-- The pool hall's floor: the ASA's 2 m surround at the sides and ends.
riviera.floor.create{ loops = { rect(-2, -2, 27, 14.5) }, level_id = l1 }

-- 25 × 12.5 m, its shallow end the west wall: 1.0 m, 1 in 17 to 1.35 m at 6 m, 1.8 m at 15 m,
-- 2.0 m at the deep end.
local pool = riviera.pool.create{ level = l1, name = "Community pool", outline = { { 0, 12.5 }, { 0, 0 }, { 25, 0 }, { 25, 12.5 } },
  profile = { { at = 0, depth = 1.0 }, { at = 6, depth = 1.35 }, { at = 15, depth = 1.8 }, { at = 25, depth = 2.0 } } }
-- Six 2.0 m lanes with 0.25 m at the side walls; no touch pads.
local lanes = riviera.pool.lanes{ pool_id = pool, count = 6, width = 2.0, gap = 0.25, pads = 0 }
-- Depth markings in metres (the UK code setting's default), readable from the surround and the
-- water (HSG179).
riviera.pool.depth_markings{ pool_id = pool }
-- Steps at the shallow end: Sport England's 0.14 m risers and 0.30 m goings.
local s = riviera.pool.add_entry{ pool_id = pool, kind = "stairs", edge = 1, at = 2, riser = 0.14, tread = 0.3 }
print(string.format("%d lanes; steps of %d risers of %.0f mm", #lanes.widths, s.risers, s.riser * 1000))

-- A pool hoist on the south side at the shallow end; ladders 1 m from the deep end's wall; two
-- drains between the lines of lanes 3 and 4, set 5 mm above the floor at their middles, as it
-- falls 1 in 50 under them.
local t = riviera.furnishing.load{ names = { "Pool lift, 300 lb", "Pool ladder, 3 steps", "Main drain cover 300 × 300" } }
local function place(name, x, y, turn, offset)
  return riviera.mesh_object.place{ type_id = t[name], point = { x, y }, level = l1, rotation = turn, offset = offset or 0 }
end
place("Pool lift, 300 lb", 3, -0.6, math.pi)
place("Pool ladder, 3 steps", 24, 12.5, 0)
place("Pool ladder, 3 steps", 24, 0, math.pi)
place("Main drain cover 300 × 300", 21, 6.25, 0, -2.065)
place("Main drain cover 300 × 300", 23, 6.25, 0, -2.105)

local c = riviera.pool.check{}
for _, l in ipairs(c.pools[1].loads) do print(string.format("%s: %d bathers", l.rule, l.bathers)) end
for _, f in ipairs(c.findings) do print(f.severity, f.rule, f.message) end
assert(#c.findings == 0, "the pool meets Sport England's guidance and HSG179")
assert(#riviera.overlap.find{}.overlaps == 0, "nothing overlaps")
