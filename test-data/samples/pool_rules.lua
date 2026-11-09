-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A public pool checked against the codes
-- description: A 20 × 10 m public pool, 0.9 m deep falling to 5 ft at 1:12.8 and to 2.5 m at 1:3.1, with ladders on both sides, a pool lift and two main drains 2 m apart; checked against the CDC's Model Aquatic Health Code, the VGB Act's drain rules and the ADA's pool rules.
--
-- Riviera sample model (test-data/samples): swimming-pool checks (ARC-POOL-002). pool.check
-- checks each pool against the pool rule packs in use: the ISPSC and VGB packs under the IBC,
-- the ADA's with the ADA rule set, and the MAHC when the project chooses it (rules.set_packs).

local l1 = riviera.level.list()[1].id
riviera.clearance.set_rule_set{ standard = "ada_2010" }
riviera.rules.set_packs{ domain = "pool", packs = { "us_mahc_2024", "us_vgb", "ada_2010_pools" } }

local function rect(x0, y0, x1, y1)
  local p = { { x0, y0 }, { x1, y0 }, { x1, y1 }, { x0, y1 } }
  local out = {}
  for i = 1, 4 do out[i] = { line = { start = p[i], ["end"] = p[i % 4 + 1] } } end
  return out
end
riviera.floor.create{ loops = { rect(0, 0, 32, 20) }, level_id = l1 }

-- 20 × 10 m, its shallow end at x = 6: 0.9 m, to 5 ft over 8 m, to 2.5 m over 3 m more.
local pool = riviera.pool.create{ level = l1, name = "Pool", outline = { { 6, 15 }, { 6, 5 }, { 26, 5 }, { 26, 15 } },
  profile = { { at = 0, depth = 0.9 }, { at = 8, depth = 1.524 }, { at = 11, depth = 2.5 } } }

-- Entries: ladders on both long sides at each end (the pool is over 30 ft wide), and a lift.
local t = riviera.bath.load_components{ names = { "Pool ladder", "Pool lift", "Pool main drain, 300 × 300" } }
local function place(ty, x, y, turn, offset)
  return riviera.bath.place_component{ type_id = ty, point = { x, y }, level = l1, rotation = turn, offset = offset or 0, work_plane = true }
end
place(t["Pool ladder"], 9, 15.3, 0)
place(t["Pool ladder"], 24.5, 15.3, 0)
place(t["Pool ladder"], 9, 4.7, math.pi)
place(t["Pool ladder"], 24.5, 4.7, math.pi)
place(t["Pool lift"], 10, 4.4, math.pi)
-- Two blockable main drains in the deep end, 2 m apart (the VGB Act's 3 ft at least).
place(t["Pool main drain, 300 × 300"], 24, 9, 0, -2.65)
place(t["Pool main drain, 300 × 300"], 24, 11, 0, -2.65)

local c = riviera.pool.check{}
for _, p in ipairs(c.pools) do
  print(string.format("%s (%s): %.0f m², %d entries, %d accessible, %d drains", p.name, p.class, p.area, p.entries, p.accessible_entries, p.drains))
  for _, l in ipairs(p.loads) do print(string.format("  %s: %d bathers (%.2f m² each)", l.rule, l.bathers, l.per)) end
end
for _, f in ipairs(c.findings) do print(f.severity, f.rule, f.message) end
for _, f in ipairs(c.findings) do assert(f.severity ~= "error", f.rule .. ": " .. f.message) end
