-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: Store aisles
-- description: A 14 × 12 m sales floor with a wall run of gondolas and three island runs with end caps, 2 m apart: wide enough for the 2010 ADA Standards' 36 in route and 48 in turn (§403.5), the IBC's Group M aisle and two carts passing.
--
-- Riviera sample model (test-data/samples): gondola runs (ARC-RETAIL-001). retail.run fills a
-- run with 48 in sections and closes an island with end caps; the interiors and retail rule
-- packs keep the aisles in front of them clear.

local l1 = riviera.level.list()[1].id
riviera.code.set{ code = "ibc" }
riviera.clearance.set_rule_set{ standard = "ada_2010" }
riviera.rules.set_packs{ domain = "retail", packs = { "us_ibc_mercantile", "practice_retail" } }

local function wall(a, b)
  return riviera.wall.create{ curve = { line = { start = a, ["end"] = b } }, base_level = l1, top_constraint = { unconnected = 4.0 } }
end
-- The sales floor's faces at x = 0 and 14, y = 0 and 12.
wall({ -0.1, -0.1 }, { 14.1, -0.1 })
wall({ 14.1, -0.1 }, { 14.1, 12.1 })
wall({ 14.1, 12.1 }, { -0.1, 12.1 })
wall({ -0.1, 12.1 }, { -0.1, -0.1 })

local IN = 0.0254
-- Six 48 in sections along the north wall, facing into the store.
local w = riviera.retail.run{ start = { 1.0, 12.0 }, ["end"] = { 1.0 + 6 * 48 * IN, 12.0 }, level = l1, kind = "wall" }
-- Islands of four 48 in sections and two 18.5 in end caps (229 in), 2 m apart face to face.
local depth_wall, half_island, aisle = 18.5 * IN, 17.25 * IN, 2.0
local y = 12.0 - depth_wall - aisle - half_island
for _ = 1, 3 do
  local r = riviera.retail.run{ start = { 2.0, y }, ["end"] = { 2.0 + 229 * IN, y }, level = l1, kind = "island" }
  print(string.format("island at y = %.3f: %d fixtures, %.0f mm deep", y, #r.sections, r.depth * 1000))
  y = y - 2 * half_island - aisle
end
print(string.format("wall run: %d sections", #w.sections))

assert(#riviera.rules.check{} == 0, "every aisle is wide enough")
assert(#riviera.overlap.find{}.overlaps == 0, "nothing overlaps")
