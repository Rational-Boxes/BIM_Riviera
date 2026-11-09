# Swimming Pools — Plan

**Status:** Draft 0.2 · 2026-11-09 · the owner answered every question (§10); steps 1–2 built (§11)
**Research:** `research/23_swimming_pools.md` (sources S2762–S2868)
**Related:** the bathroom toolset's hot tubs, saunas, steam rooms, spa access and whirlpool baths
(ARC-BATH-013…017); the shared rule packs (ARC-RULE-001/002); the gym toolset (ARC-GYM-001/002).

## 1. Scope

The owner asked on 2026-11-08 for specifications for swimming pools. Pools of every kind:
- residential: rectangular, freeform, lap, plunge, infinity edge, beach entry;
- public and leisure: leisure pools, teaching and learner pools, wading pools;
- competition: 25 yd, 25 m and 50 m, and diving;
- hydrotherapy and spa pools.

The toolset covers the pool's geometry and equipment, the rules that check them, and what the
pool asks of the building:
- the hall's air and dehumidification;
- the water's heat and treatment plant;
- structure, bonding and lighting.

Water parks, wave pools and lazy rivers are out of scope beyond headline sizes (research/23 D.1.4).

## 2. Principles

- **One pool element, built from a recipe.** Like the other Riviera elements:
  - an outline, sketched or typed (the sketch-and-coordinates rule);
  - a water level, a depth profile, an edge system, coping, steps and a shell.
  It regenerates when any of these change.
- **Rules in packs**, in the shared format `riviera.rules-1` (ARC-RULE-001):
  - a `pool` section for slopes, depths, entries, markings, diving envelopes, decks and barriers;
  - use zones for clear decks round lifts, ladders and blocks;
  - jurisdictions and editions as separate packs, so conflicts (research/23 E.1) stay visible.
- **Equipment as typed placeholders** in a *Riviera Pools* component library, with their sources,
  until a product is chosen (the gym and retail pattern, ARC-FURN-001).
- **Integration through the existing tools.** Evaporation and dehumidification feed the HVAC
  loads and the energy model. The water's weight and groundwater uplift feed the structural
  loads. The bonding grid and underwater lights feed the electrical model.
- **Every value cited.** Values no source gives are judgement, logged in `EXPERT_REVIEW.md`.

## 3. The pool element (`pool.*`)

