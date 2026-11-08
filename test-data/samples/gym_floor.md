# A fitness room: building it by hand

How to build `gym_floor.ifc` in the window. Each step names the UI path; **[gap]** marks a step
the window cannot do yet, and the [gaps](#gaps) section lists them. The script `gym_floor.lua` is
the reference: the same commands with exact values.

Start with **File › New**. In **Manage › Model Properties**, set the **Accessibility rule set** to
*2010 ADA Standards*. Under **Gym rules** the equipment makers' clearances are in use already.

1. **The room.** Draw four walls 4 m tall round an 18 × 10 m room from (0, 0), place a room in it
   (*Fitness*), and put a ceiling 3 m up with **Architecture › Build › Ceiling**.
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
6. **Check.** **View › Coordination › Check Gym** lists each type with an accessible unit and its
   30 × 48 in clear floor space (ADA §236.1, §1004.1) and finds nothing. Lower the ceiling to 2.4 m
   and the treadmills, stair climber and rack are notes. Move the mirror to the north wall behind
   the treadmills and it is a note too. **Check Rules** finds every use zone clear; put the bench
   behind a treadmill to see its rear zone reported.

## Gaps

- **[gap]** Gym layouts (`gym.layout`: circuits and studios fitted to a room), the UK's inclusive
  transfer space and IFI counts are not built.
- **[gap]** The equipment's circuits, heat gains, outdoor air and floor loads do not yet feed the
  electrical, energy and structural tools; anchors are not modelled.
- **[gap]** The fan-light's light kit is not aggregated under the fan; fan-to-wall clearance is not
  checked.
