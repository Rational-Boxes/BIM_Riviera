-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A hotel floor
-- description: Six guest rooms along a corridor, each furnished from a hotel template: four kings, two queens and an accessible king with its 30 × 48 in bed spaces. The room matrix counts them and finds the 2010 ADA Standards' Tables 224.2 and 224.4 and §224.5 met.
--
-- Riviera sample model (test-data/samples): hotel guest rooms (ARC-INT-002). interior.layout's
-- hotel templates furnish each room and mark it as a guest room; hotel.set_room adds the
-- communication features; hotel.room_matrix counts the rooms by floor and class and checks them.

local l1 = riviera.level.list()[1].id
riviera.code.set{ code = "ibc" }
riviera.clearance.set_rule_set{ standard = "ada_2010" }
riviera.rules.set_packs{ domain = "interiors", packs = { "ada_2010_interiors", "practice_interiors" } }

local function wall(a, b)
  return riviera.wall.create{ curve = { line = { start = a, ["end"] = b } }, base_level = l1, top_constraint = { unconnected = 2.7 } }
end

-- Rooms 5.0 m deep between the corridor wall (its face at y = 0.1) and the outer wall (its face at
-- y = 5.1), with 200 mm partitions between them. Widths between partition faces:
local rooms = {
  { width = 3.6, template = "hotel_king" },
  { width = 3.6, template = "hotel_king" },
  { width = 4.0, template = "hotel_double_queen" },
  { width = 3.8, template = "hotel_accessible_king" },
  { width = 3.6, template = "hotel_king" },
  { width = 3.6, template = "hotel_king" },
}
local x, xs = 0.0, { 0.0 }
for _, r in ipairs(rooms) do
  x = x + r.width + 0.2
  xs[#xs + 1] = x
end
wall({ -0.1, 0 }, { x + 0.1, 0 })
local outer = wall({ -0.1, 5.2 }, { x + 0.1, 5.2 })
for _, px in ipairs(xs) do
  wall({ px, 0.1 }, { px, 5.1 })
end

-- Each room furnished with its bed against the outer wall, and marked as a guest room.
local guest = {}
for i, r in ipairs(rooms) do
  local cx = (xs[i] + xs[i + 1]) / 2
  local l = riviera.interior.layout{ template = r.template, point = { cx, 2.6 }, level = l1, wall_id = outer }
  assert(#l.placed > 0, r.template .. " fits its room")
  guest[i] = l.guest_room
end

-- Communication features (§806.3): the accessible room (§224.5), a king and the double queen,
-- so two count and both classes have one.
riviera.hotel.set_room{ room_ids = { guest[4], guest[1], guest[3] }, communication = true }

local m = riviera.hotel.room_matrix{}
print(string.format("%d guest rooms: %d mobility, %d communication counted", m.total, m.ada.without_roll_in + m.ada.roll_in, m.ada.communication_counted))
assert(#m.findings == 0, "the ADA counts are met")
assert(#riviera.rules.check{} == 0, "every piece keeps its clear spaces")
assert(#riviera.overlap.find{}.overlaps == 0, "nothing overlaps")
