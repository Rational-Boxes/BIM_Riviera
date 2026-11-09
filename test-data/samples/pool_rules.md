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
3. **The pool.** Choose **Architecture › Build › Swimming Pool**, *Typed outline*, and type the
   outline `6,15; 6,5; 26,5; 26,15` (its first edge, on x = 6, is the shallow end) and the depths
   `0:0.9, 8:1.524, 11:2.5`. The floor falls 1:12.8 to the 5 ft break and 1:3.1 beyond it.
4. **Built-in entries.** Select the pool and choose **Architecture › Build › Pool Entry**:
   *Stairs*, then click by the shallow end's wall at (6, 8): five equal risers of 210 mm on 12 in
   treads. Again, *Sloped entry (ADA)*, at (6, 13): a 1:12 ramp 10 m long to 27 in below the
   water.
5. **Ladders and a lift.** From the *Riviera Pools* library (**Insert › Component Libraries**),
   place the *Pool ladder, 3 steps* on the north wall's face at (24.5, 15) and, turned 180°, on
   the south wall's at (24.5, 5); and the *Pool lift, 300 lb* turned 180° at (10, 4.4), facing
   the water.
6. **Drains.** Place two *Main drain cover 300 × 300* from the library at (24, 9) and (24, 11), 2.65 m down (the
   floor of the deep end): two blockable drains, 2 m apart.
7. **Check.** **Coordination › Check Pools** lists each pool's entries, accessible means of entry
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
