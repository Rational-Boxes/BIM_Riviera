-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A resort pool with a beach entry
-- description: A 25 × 12 m public pool, 0.9 m deep to 14 m and then down to 2.0 m at the deep end, entered by a 6 m zero-depth beach at its shallow end, 4.5 m stairs with three handrails, an ADA sloped entry with its two handrails, and recessed steps with grab rails in the deep end's wall; checked against the MAHC, the ISPSC, the VGB Act and the ADA.
--
-- Riviera sample model (test-data/samples): handrails, recessed steps and beach entries
-- (ARC-POOL-013). Every entry built into a pool carries its handrails (or, for recessed steps,
-- grab rails), which pool.check measures: how many, how high, and how far apart.

local l1 = riviera.level.list()[1].id
riviera.clearance.set_rule_set{ standard = "ada_2010" }
riviera.rules.set_packs{ domain = "pool", packs = { "us_mahc_2024", "us_ispsc", "us_vgb", "ada_2010_pools" } }

local function rect(x0, y0, x1, y1)
  local p = { { x0, y0 }, { x1, y0 }, { x1, y1 }, { x0, y1 } }
  local out = {}
  for i = 1, 4 do out[i] = { line = { start = p[i], ["end"] = p[i % 4 + 1] } } end
  return out
end
-- The deck, its top at the level.
riviera.floor.create{ loops = { rect(0, 0, 34, 22) }, level_id = l1 }

-- The MAHC's 6 ft enclosure (ARC-POOL-012), its gate opening outward.
local fence_type = riviera.railing_type.create{ name = "Pool enclosure 6 ft", height = 1.85, bottom_rail_height = 0.05, baluster = 0.025, baluster_spacing = 0.12 }
local fence = riviera.railing.create{ type_id = fence_type, path = { { 0.3, 0.3 }, { 33.7, 0.3 }, { 33.7, 21.7 }, { 0.3, 21.7 } }, closed = true, base_level = l1 }
riviera.railing.add_gate{ railing_id = fence, point = { 17, 0.3 }, swing_out = true }

-- 25 × 12 m, its shallow end the west wall (edge 1): 0.9 m to 14 m out, 5 ft at 21.5 m (1:12),
-- 2.0 m at 24 m, and level to the east wall (edge 3).
local pool = riviera.pool.create{ level = l1, name = "Resort pool", outline = { { 4.5, 17 }, { 4.5, 5 }, { 29.5, 5 }, { 29.5, 17 } },
  profile = { { at = 0, depth = 0.9 }, { at = 14, depth = 0.9 }, { at = 21.5, depth = 1.524 }, { at = 24, depth = 2.0 }, { at = 25, depth = 2.0 } } }
riviera.pool.depth_markings{ pool_id = pool }

-- A beach in the middle 6 m of the shallow end, rising 1:12 (the MAHC's steepest) from the
-- floor to the deck: the water's edge 1.8 m out from the wall, the floor met 12.6 m out.
local beach = riviera.pool.add_entry{ pool_id = pool, kind = "beach", edge = 1, at = 6, width = 6 }
print(string.format("Beach: 1:%.0f, the water's edge %.2f m out, the floor met %.2f m out", 1 / beach.slope, beach.waterline, beach.run))

-- Stairs 4.5 m wide on the south wall by the shallow end: wider than 5 ft, so handrails on both
-- sides, and a third down the middle keeps them 2.2 m apart (the MAHC's 12 ft at most).
local stairs = riviera.pool.add_entry{ pool_id = pool, kind = "stairs", edge = 2, at = 4, width = 4.5, handrails = 3 }
print(string.format("Stairs: %d risers of %.0f mm, %d handrails %.0f mm high, %.2f m apart", stairs.risers, stairs.riser * 1000, stairs.handrails, stairs.handrail_height * 1000, stairs.handrail_spacing))

-- The ADA's sloped entry from the north wall: 1:12 to 27 in under the water, its handrails
-- 35 in apart clear (ADA 1009.3.3's 33–38 in).
local ramp = riviera.pool.add_entry{ pool_id = pool, kind = "sloped_entry", edge = 4, at = 7.5 }
print(string.format("Sloped entry: %.2f m to %.0f mm below the water, handrails %.0f mm apart clear", ramp.run, ramp.reaches * 1000, ramp.handrail_gap * 1000))

-- Recessed steps in the deep end's wall: 18 in treads cut 5½ in into the wall 10 in apart, the
-- top one 10 in below the deck, with a grab rail each side 21 in apart.
local steps = riviera.pool.add_entry{ pool_id = pool, kind = "recessed_steps", edge = 3, at = 6 }
print(string.format("Recessed steps: %d treads %.0f mm apart, grab rails %.0f mm apart clear and %.0f mm above the deck", steps.risers, steps.riser * 1000, steps.handrail_gap * 1000, steps.handrail_height * 1000))

-- Two main drains on the level deep floor, 2 m apart (VGB).
local t = riviera.furnishing.load{ names = { "Main drain cover 300 × 300" } }
for _, y in ipairs({ 10, 12 }) do
  riviera.mesh_object.place{ type_id = t["Main drain cover 300 × 300"], point = { 29, y }, level = l1, offset = -2.15 }
end

local i = riviera.pool.info{ pool_id = pool }
print(string.format("%.1f m² of water, %.1f m³ (less what the beach, stairs and ramp displace)", i.area, i.volume))
local c = riviera.pool.check{}
for _, p in ipairs(c.pools) do
  print(string.format("%s (%s): %d entries, %d accessible", p.name, p.class, p.entries, p.accessible_entries))
end
for _, f in ipairs(c.findings) do print(f.severity, f.rule, f.message) end
for _, f in ipairs(c.findings) do assert(f.severity ~= "error", f.rule .. ": " .. f.message) end
assert(#riviera.overlap.find{}.overlaps == 0, "nothing overlaps")
