-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A public pool checked against the codes
-- description: A 20 × 10 m public pool, 0.9 m deep falling to 5 ft at 1:12.8 and to 2.5 m at 1:3.1, with stairs and an ADA sloped entry at the shallow end, ladders on both sides of the deep end, a pool lift and two main drains 2 m apart; checked against the CDC's Model Aquatic Health Code, the VGB Act's drain rules and the ADA's pool rules.
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

-- The MAHC's enclosure (ARC-POOL-012): a 1.85 m (6 ft) fence round the deck, 2 in off it, its
-- balusters 95 mm apart, with a gate in the south side opening outward, away from the pool.
local fence_type = riviera.railing_type.create{ name = "Pool enclosure 6 ft", height = 1.85, bottom_rail_height = 0.05, baluster = 0.025, baluster_spacing = 0.12 }
local fence = riviera.railing.create{ type_id = fence_type, path = { { 0.3, 0.3 }, { 31.7, 0.3 }, { 31.7, 19.7 }, { 0.3, 19.7 } }, closed = true, base_level = l1 }
local gate = riviera.railing.add_gate{ railing_id = fence, point = { 16, 0.3 }, swing_out = true }
-- Self-closing, self-latching and lockable (ARC-POOL-016), its latch 54.3 in up: over the MAHC's 54 in.
riviera.railing.gate_hardware{ gate_id = gate, latch_height = 1.38 }

-- 20 × 10 m, its shallow end at x = 6: 0.9 m, to 5 ft over 8 m, to 2.5 m over 3 m more.
local pool = riviera.pool.create{ level = l1, name = "Pool", outline = { { 6, 15 }, { 6, 5 }, { 26, 5 }, { 26, 15 } },
  profile = { { at = 0, depth = 0.9 }, { at = 8, depth = 1.524 }, { at = 11, depth = 2.5 } } }
-- Depth markings (ARC-POOL-011): every 1 ft of depth to 5 ft, at most 25 ft apart, NO DIVING to
-- 5 ft, and the break-point band with the float line 1 ft shallower.
local marks = riviera.pool.depth_markings{ pool_id = pool }
print(string.format("%d depth markers; the break-point band at %.2f m along the pool, the float line at %.2f m", #marks.markers, marks.lines.band[1], marks.lines.float_line))

-- Entries built into the shallow end's wall (ARC-POOL-003): stairs, and a sloped entry falling
-- 1:12 to 27 in below the water (ADA 1009.3).
local stairs = riviera.pool.add_entry{ pool_id = pool, kind = "stairs", point = { 6, 8 } }
local ramp = riviera.pool.add_entry{ pool_id = pool, kind = "sloped_entry", point = { 6, 13 } }
print(string.format("Stairs: %d risers of %.0f mm; sloped entry: %.2f m long to %.0f mm below the water", stairs.risers, stairs.riser * 1000, ramp.run, ramp.reaches * 1000))

-- From the Riviera Pools library: ladders on both long sides at the deep end (the pool is over
-- 30 ft wide), set on the walls' faces; a lift on the south deck facing the water.
local t = riviera.furnishing.load{ names = { "Pool ladder, 3 steps", "Pool lift, 300 lb", "Main drain cover 300 × 300" } }
local function place(ty, x, y, turn, offset)
  return riviera.mesh_object.place{ type_id = ty, point = { x, y }, level = l1, rotation = turn, offset = offset or 0 }
end
place(t["Pool ladder, 3 steps"], 24.5, 15, 0)
place(t["Pool ladder, 3 steps"], 24.5, 5, math.pi)
place(t["Pool lift, 300 lb"], 10, 4.4, math.pi)
-- Two blockable main drains in the deep end, 2 m apart (the VGB Act's 3 ft at least).
place(t["Main drain cover 300 × 300"], 24, 9, 0, -2.65)
place(t["Main drain cover 300 × 300"], 24, 11, 0, -2.65)

local c = riviera.pool.check{}
for _, p in ipairs(c.pools) do
  print(string.format("%s (%s): %.0f m², %d entries, %d accessible, %d drains", p.name, p.class, p.area, p.entries, p.accessible_entries, p.drains))
  for _, l in ipairs(p.loads) do print(string.format("  %s: %d bathers (%.2f m² each)", l.rule, l.bathers, l.per)) end
end
for _, f in ipairs(c.findings) do print(f.severity, f.rule, f.message) end
for _, f in ipairs(c.findings) do assert(f.severity ~= "error", f.rule .. ": " .. f.message) end
