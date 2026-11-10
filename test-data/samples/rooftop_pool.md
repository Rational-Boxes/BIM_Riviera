# A rooftop pool: building it by hand

How to build `rooftop_pool.ifc` in the window. Each step names the UI path; **[gap]** marks a
step the window cannot do yet, and the [gaps](#gaps) section lists them. The script
`rooftop_pool.lua` is the reference: the same commands with exact values.

Start with **File › New**.

1. **The levels.** Move *Level 1* to 8.0 m and rename it *Plant* (**Architecture › Datum ›
   Level** properties), then add a level *Roof* at 12.0 m.
2. **The terrace.** On *Roof*, draw a floor 18 × 12 m from (0, 0) with **Architecture › Build ›
   Floor**.
3. **The pool.** Choose **Architecture › Build › Swimming Pool** on *Roof*, type the outline
   `4,8; 4,4; 14,4; 14,8` and the depths `0:1.2, 10:1.2`: 1.2 m throughout, a hotel pool with no
   diving. Set its shell to 250 mm of cast concrete, tiled, with a membrane. **[gap]** The shell's
   tiling and membrane are set from a script (`pool.set{shell = …}`).
4. **Equipment.** From **Insert › Component Libraries › Riviera Pools**, place a *Pool lift,
   300 lb* on the north deck at (6.5, 8.6), a *Pool ladder, 3 steps* on the north wall's face
   at (12.5, 8), and two *Underwater light, large niche* on the south wall's face at (7, 4) and
   (11, 4), turned 180° to face the water. The library's light sits with its lens top 18 in
   (457 mm) under water 150 mm below the deck, as NEC 680.23(A)(5) asks.
5. **The terrace's electrics.** With **Systems › Electrical › Device**, place two *Receptacle*
   at (6, 10.5) and (12, 10.5), 0.45 m up and standing free, and a *Light Switch* at (16, 6),
   1.1 m up; with **Lighting Fixture**, two *Ceiling Luminaire* at (6, 0.5) and (12, 0.5),
   3.0 m up; with **Electrical Equipment**, a *Distribution Board* at (17.5, 11.5).
6. **Its loads.** Select the pool and choose **View › Coordination › Pool Structure**. A level
   is below it, so it is taken as standing on a structure: the water, the shell and the tiles in
   kN; the load on average over the footprint, on the floor at its deepest, and along the walls
   as a line load; the walls' hydrostatic pressure and moment; the lift's and the ladder's
   advisory point loads; and the slab's deflection limit (its 4 m span over 360, 11 mm), with
   the rooftop notes (membrane and drainage, geofoam, no plumbing cast in, a flood test).
7. **On the slab.** **[gap]** Draw the slab under the shell as an analytical panel 12 × 6 m at
   10.4 m (**Analyze › Analytical Model › Panel**), add a load nature *Fluid* (F), and load
   cases *D* and *F*; then put the pool's weight on it with `pool.apply_loads` from a script:
   the window has no button for it yet. The shell and tiles go in *D* and the water in *F*,
   each its total spread over the panel's 72 m².
8. **Electrical zones.** Choose **View › Coordination › Pool Electrical Zones**. Under the IBC
   it takes NEC Article 680: the receptacles 2.5 m from the water (more than 6 ft) need GFCI
   protection, being within 20 ft; the switch is more than 5 ft away; the luminaires, 3.5 m
   out, are beyond 10 ft; the underwater lights run at 12 V, under the 15 V low-voltage contact
   limit. It sizes the bonding grid (8 AWG copper on a 12 in grid under the 3 ft perimeter
   surface, tied to the shell's steel at four points) and lists the lift, the ladder and the
   lights to bond.

## Gaps

- **[gap]** The shell's tiling and membrane, and `pool.apply_loads`, are script-only.
- **[gap]** The zones' rings are returned as outlines but not drawn in the views.
- **[gap]** The loads are spread evenly over the panel; the water's pressure under the deepest
  point and the walls' line loads are not placed separately.
- **[gap]** Point loads are advisory values by equipment type, not the makers' data.
