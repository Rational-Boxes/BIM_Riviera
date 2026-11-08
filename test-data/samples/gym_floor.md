# A fitness room: building it by hand

How to build `gym_floor.ifc` in the window. Each step names the UI path; **[gap]** marks a step
the window cannot do yet, and the [gaps](#gaps) section lists them. The script `gym_floor.lua` is
the reference: the same commands with exact values.

Start with **File › New**. In **Manage › Model Properties**, set the **Accessibility rule set** to
*2010 ADA Standards*. Under **Gym rules** the equipment makers' clearances are in use already.

1. **The rooms.** Draw four walls 4 m tall round a 26 × 17 m suite from (0, 0), then two
   partitions that stop at the other walls' faces: along y = 10 from x = 0.1 to 25.9, and along
   x = 18 from y = 0.1 to 9.9. Place three rooms: *Fitness* at (9, 5), *Cycle studio* at (22, 5)
   and *Circuit* at (13, 13.5). Put a ceiling 3 m up in the fitness room with
   **Architecture › Build › Ceiling**.
2. **Cardio.** From **Insert › Component Libraries › Riviera Gym Equipment › Cardio**, place four
   commercial treadmills along the north wall at x = 1.5, 3.5, 5.5 and 7.5 m, y = 8.6 m, each
   stepped on from the south. Their 0.5 m side zones and 2 m rear zones show as virtual volumes.
   South of them, place an upright bike, a recumbent bike, an elliptical, a stair climber and a
   rower at y = 2.5 m.
3. **Strength.** From *Strength* and *Free weights*, place a cable crossover at (13.5, 2), a power
   rack at (12, 6), a leg press at (15.5, 7), a flat bench at (11, 4) and a two-tier dumbbell rack
   at (12, 9.3) facing south. Each strength station keeps 0.9 m clear round it (the ACSM's,
   advisory).
4. **A ceiling fan.** From **Riviera Interiors › Fans**, place the 52 in ceiling fan at (4.5, 4.5),
   2.5 m up.
5. **A mirror.** Choose **Architecture › Build › Wall Mirror**, keep 0.4 to 2.1 m and *Safety
   glazing*, and click (17.8, 4.5), (17.8, 9) and a point in the room. It faces the strength area,
   away from the treadmills.
6. **A cycling studio.** Select the *Cycle studio* room and choose **Architecture › Build ›
   Studio**, *Indoor cycling*. At 1.5 × 2.0 m a bike its 76 m² holds about 25. Twelve bikes go in
   rows 2 m clear of the instructor's wall, facing it. **[gap]** The window takes the longest wall
   as the instructor's; the script picks the east wall with `head`.
7. **A circuit room.** Select the *Circuit* room and choose **Architecture › Build › Gym Layout**.
   Enter 3 commercial treadmills, 2 recumbent bikes, a rower, a leg press and a Smith machine. They
   go in a row along the north wall, each kept clear by its own zones. **[gap]** The window lays the
   rows from the longest wall; the script names the north wall with `head`.
8. **Check.** **View › Coordination › Check Gym** lists each type with an accessible unit and its
   30 × 48 in clear floor space (ADA §236.1, §1004.1) and finds nothing. Lower the ceiling to 2.4 m
   and the treadmills, stair climber and rack are notes. Move the mirror to the north wall behind
   the treadmills and it is a note too. **Check Rules** finds every use zone clear; put the bench
   behind a treadmill to see its rear zone reported. Check Gym also gives each room's occupant
   load at 50 ft² gross (IBC Table 1004.5) and notes a room with less than 5 m² a piece.

## Gaps

- **[gap]** The window cannot pick the head wall for Gym Layout and Studio yet. Whether the accessible
  machines are on an accessible route, and the UK's inclusive transfer space and IFI counts, are not
  checked.
- **[gap]** The equipment's circuits, heat gains, outdoor air and floor loads do not yet feed the
  electrical, energy and structural tools; anchors are not modelled.
- **[gap]** The fan-light's light kit is not aggregated under the fan; fan-to-wall clearance is not
  checked.
