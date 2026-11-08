# Store aisles: building it by hand

How to build `store_aisles.ifc` in the window. Each step names the UI path; **[gap]** marks a step
the window cannot do yet, and the [gaps](#gaps) section lists them. The script `store_aisles.lua`
is the reference: the same commands with exact values.

Start with **File › New**. In **Manage › Model Properties**:
- set the **Building code** to the *IBC* and the **Accessibility rule set** to *2010 ADA Standards*;
- under **Retail rules**, tick *IBC Group M: aisles* and *Store planning practice*.

1. **The walls.** Draw four 200 mm walls, 4 m high, round a 14 × 12 m floor: their inside faces
   at x = 0 and 14 and y = 0 and 12.
2. **A wall run.** In a plan, choose **Architecture › Build › Gondola Run**, pick *Wall*, *48 in*
   sections, a height of 78 in, and click the north wall's face from (1.0, 12.0) to
   (8.3152, 12.0): six sections facing into the store. 78 in is the most a wall upright on 16 in
   base shelves stands without anchoring (Madix); the default 84 in would be noted.
3. **Islands.** Choose **Gondola Run** again, pick *Island*, and click its centre line from
   (2.0, 9.0918) to (7.8166, 9.0918): four 48 in sections between two end caps, 35¾ in deep.
   Repeat at y = 6.2156 and 3.3394. Face to face, the aisles are 2.0 m.
4. **Check.** **View › Coordination › Check Rules** finds nothing: each aisle is wider than the
   ADA's 36 in route (§403.5.1), the IBC's 44 in Group M aisle and the 6 ft practice cart aisle,
   and 2 m beyond each end cap leaves the 48 in turn (§403.5.2). Move the middle island 1.2 m north,
   leaving a 0.8 m aisle: Check Rules reports it against each pack. Move it back.
5. **The store.** Place a room in the sales floor (**Architecture › Room**), select it and choose
   **Architecture › Build › Store Area** › *Sales floor*. Load the *Upright Sprinkler* starter
   type and place heads 3.9 m up on a 3 m grid from (1.5, 1.5) to (13.5, 10.5).
6. **Check the store.** **View › Coordination › Check Store** counts 31 occupants (168 m² at
   60 ft², 5.57 m², a person) and finds nothing: every gondola top is more than 18 in under the
   deflectors, and none is over its unanchored height.

## Gaps

- **[gap]** The aisle widths are not shown while the run is drawn; check them after.
- **[gap]** The occupant load does not yet feed the egress tools; fitting rooms and signs are not
  checked yet.
- **[gap]** Passing spaces on long aisles under 60 in (§403.5.3), aisles sized from the occupant
  load, and runs along a polyline come later.
