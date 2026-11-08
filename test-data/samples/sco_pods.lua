-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Riviera contributors

-- title: Self-checkout pods
-- description: Two self-checkout pods by a store's front: six pedestal units round an 11 ft courtyard with an attendant station in view of every screen, and a quad pod of scan-and-bag lanes; each pod counts as its own bank under ADA Table 227.2, and each accessible unit is checked against a profile from EN 301 549 and the Access Board's 2022 ANPRM.
--
-- Riviera sample model (test-data/samples): self-checkout pods (ARC-RETAIL-004). sco.place_pod
-- lays the units in two facing rows with the attendant at the closed end and the shared queue at
-- the open end; sco.set_profile states a unit's accessibility features; sco.check checks the
-- courtyard, sightlines, queue, lane-light headroom and the profile (practice notes and advisory
-- findings: there is no fixed US rule for self-checkout).

local l1 = riviera.level.list()[1].id
riviera.code.set{ code = "ibc" }
riviera.clearance.set_rule_set{ standard = "ada_2010" }
riviera.rules.set_packs{ domain = "retail", packs = { "us_ibc_mercantile", "practice_retail" } }

-- Six pedestals, the pod's open end at (4, 4) and its closed end to the north.
local express = riviera.sco.place_pod{ entry = { 4.0, 4.0 }, toward = { 4.0, 10.0 }, level = l1, units = 6 }
-- A quad pod of full scan-and-bag lanes beside it, 12 ft across.
local lanes = riviera.sco.place_pod{ entry = { 12.0, 4.0 }, toward = { 12.0, 10.0 }, level = l1, item = "Self-checkout lane", courtyard = 12 * 0.3048 }

-- The accessible units' features, as the maker states them: every operable part 800–1150 mm up
-- with a forward reach 200 mm over the deck, the screen at 1300 mm, speech, a tactile keypad and
-- Braille.
local accessible = {}
for _, pod in ipairs({ express, lanes }) do
  for _, k in ipairs(pod.accessible) do accessible[#accessible + 1] = pod.units[k + 1] end
end
riviera.sco.set_profile{ element_ids = accessible, operable = { 0.80, 1.15 }, approach = "forward", obstruction_depth = 0.20, screen_height = 1.30, speech = true, tactile_keypad = true, braille = true }

local c = riviera.sco.check{}
for _, p in ipairs(c.pods) do
  print(string.format("pod %s: %d units, %d attendant, %d meeting the profile, courtyard %.2f m", p.pod, p.units, p.attendants, p.meeting_profile, p.courtyard))
end
assert(#c.findings == 0, "the pods meet the practice and the profile")

local t = riviera.checkout.check{}
assert(#t.functions == 2 and #t.findings == 0, "each pod is its own bank, Table 227.2 met")
assert(#riviera.rules.check{} == 0, "the accessible units' 36 in is clear")
assert(#riviera.overlap.find{}.overlaps == 0, "nothing overlaps")
