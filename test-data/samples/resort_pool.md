# A resort pool with a beach entry: building it by hand

How to build `resort_pool.ifc` in the window. Each step names the UI path. **[gap]** marks a
step the window cannot do yet, and the [gaps](#gaps) section lists them. The script
`resort_pool.lua` is the reference: the same commands with exact values.

Start with **File › New**.

1. **The codes.** Choose the ADA in **Manage › Settings › Accessibility Rule Set**. **[gap]**
   Choosing the pool rule packs (the MAHC, the ISPSC, the VGB Act and the ADA's pool rules) is
   done from a script with `rules.set_packs`.
2. **The deck.** Draw a floor 34 × 22 m from (0, 0) with **Architecture › Build › Floor**.
3. **The enclosure.** Make a railing type *Pool enclosure 6 ft*: 1.85 m high, its bottom rail
   50 mm up, 25 mm balusters at 120 mm (**[gap]** `railing_type.create` from a script). Draw it
   round the deck with **Architecture › Build › Railing**, closed, from (0.3, 0.3) to
   (33.7, 21.7). Add a gate at (17, 0.3) with Modify | Railings › **Gate**; **[gap]** set it to
   swing out from a script (`swing_out = true`).
4. **The pool.** Choose **Architecture › Build › Swimming Pool**, type the outline
   `4.5,17; 4.5,5; 29.5,5; 29.5,17` and the depths `0:0.9, 14:0.9, 21.5:1.524, 24:2.0, 25:2.0`:
   0.9 m deep for 14 m from the west wall, 5 ft at 21.5 m (1:12), and 2.0 m from 24 m to the
   east wall. Add its markings with **Architecture › Build › Pool Depth Markings**.
5. **The beach.** **Architecture › Build › Pool Entry**: choose *Beach (zero-depth) entry*, type
   width 6, then click the west wall at (4.5, 11). The beach rises 1:12 from the floor to the
   deck. The water's edge is 1.8 m out from the wall, and the beach meets the floor 12.6 m out.
   A beach has no handrails unless you ask for them.
6. **The stairs.** **Pool Entry** again: *Stairs*, width 4.5, handrails 3, then click the south
   wall at (8.5, 5). There are five risers of 0.21 m. The stairs are wider than 5 ft, so the
   MAHC wants handrails on both sides, at most 12 ft apart: three handrails 0.914 m (36 in)
   above the nosings, 2.2 m apart.
7. **The sloped entry.** **Pool Entry**: *Sloped entry (ADA)*, then click the north wall at
   (22, 17). It runs 10.03 m at 1:12 to 27 in under the water. Its two handrails are 0.892 m
   (35 in) apart clear, inside ADA 1009.3.3's 33–38 in.
8. **The recessed steps.** **Pool Entry**: *Recessed steps with grab rails*, then click the east
   wall at (29.5, 11). There are four 18 in treads, each cut 5½ in into the wall. They are
   10 in apart, the top one 10 in below the deck. A grab rail each side stands 0.85 m above
   the deck, the two 21 in apart, curving over the coping into the water.
9. **The swimout.** **Pool Entry**: *Swimout (a seat in an alcove)*, then click the south wall
   at (21.5, 5). Its seat is 1.5 m wide and 0.45 m (18 in) under the water, in an alcove 0.6 m
   (24 in) out behind the wall. The shell wraps round it and the deck is cut round it. A 1½ in
   contrasting stripe marks the seat's leading edge, as one does each stair tread.
10. **The drains.** From **Insert › Component Libraries › Riviera Pools**, place two *Main drain
   cover 300 × 300* on the deep floor at (29, 10) and (29, 12), offset −2.15 m.
11. **Check it.** **Analyze › Check Pools**. The beach, the stairs and the recessed steps are
    entries, and the sloped entry is the accessible one. The swimout's depth, seat and stripe pass
    the ISPSC (411.5.1), and the treads' stripes the MAHC (4.5.4.2). The handrails, grab rails, treads and
    the beach's slope pass the MAHC, the ISPSC and the ADA. Try a 1:9 beach (both the MAHC's
    1:12 and the ISPSC's 1:10 fail) or one handrail on the stairs (the MAHC asks for both
    sides over 5 ft).

## Gaps

- **[gap]** Choosing pool rule packs, making a railing type and setting a gate's swing are done
  from scripts.
- **[gap]** The entries cannot be edited in the view once built. Change them with `pool.set`
  (its `entries`) or remove one with `pool.remove_entry` and build it again.
- **[gap]** The beach's trench drain at the waterline (MAHC 4.5.9) is not modelled.
- **[gap]** The coping does not run round a swimout's alcove.
