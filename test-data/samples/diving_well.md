# A diving well: building it by hand

How to build `diving_well.ifc` in the window. Each step names the UI path; **[gap]** marks a step
the window cannot do yet, and the [gaps](#gaps) section lists them. The script `diving_well.lua`
is the reference: the same commands with exact values.

Start with **File › New**.

1. **The rules.** In **Manage › Model Properties**, under *Pool rules*, tick *CDC Model Aquatic
   Health Code* and *Virginia Graeme Baker Pool and Spa Safety Act*.
2. **The deck.** Draw a floor from (−6, −5) to (20, 15) with **Architecture › Build › Floor**.
3. **The enclosure.** Make a railing type *Pool enclosure 6 ft* 1.85 m high, its bottom rail
   50 mm up and 25 mm balusters at 120 mm (`railing_type.create` from a script **[gap]**: the
   window has no railing type editor yet). Draw it round the deck with **Architecture › Build ›
   Railing**, closed, from (−5.7, −4.7) to (19.7, 14.7), and add a gate in its south side with Modify | Railings › **Gate**,
   swinging out, away from the pool (**[gap]** its swing is set from a script,
   `swing_out = true`). **View › Coordination › Pool Barriers** shows it enclosing the pool; the
   MAHC asks for 6 ft (4.8.6).
4. **The well.** Choose **Architecture › Build › Swimming Pool**, type the outline
   `0,10; 0,0; 14,0; 14,10` (the west wall first) and the depths `0:3.8, 8:3.8, 14:2.5`.
5. **Depth markings.** Select the well and choose **Architecture › Build › Pool Depth Markings**.
   It is deeper than 5 ft throughout, so there is no break-point band and no NO DIVING marker.
6. **The boards.** From **Insert › Component Libraries › Riviera Pools › Diving**, place two
   *Diving board 1 m, on stand*, turned 90° to point east, at (−0.54, 3.6) and (−0.54, 6.4): each
   tip is then 1.9 m over the water.
7. **Ladders and drains.** Place a *Pool ladder, 3 steps* on the north wall's face at (13, 10)
   and, turned 180°, on the south wall's at (13, 0); and two *Main drain cover 300 × 300* at
   (7, 4.25) and (7, 5.75), 3.95 m down.
8. **Check.** **View › Coordination › Check Pools** takes each board's plummet and its height
   above the water (1.0 m) and checks the MAHC's Table 4.8.2.3 column for a 1 m board:
   - A, back to the wall: 1.90 m (6 ft at least);
   - B, to the side walls: 3.60 m (10 ft); C, to the other plummet: 2.80 m (8.83 ft);
   - D, to the wall ahead: 12.10 m (29.58 ft); E, clear overhead: nothing over the boards;
   - H, the depth at the plummet: 3.80 m (12 ft); K, 16.5 ft ahead: 3.80 m (11.28 ft); M, 8.25 ft
     to each side: 3.80 m (11.63 ft).
   It passes. Move a board within 10 ft of a side wall, or make the well shallower with
   `pool.set`, and the check names the letter. Stairs or a bench built into the well within the
   envelope are reported too. **Check Rules** keeps 4 ft of deck clear beside and behind each
   board.

## Gaps

- **[gap]** Only 1 m springboards are in the library: 3 m boards and platforms, and their towers,
  are not built yet, though the tables have their columns.
- **[gap]** The envelope's clear space F and G (behind, beside and ahead, overhead) and the
  30° slope N are not checked; ceilings are checked over the plummet only (E).
- **[gap]** The residential diving envelopes (ISPSC Table 804.1) are not checked yet.
