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
5. **Starting platforms.** From **Insert › Component Libraries › Riviera Pools › Competition**,
   place a *Starting platform 24 × 32 in* at the east end of each lane, turned to face west
   (−90°), its front edge on the wall: at x = 28.92 and y = 5.75, 8.25, 10.75, 13.25, 15.75 and
   18.25 (the lanes' centres).
6. **Ladders, a lift and a chair.** From *Access*, place a *Pool ladder, 3 steps* on the north
   wall's face at (5, 19.5) and (27, 19.5), and turned 180° on the south wall's at (5, 4.5) and
   (27, 4.5). Place the *Pool lift, 300 lb* turned 180° at (8, 3.6), and from *Safety* the
   *Lifeguard chair 6 ft* at (16, 21.6), both facing the water.
7. **Drains.** From *Fittings*, place two *Main drain cover 300 × 300* at (16, 11) and (16, 13),
   2.15 m down on the floor.
8. **The teaching pool's entries.** Select the teaching pool and choose **Architecture › Build ›
   Pool Entry**: *Stairs*, clicking by its west wall at (30, 18.5), then *Sloped entry (ADA)* at
   (30, 21.5), and *Stairs* again by its east wall at (42, 20). The stairs step down in equal
   risers on 12 in treads; the sloped entry falls 1:12 for 10 m to 27 in below the water. The
   ISPSC wants an entry at each end and does not count accessible devices among them, hence the
   second stairs.
9. **Check.** **View › Coordination › Check Pools** lists each pool's entries, accessible
   entries and drains: the main pool has four ladders and the lift, the teaching pool its two sets
   of stairs (and its sloped entry, an accessible one). Nothing overlaps (**Check Overlaps**).
10. **IFC.** Saved, each pool is an `IfcElementAssembly` (SWIMMINGPOOL) of an `IfcWall`, an
   `IfcSlab` (BASESLAB) and an `IfcCovering` (COPING), with the teaching pool's `IfcStair` and
   `IfcRamp`, and its water an `IfcSpace` (WATER) under the level carrying its area and volume.
   The equipment is ordinary typed objects: the lift an `IfcTransportElement`, the rest
   `IfcFurniture` of their kinds.

## Gaps

- **[gap]** The window places templates and typed outlines; sketching a freeform outline with the
  sketch tools, and editing a pool's profile in the window (`pool.set` from a script), come later.
- **[gap]** Lane lines, T-ends, backstroke flags, the false-start rope and touch pads, and the
  World Aquatics checks of a competition pool, are the next step; equipment is placed by
  coordinates, not snapped to the pool's edge.
- **[gap]** Deck-level gutters and infinity-edge basins are recorded but not drawn, the wall–floor
  cove is a value only, and tiling and membrane layers are not drawn.
