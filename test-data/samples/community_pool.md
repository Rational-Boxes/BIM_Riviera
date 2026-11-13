# A community pool in England: building it by hand

How to build `community_pool.ifc` in the window. Each step names the UI path; **[gap]** marks a
step the window cannot do yet, and the [gaps](#gaps) section lists them. The script
`community_pool.lua` is the reference: the same commands with exact values.

Start with **File › New**.

1. **The code.** In **Manage › Model Properties** choose the UK (Approved Document K) as the
   building code. The pool packs are then Sport England's guidance and HSE's HSG179.
2. **The hall's floor.** Draw a floor from (−2, −2) to (27, 14.5) with **Architecture › Build ›
   Floor**: a 2 m surround round the pool.
3. **The pool.** Choose **Interiors › Pools › Swimming Pool**, type the outline
   `0,12.5; 0,0; 25,0; 25,12.5` and the depths `0:1.0, 6:1.35, 15:1.8, 25:2.0`: Sport England's
   typical 25 m profile, 1 in 17 to the 1.35 m non-swimmer depth.
4. **Lanes.** Select the pool and choose **Interiors › Pools › Pool Lanes**: six lanes 2.0 m
   wide, 0.25 m side gaps, no touch pads.
5. **Depth markings.** **Interiors › Pools › Pool Depth Markings**: under the UK code setting
   they read in metres, every 0.5 m of depth and at most 7.6 m apart, as HSG179 asks (the actual
   depth, readable from the surround and the water).
6. **Steps.** **Interiors › Pools › Pool Entry**, *Stairs*, with a tread of 0.30 m and the
   highest riser 0.14 m, clicking by the west wall at (0, 10.5): nine risers of 128 mm.
7. **Hoist, ladders and drains.** From **Insert › Component Libraries › Riviera Pools**, place
   the *Pool lift, 300 lb* turned 180° at (3, −0.6); a *Pool ladder, 3 steps* on the north wall's
   face at (24, 12.5) and, turned 180°, on the south wall's at (24, 0); and two *Main drain cover
   300 × 300* at (21, 6.25) and (23, 6.25), 2.065 m and 2.105 m down: 5 mm above the sloping floor at
   their middles, so their edges clear it.
8. **Check.** **View › Coordination › Check Pools**: no notes. HSG179's 3 m² a bather gives a
   starting bathing load of 104. Make the floor steeper than 1 in 17 above 1.35 m, or the risers
   higher than 0.14 m, and Sport England's guidance says so as a note.

## Gaps

- **[gap]** The start- and turn-end surround widths (3 m and 2 m), the 1.35 m non-swimmer line
  and rope, depth signs and lifeguard numbers are not checked yet.
- **[gap]** Handrails on the steps are not built yet.
