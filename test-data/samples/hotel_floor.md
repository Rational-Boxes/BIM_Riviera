# A hotel floor: building it by hand

How to build `hotel_floor.ifc` in the window. Each step names the UI path; **[gap]** marks a step
the window cannot do yet, and the [gaps](#gaps) section lists them. The script `hotel_floor.lua`
is the reference: the same commands with exact values.

Start with **File › New**. In **Manage › Model Properties**:
- set the **Building code** to the *IBC* and the **Accessibility rule set** to *2010 ADA Standards*;
- under **Interiors rules**, also tick *Interior planning practice*.

1. **The walls.** Draw 200 mm walls, 2.7 m high: the corridor wall from (−0.1, 0) to (23.5, 0) and
   the outer wall from (−0.1, 5.2) to (23.5, 5.2). Their room faces are at y = 0.1 and 5.1. Add
   partitions from y = 0.1 to 5.1 at x = 0, 3.8, 7.6, 11.8, 15.8, 19.6 and 23.4: six rooms 5.0 m
   deep and 3.6, 3.6, 4.0, 3.8, 3.6 and 3.6 m wide between the partitions' faces.
2. **The kings.** Choose **Architecture › Build › Room Layout**, pick *Hotel room, king*, select
   the outer wall as the head wall **[gap]**, and click inside the first room. The bed, two
   nightstands, the dresser, the desk and its chair and a luggage rack are placed. The room is
   placed and marked as a guest room (class king, one bed). Repeat in rooms 2, 5 and 6.
3. **The double queen.** In room 3 pick *Hotel room, two queens*: two queen beds with a nightstand
   between them.
4. **The accessible king.** In room 4 pick *Hotel room, accessible king*. Its bed keeps a
   30 × 48 in clear space on each side (§806.2.3), so it needs a 5.0 m deep room. The room is
   marked with mobility features. The standard rooms' pieces are marked not accessible, so they
   have no ADA bed spaces.
5. **Communication features.** Select the rooms of rooms 1, 3 and 4 (their room tags) and choose
   **Architecture › Build › Guest Room**. Tick *Communication* and keep the other values.
6. **Check.** **View › Coordination › Room Matrix** counts six rooms: one with mobility features
   (Table 224.2 asks one for 1–25 rooms), and two with communication features that count (Table
   224.4 asks two). The accessible room has communication features too, as §224.5 asks, but it
   does not count toward the two: only a tenth of the mobility rooms may. **View › Coordination ›
   Check Rules** finds nothing.

## Gaps

- **[gap]** Room Layout always puts the main piece against one of the room's longer walls; the
  script names the outer wall. Here the rooms are deeper than wide, so the window picks a
  partition and the layout does not fit; draw the rooms wider than deep, or use the script.
- **[gap]** Guest-room doors, bathrooms (lay them out with Bathroom Layout in their own rooms) and
  the corridor are not part of the templates, and guest rooms are not yet modular units
  (ARC-GRP-004): each room is laid out on its own.
