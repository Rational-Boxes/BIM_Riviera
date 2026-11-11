# A public pool checked against the codes: building it by hand

How to build `pool_rules.ifc` in the window and check it. Each step names the UI path; **[gap]**
marks a step the window cannot do yet, and the [gaps](#gaps) section lists them. The script
`pool_rules.lua` is the reference: the same commands with exact values.

Start with **File › New**.

1. **The rules.** In **Manage › Model Properties** choose the *2010 ADA Standards* as the
   accessibility rule set and, under *Pool rules*, tick *CDC Model Aquatic Health Code*, *Virginia
   Graeme Baker Pool and Spa Safety Act* and *2010 ADA Standards: swimming pools*. (Under the IBC,
   the automatic choice is the ISPSC and VGB packs; the MAHC is a project's choice.)
2. **The deck.** Draw a floor 32 × 20 m from (0, 0) with **Architecture › Build › Floor**.
3. **The enclosure.** Make a railing type *Pool enclosure 6 ft* 1.85 m high, its bottom rail
   50 mm up and 25 mm balusters at 120 mm (`railing_type.create` from a script **[gap]**: the
   window has no railing type editor yet). Draw it round the deck with **Architecture › Build ›
   Railing**, closed, from (0.3, 0.3) to (31.7, 19.7), and add a gate in its south side with Modify | Railings › **Gate**,
   swinging out, away from the pool (**[gap]** its swing is set from a script,
   `swing_out = true`). Then select the gate and choose Modify | Railings › **Gate Hardware**: self-closing, self-latching and lockable, its latch 1.38 m (54.3 in) up. **View › Coordination › Pool Barriers** shows it enclosing the pool; the
   MAHC asks for 6 ft (4.8.6).
4. **The pool.** Choose **Architecture › Build › Swimming Pool**, *Typed outline*, and type the
   outline `6,15; 6,5; 26,5; 26,15` (its first edge, on x = 6, is the shallow end) and the depths
   `0:0.9, 8:1.524, 11:2.5`. The floor falls 1:12.8 to the 5 ft break and 1:3.1 beyond it.
5. **Depth markings.** Select the pool and choose **Architecture › Build › Pool Depth Markings**:
   markers in feet and inches every foot of depth to 5 ft (the MAHC's step; the ISPSC asks for
   2 ft), at most 25 ft apart, with NO DIVING to 5 ft; and the break-point band 8 m along the pool,
   where the floor reaches 5 ft, with the float line 1 ft shallower.
6. **Built-in entries.** Select the pool and choose **Architecture › Build › Pool Entry**:
   *Stairs*, then click by the shallow end's wall at (6, 8): five equal risers of 210 mm on 12 in
   treads. Again, *Sloped entry (ADA)*, at (6, 13): a 1:12 ramp 10 m long to 27 in below the
   water.
7. **Ladders and a lift.** From the *Riviera Pools* library (**Insert › Component Libraries**),
   place the *Pool ladder, 3 steps* on the north wall's face at (24.5, 15) and, turned 180°, on
   the south wall's at (24.5, 5); and the *Pool lift, 300 lb* turned 180° at (10, 4.4), facing
   the water.
8. **Drains.** Place two *Main drain cover 300 × 300* from the library at (24, 9) and (24, 11), 2.65 m down (the
   floor of the deep end): two blockable drains, 2 m apart.
9. **Check.** **Coordination › Check Pools** lists each pool's entries, accessible means of entry
   and drains, its bather loads (the MAHC's 20 ft² of water a bather: 107), and any findings by
   pack and clause. This pool passes. Move one drain within 3 ft of the other, or make the
   shallow slope steeper than 1:12 with `pool.set`, and the check names the rule.

## Gaps

- **[gap]** Recessed steps, swimouts, beach entries and handrails are not built yet; ladders, lifts
  and drains are library items, and the checks count them by type.
- **[gap]** Depth markings, NO DIVING markers, the break-point line and float line, barriers and
  gates, the diving envelopes and the MAHC's "within 10 ft of each end" placement of entries are
  not checked yet.
- **[gap]** Editing a pool's profile in the window (`pool.set` from a script) comes later.
