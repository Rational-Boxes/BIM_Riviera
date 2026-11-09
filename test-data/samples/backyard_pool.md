# A backyard pool: building it by hand

How to build `backyard_pool.ifc` in the window. Each step names the UI path; **[gap]** marks a
step the window cannot do yet, and the [gaps](#gaps) section lists them. The script
`backyard_pool.lua` is the reference: the same commands with exact values.

Start with **File › New**.

1. **The patio.** Draw a floor 16 × 12 m from (0, 0) with **Architecture › Build › Floor**.
2. **The pool.** Choose **Architecture › Build › Swimming Pool**, type the outline
   `3,8.5; 3,3.5; 13,3.5; 13,8.5` (its first edge, the west wall, is the shallow end) and the
   depths `0:1.0, 4:1.4, 5:1.524, 7:2.0`: 1.0 m (39 in) at the shallow end, falling 1:10 to
   1.4 m and 1:8 to the 5 ft break (the ISPSC allows 1:7), then to 2.0 m. **[gap]** Mark it residential with `pool.set{class = "residential"}` from a script:
   the window has no class field yet.
3. **Stairs and a bench.** Select the pool and choose **Architecture › Build › Pool Entry**:
   *Stairs* 1.8 m wide, clicking by the west wall at (3, 7.5); then *Underwater bench*, clicking
   by the south wall at (5.5, 3.5). The stairs step down 1.15 m in five equal risers of 230 mm
   on 12 in treads; the bench's seat is 0.45 m below the water.
4. **Ladder, drains and light.** From **Insert › Component Libraries › Riviera Pools**, place a
   *Pool ladder, 3 steps* on the north wall's face at (11.5, 8.5); two *Main drain cover 300 ×
   300* at (11.5, 5.25) and (11.5, 6.75), 2.15 m down on the deep floor; and an *Underwater
   light, large niche* on the east wall's face at (13, 6), turned −90° to face the water.
5. **Check.** **View › Coordination › Check Pools** checks it as a residential pool: the ISPSC's
   33–48 in shallow end, 1:7 at most to the transition, and 10 in stair treads. It passes; the
   public-pool rules (a 6 ft deck, two entries, the VGB Act's drains) do not apply to it.

## Gaps

- **[gap]** A pool's class is set from a script; the Swimming Pool dialog has no field for it.
- **[gap]** The ISPSC's barrier (a 48 in fence, self-closing gates, §305), the residential
  diving envelope (Table 804.1) and the NEC's bonding are not checked yet.
- **[gap]** Equipment is placed by coordinates, not snapped to the pool's edge.
