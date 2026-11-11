# A backyard pool: building it by hand

How to build `backyard_pool.ifc` in the window. Each step names the UI path; **[gap]** marks a
step the window cannot do yet, and the [gaps](#gaps) section lists them. The script
`backyard_pool.lua` is the reference: the same commands with exact values.

Start with **File › New**.

1. **The patio.** Draw a floor 16 × 12 m from (0, 0) with **Architecture › Build › Floor**.
2. **The barrier.** Make a railing type *Pool fence 4 ft* 1.25 m high, its bottom rail 50 mm up
   and 25 mm balusters at 120 mm, so its rails are over 45 in apart and its gaps under 4 in
   (`railing_type.create` from a script **[gap]**). Draw it round the patio with **Architecture ›
   Build › Railing**, closed, from (0.3, 0.3) to (15.7, 11.7), and add a gate with Modify | Railings › **Gate**, swinging out, away from the pool
   (**[gap]** its swing is set from a script, `swing_out = true`), at (8, 0.3).
   **View › Coordination › Pool Barriers** shows it enclosing the pool (ISPSC 305).
3. **The pool.** Choose **Architecture › Build › Swimming Pool**, type the outline
   `3,8.5; 3,3.5; 13,3.5; 13,8.5` (its first edge, the west wall, is the shallow end) and the
   depths `0:1.0, 4:1.4, 5:1.524, 7:2.0`: 1.0 m (39 in) at the shallow end, falling 1:10 to
   1.4 m and 1:8 to the 5 ft break (the ISPSC allows 1:7), then to 2.0 m. **[gap]** Mark it residential with `pool.set{class = "residential"}` from a script:
   the window has no class field yet.
4. **Stairs and a bench.** Select the pool and choose **Architecture › Build › Pool Entry**:
   *Stairs* 1.8 m wide, clicking by the west wall at (3, 7.5); then *Underwater bench*, clicking
   by the south wall at (5.5, 3.5). The stairs step down 1.15 m in five equal risers of 230 mm
   on 12 in treads; the bench's seat is 0.45 m below the water.
5. **Ladder, drains and light.** From **Insert › Component Libraries › Riviera Pools**, place a
   *Pool ladder, 3 steps* on the north wall's face at (11.5, 8.5); two *Main drain cover 300 ×
   300* at (11.5, 5.25) and (11.5, 6.75), 2.15 m down on the deep floor; and an *Underwater
   light, large niche* on the east wall's face at (13, 6), turned −90° to face the water.
6. **Check.** **View › Coordination › Check Pools** checks it as a residential pool: the ISPSC's
   33–48 in shallow end, 1:7 at most to the transition, and 10 in stair treads. It passes; the
   public-pool rules (a 6 ft deck, two entries, the VGB Act's drains) do not apply to it.

7. **Electrics.** Place a *Receptacle* (**Systems › Electrical › Device**) standing free at
   (8, 11), 0.45 m up: 2.5 m (8.2 ft) from the pool, the receptacle a dwelling's pool needs
   6–20 ft from its inside wall (NEC 680.22(A)(1)). **View › Coordination › Pool Electrical
   Zones** asks for GFCI on it, finds the light 18 in under the water and on 12 V, and lists
   the ladder and the light for the bonding grid under the 3 ft perimeter surface.

## Gaps

- **[gap]** A pool's class is set from a script; the Swimming Pool dialog has no field for it.
- **[gap]** The residential diving envelope (Table 804.1) is not checked yet; gates' self-closing
  and self-latching hardware and latch heights are not modelled.
- **[gap]** Equipment is placed by coordinates, not snapped to the pool's edge.
