-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A swimming pool hall
-- description: A pool hall's deck with a six-lane 25 m competition pool, 2.0 m deep throughout, and a 12 × 7 m teaching pool beside it whose floor falls from 0.6 m to 0.9 m; each pool's shell walls, base slab, coping and water are generated, and the deck is cut round them.
--
-- Riviera sample model (test-data/samples): swimming pools (ARC-POOL-001). pool.create makes a
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
assert(#riviera.overlap.find{}.overlaps == 0, "the deck is cut round the pools")
