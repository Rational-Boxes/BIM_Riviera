-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: A store front
-- description: A supermarket's front end: nine staffed lanes with three accessible ones spread across the bank, five express lanes in tandem pairs with two accessible, and a bank of four self-checkouts counted on its own, as ADA Table 227.2 asks; each lane's queue kept clear 3 m beyond its in-feed.
--
-- Riviera sample model (test-data/samples): checkout front ends (ARC-RETAIL-003). frontend.layout
-- lays each bank at a pitch along the front and makes the lanes Table 227.2 asks accessible;
-- checkout.check counts every function; checkout.lanes_estimate sizes the bank (an estimate).

local l1 = riviera.level.list()[1].id
riviera.code.set{ code = "ibc" }
riviera.clearance.set_rule_set{ standard = "ada_2010" }
riviera.rules.set_packs{ domain = "retail", packs = { "us_ibc_mercantile", "practice_retail" } }

local IN = 0.0254
-- The lanes' exit ends on y = 3 (a 3 m power aisle to the store front at y = 0), reaching north.
local main = riviera.frontend.layout{ start = { 0.0, 3.0 }, ["end"] = { 20.0, 3.0 }, level = l1, toward = { 0.0, 10.0 } }
local express = riviera.frontend.layout{ start = { 21.0, 3.0 }, ["end"] = { 32.0, 3.0 }, level = l1, toward = { 21.0, 10.0 }, count = 5, ["function"] = "express", handing = "paired" }
local sco = riviera.frontend.layout{ start = { 33.0, 3.0 }, ["end"] = { 43.0, 3.0 }, level = l1, toward = { 33.0, 10.0 }, item = "Self-checkout lane", count = 4, pitch = 1.359 + 36 * IN }

local c = riviera.checkout.check{}
for _, f in ipairs(c.functions) do
  print(string.format("%s: %d lanes, %d accessible (%d needed)", f["function"], f.lanes, f.accessible, f.required))
end
assert(#c.findings == 0, "Table 227.2 is met")
local e = riviera.checkout.lanes_estimate{ customers_per_hour = 300, items_per_basket = 20 }
print(string.format("estimate: about %d lanes for 300 customers an hour", e.lanes))

assert(#riviera.rules.check{} == 0, "the accessible aisles and the queues are clear")
assert(#riviera.overlap.find{}.overlaps == 0, "nothing overlaps")