- `pool.create(outline | template, level, water_level, profile, edge, coping?, shell?)`:
  - Templates: rectangle, L, kidney, freeform, lap, plunge, competition 25 yd, 25 m and 50 m,
    teaching, wading.
  - **Profile:** depth points or segments, from shallow end to deep end:
    - the break point at 5 ft (1.52 m), where the slope limit changes;
    - floor slopes per segment (1:12 / 1:3 under the MAHC; 1:7 in ISPSC residential; 1:20 / 1:17
      in Sport England's preferred gradients);
    - constant-depth and movable-floor options;
    - the wall–floor cove radius.
  - **Edge:** skimmer; deck-level gutters (Finnish, Wiesbaden or Zurich) with the grating width;
    infinity edge with its catch basin; raised edge.
  - **Coping:** profile, width, overhang, per edge.
  - **Shell:** cast concrete, shotcrete, panel or one-piece, with thickness, waterproofing and
    tiling layers.
- `pool.steps`, `pool.bench`, `pool.swimout`, `pool.beach_entry`: entries cut into the shell.
- `pool.lanes`: lane lines, T-ends and wall targets from the World Aquatics/FINA or NFHS presets,
  with the touch-pad allowance in the tank's length.
- Read-outs: water area, perimeter, volume, deck area, and the depth at any point.

## 4. Equipment library (*Riviera Pools*)

From research/23 D.4 and the ADA, MAHC and FINA tables:
- ladders and grab rails; recessed treads; pool stairs; starting blocks;
- backstroke flags and the false-start rope; lane ropes and anchors;
- lifeguard chairs; 1 m and 3 m diving boards and stands; slides; pool lifts;
- safety and thermal covers; underwater lights;
- main drains, returns, skimmers and gutter outlets; the balance tank.

Each item carries:
- its size, mass and source;
- its use zone, for example a pool lift's clear deck or a starting block's approach;
- its anchoring;
- for powered items, its electrical data.

## 5. Rule packs and checks (`pool.check`)

| Pack | Layer | Covers |
|---|---|---|
| `us_mahc_2024` | minimum | CDC MAHC 2024: slopes, cove radii, stairs, handrails, ladders, entries and spacing, depth and NO DIVING markings, the break-point line, decks, the diving-well table, enclosures, lifeguard chairs, bather load, turnover, drains, lighting, indoor air, chemical rooms (research/23 A) |
| `us_ispsc` | minimum | ISPSC barriers (§305), residential diving (Table 804.1), and public-pool values marked [S] until read in ICC's free view |
| `ada_2010_pools` | accessible | §242 counts by perimeter (300 linear ft) and §1009's lifts, sloped entries, transfer walls and systems, and pool stairs |
| `us_vgb` | minimum | unblockable drains, dual drains ≥ 3 ft apart, the CPSC cover rule |
| `uk_sport_england` | recommended | Sport England 2013 and AISF 2024: pool types, lanes, depths and gradients, surround widths, hall height, accessibility of the water |
| `uk_hsg179` | recommended | HSE: lifeguard zones, signage, depth markings, diving depths |
| `world_aquatics` | minimum (when a pool is marked for competition) | FINA 2017–2021 with the 2026 checklist changes: lanes, depths, blocks, flags and ropes, the diving table, water polo, artistic swimming |

`pool.check` reports, per pool:
- each segment's slope and the depths against the pack;
- entries by pool width and their spacing;
- depth markings;
- diving envelopes against steps, ledges and other obstructions (the overlap engine);
- deck widths and barriers;
- §242's accessible entries;
- drains;
- the competition geometry where the pool is marked for it;
- bather load and occupant load (50 ft² of water, 15 ft² of deck, gross, under the IBC).

## 6. Building integration

- **Natatorium air:**
  - evaporation by the ASHRAE equation with activity factors, through the ENERGY STAR and
    EnergyPlus texts (research/23 C.1.1), as the latent load;
  - design conditions: air near or a little above the water, and RH 50–60 % or 70 %, per pack;
  - outdoor air at 0.48 cfm/ft² of pool and deck, plus spectators;
  - the hall kept under negative pressure;
  - dew point at the envelope, and a vapour retarder of 0.2 perm or less.
- **Water heat balance:** losses by evaporation, convection, radiation and conduction, and the
  savings from covers, for the energy model.
- **Water treatment:**
  - turnover by pool type and depth;
  - filter area and backwash rate;
  - suction 1.8 m/s and discharge 2.4 m/s;
  - gutters at 125 %, and the surge volume (40.7 L/m² in the MAHC);
  - plant-room area (15–30 % of the water area, Sport England);
  - chemical stores separated and ventilated.
- **Structure:**
  - the water's weight, full and empty, with groundwater uplift;
  - point loads from blocks, boards and lifts;
  - elevated pools' loads per m², for the structural tools.
- **Electrical:**
  - the equipotential bonding grid;
  - IEC 60364-7-702 zones 0/1/2 or NEC 680 distances, by jurisdiction;
  - 12 V SELV underwater lights; RCD/GFCI requirements.
- **Lighting and acoustics:**
  - illuminance at the water: 300/500/600/1500 lx by use (Sport England), and 323 lx under the
    MAHC;
  - underwater lighting at 8 lm/ft² or more;
  - reverberation time 1.5–2.0 s, or BB93's limit for school pools.
- **Materials:** stainless steel grades for load-bearing parts in the pool atmosphere (CRC V: 1.4565,
  1.4529, 1.4547); no suspended ceilings in materials that corrode.

## 7. IFC mapping

No IFC version has a pool entity (research/23 E.2). The proposal, which is Question 1:
- the pool hall or outdoor area is an `IfcSpace`;
- the water is an `IfcSpace` USERDEFINED ("WATER") inside it, with its area and volume
  quantities;
- the shell floor is an `IfcSlab` BASESLAB and its walls `IfcWall`, RETAININGWALL in IFC 4.3
  and SOLIDWALL in IFC4;
- tiling and liner are `IfcCovering` FLOORING, CLADDING and MEMBRANE; coping is `IfcCovering`
  COPING in IFC 4.3, USERDEFINED in IFC4; lane lines are `IfcSurfaceFeature` LINEMARKING in
  IFC 4.3;
- equipment follows NBS's Uniclass mappings: ladders `IfcStair` USERDEFINED, lifts
  `IfcTransportElement` USERDEFINED, covers `IfcCovering` USERDEFINED, boards and blocks
  `IfcFurniture`, filters `IfcFilter` WATERFILTER, the balance tank `IfcTank`;
- small pools and spas may instead be `IfcSanitaryTerminal` BATH with BathType POOL, SPA, PLUNGE
  or TREATMENT;
- diving envelopes and clear decks are `IfcVirtualElement` CLEARANCE in IFC 4.3
  (`IfcBuildingElementProxy` PROVISIONFORSPACE in IFC4);
- Uniclass codes per research/23 D.5.3.

## 8. Build order (proposed)

1. The pool element: outline, water level, profile, edge, coping, read-outs. Spec first.
2. The pool rule-pack section and `pool.check` with the MAHC, ISPSC and ADA packs.
3. Entries (steps, benches, beach entry) and the equipment library with use zones.
4. Competition presets: lanes, markings, blocks, the World Aquatics pack; diving envelopes
   checked with the overlap engine.
5. UK packs (Sport England, HSG179).
6. Natatorium HVAC and the water heat balance, feeding the loads and energy model.
7. Water treatment sizing and the plant room.
8. Structural loads and electrical bonding.

Each step gets its spec section, a sample with a help walkthrough, and registered commands
exposed to Lua and MCP.

## 9. Questions for the owner

1. **IFC representation:** the shell as slabs and walls, with the water as an `IfcSpace` (as §7)?
   Or one `IfcBuildingElementProxy` for the whole pool, which is simpler but loses the parts?
2. **Shell and the building:** should the pool generate its own shell (walls and floor slab, cut
   into the level's floor)? Or sit in a pit modelled with the ordinary wall and floor tools?
3. **Default packs in the US:** the ISPSC through the building-code setting with the MAHC
   opt-in? Or the MAHC by default for public pools (it is a model code adopted unevenly by
   states)?
4. **Conflicting values:** keep each pack's own value and show the conflict? Or add a "strictest"
   pack that takes the most demanding value of each?
5. **Competition values:** ship FINA 2017–2021 with the 2026 checklist changes, marked "verify
   against the current World Aquatics text"? Or wait until the current Part 2 text is read?
6. **Order:** residential pools first, or public and competition pools first?
7. **Natatorium HVAC and water treatment:** build them with the pool (steps 6–7), or after the
   geometry and checks are done?

## 10. The owner's answers (2026-11-08)

- **Q1, IFC:** as §7. The shell is slabs and walls, tiling and coping are coverings, and the water
  is an `IfcSpace` USERDEFINED ("WATER"). Small pools and spas may be `IfcSanitaryTerminal` BATH.
- **Q2, shell:** the pool generates its own shell (walls and sloped floor) from its outline and
  profile. It cuts the level's floor or deck round it and regenerates when the profile changes.
- **Q3, US packs:** the ISPSC follows the building-code setting, and the MAHC is chosen per
  project. The ADA pool rules apply with the ADA rule set.
- **Q4, conflicts:** each pack checks its own text's value and cites it. A project picks the packs
  its jurisdiction adopted, and conflicts show where packs disagree. No "strictest" pack.
- **Q5, competition:** ship FINA 2017–2021 with the 2026 checklist changes. Each finding says to
  verify against the current World Aquatics text.
- **Q6, order:** public and competition pools first; residential presets follow.
- **Q7, integration:** after the geometry and checks: steps 6–8 of §8 stay where they are.

## 11. Progress

- Step 1 is built (ARC-POOL-001, 2026-11-08): the pool element from a template or a typed
  outline, its depth profile in planar strips, the generated shell walls and base slab, coping and
  water, the floor openings, the read-outs, and the IFC assembly with its WATER space. Sketching
  outlines in the window, moving pools, and drawing the cove, gutters, basins, raised walls,
  tiling and membrane remain. Next: step 2, `pool.check` with the ISPSC, MAHC, ADA and VGB packs.
- Step 2 is built (ARC-POOL-002, 2026-11-09): the `pool` rule kind in `riviera.rules-1`, the
  `us_mahc_2024`, `us_ispsc`, `ada_2010_pools` and `us_vgb` packs, `pool.check` with each pool's
  class, a pool ladder and main-drain covers beside the bathroom components' pool lift, View ›
  Coordination › Check Pools, and the sample *A public pool checked against the codes*. Not yet:
  depth markings, NO DIVING, the break-point and float lines, barriers and gates, where entries
  stand, stairs and handrails, deck slopes, diving tables. Next: step 3, entries cut into the
  shell and the *Riviera Pools* equipment library with use zones.
