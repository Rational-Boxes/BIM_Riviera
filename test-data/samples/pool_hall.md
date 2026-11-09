# A swimming pool hall: building it by hand

How to build `pool_hall.ifc` in the window. Each step names the UI path; **[gap]** marks a step
the window cannot do yet, and the [gaps](#gaps) section lists them. The script `pool_hall.lua` is
the reference: the same commands with exact values.

Start with **File › New**.

1. **The deck.** Draw a floor 44 × 30 m from (0, 0) with **Architecture › Build › Floor**, its top
   at the level.
2. **The main pool.** Choose **Architecture › Build › Swimming Pool**, *Competition 25 m*, and
   leave the lanes empty for six. Click (16, 12) for its centre and a point east of it for its
   deep end. The pool is 25.03 × 15 m (2.5 m lanes and 30 mm for the touch pads) and 2.0 m deep.
   Its shell walls and base slab (0.3 m of concrete), its 0.3 m coping and its water appear, and
   the deck is cut round it.
3. **The teaching pool.** Choose **Swimming Pool** again, *Teaching 12 × 7 m*, and enter the depths
   `0:0.6, 12:0.9`. Click (36, 20) and a point east of it. Its floor falls from 0.6 m to 0.9 m,
   a slope of 1:40.
4. **Its numbers.** The status bar gives each pool's water area, volume and depths as it is placed
   (`pool.info` from a script also gives each segment's slope and the shell's concrete volume).
5. **IFC.** Saved, each pool is an `IfcElementAssembly` (SWIMMINGPOOL) of an `IfcWall`, an
   `IfcSlab` (BASESLAB) and an `IfcCovering` (COPING), with its water an `IfcSpace` (WATER) under
   the level carrying its area and volume.

## Gaps

- **[gap]** The window places templates and typed outlines; sketching a freeform outline with the
  sketch tools, and editing a pool's profile in the window (`pool.set` from a script), come later.
- **[gap]** The checks (`pool.check`: slopes, entries, depth markings, decks, barriers, ADA §242),
  steps and other entries, the equipment library and the competition markings are the next steps.
- **[gap]** Deck-level gutters and infinity-edge basins are recorded but not drawn, the wall–floor
  cove is a value only, and tiling and membrane layers are not drawn.
