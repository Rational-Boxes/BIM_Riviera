# An atrium hotel with a roof pool

A 60 × 40 m hotel round a five-storey atrium, over a concrete basement with a sump: guest and
service lifts in its corners; a lobby
with a covered drop-off, a ballroom, three conference rooms and a restaurant on the ground floor;
a gym and a spa on the second floor; 88 guest rooms on four floors from two room units; and a pool
on a raised podium on the roof under the atrium's glazed roof, with sun-control baffles, solar
panels on the open roof, and a site with its drive. The
script `hotel.lua` builds it; these are the same steps in the window.

1. **Levels.** Rename the first level *Ground*. Add *Level 2* at 5.0 m, *Level 3* to *Level 5*
   3.5 m apart, *Roof* at 19.0 m, *Pool podium* at 20.8 m and *Basement* at −4.0 m
   (**Architecture › Datum › Level**).
2. **Slabs and façade.** On each floor and the roof draw a 60 × 40 m floor (**Floor**). Draw
   the four façade walls on each floor to the underside of the slab above: 4.7 m on the ground
   floor, 3.2 m above it.
3. **The frame** (**Structure**). Load the W14X90 column and the W24X76 and W24X84 beams. Place
   columns one storey at a time at (0.4, 10), (20, 10), (32, 10), (40, 10), (59.6, 10) and the
   same at y = 30, each to 0.3 m under the slab above; and full-height columns outside the
   façades at x = 8, 20, 32, 40 and 52, at y = −0.35 and 40.35, between the balconies. Draw
   girders along y = 10 and y = 30 between the columns under each floor, 0.3 m down, W24X84 under
   the roof. None crosses the atrium, and the ballroom and conference rooms are left column-free.
4. **Lifts** (**Architecture › Lifts & Transport › Elevator**). In each corner, a passenger lift from
   the ground floor to the roof, its landing turned toward the building: at (2, 4) and (2, 36)
   turned −90°, at (58, 4) and (58, 36) turned 90°. A freight lift beside the south-east one at
   (54.85, 4) and the north-west one at (5.15, 36). All six run from the basement.
5. **The atrium** (**Interiors › Glazed Spaces › Atrium**). Top level *Roof*, a ridge roof 6 m above
   it; click the void's corners (18.5, 10.5) and (41.5, 29.5) on the ground floor. Every floor
   above is opened and guarded 42 in high. To reach the roof over the pool and the patio, give
   the roof's outline from (18.2, 10.2) to (58.6, 37.8) (`roof_outline` in a script). The report
   gives six storeys open, 25 m to the glazing, the IBC 404 requirements, and the smoke exhaust
   that holds the layer 6 ft above the roof terrace (about 156 m³/s for a 5 MW fire).
6. **The basement.** Duplicate the 300 mm floor type as *Concrete Slab 300* and a wall type as
   *Concrete Retaining Wall 300*, each one concrete layer (**Edit Type › Structure**). Draw the
   basement slab from (0.15, 0.15) to (59.85, 39.85), to the walls' inner faces, and four
   retaining walls on the building's outline from 0.3 m below the level, 4.0 m high. **Structure ›
   Foundation › Wall** puts strip footings under them; they meet once at each corner. Pad
   footings under the six interior column lines, their tops 0.3 m down at the slab's underside.
   Partitions at x = 14 and 46 and y = 20 make the mechanical plant, laundry, storage and the
   electrical and water room; place a chiller, a boiler and a water heater in the plant room.
   **Systems › Plumbing › Sump**, clicking (11.5, 27) in the plant room: a 204 m drain outside the
   walls beside their footings, 2.04 L/s of groundwater at 0.01 L/s a metre (3.05 L/s design), a
   1.0 m pit 1.2 m deep holding the pumps to ten starts an hour, duty and standby 0.55 kW pumps at
   7.2 m, and the rising main under the ground floor and out to the storm drain.
7. **The ground floor.** Partitions 4.0 m high, under the girders: the long walls at x = 18
   and x = 42 the full depth; the ballroom's walls at y = 7.5 and 32.5 west of x = 18; the
   conference rooms' at y = 7.5, 15.8, 24.2 and 32.5 east of x = 42; the restaurant's south wall
   at y = 30.35, clear of the columns; and the front office's and administration's walls at
   x = 8 and 52. Doors: two in the façade under the canopy, three from the ballroom and two from
   each conference room into the atrium court, two from the restaurant. Place the rooms.
8. **Set the rooms out** (**Interiors › Rooms & Furniture › Seating Layout**, a room selected): the
   ballroom for a banquet (24 rounds of ten), Conference A as a theatre, B as a classroom, C as a
   boardroom, the restaurant for dining (30 four-tops). Each report gives the IBC occupant load,
   the exits it needs against the doors, and the egress width; none is short.
9. **The drop-off.** Two W10 posts at (21, −10.5) and (39, −10.5), standing on the forecourt
   0.3 m below the ground floor, and a flat roof 4.3 m up from (20, −11.5) to (40, −0.65):
   14 ft clear for coaches, under the balconies.
10. **Gym and spa (Level 2).** Walls round the west wing (x = 0–7.5) and the east wing
   (x = 52.5–60) between y = 7.5 and 32.5, with doors to the corridors. In the gym, **Gym
   Layout**: four treadmills, an elliptical, an upright bike and a rower facing east. In the spa
   draw a 2.4 × 2.0 m room and a 2.4 × 1.8 m room, then **Sauna** and **Steam Room** in them.
11. **Corridors.** On Levels 2–5, corridor walls at y = 7.5 and y = 32.5 from x = 8 to 52.
12. **The room unit.** Off to the side on Level 2, draw one guest room 4.0 × 7.5 m: its corridor
   wall with the entrance door, its façade wall with a window and a balcony door, two side walls,
   the room, a king bed and a desk, a *Supply Diffuser 600* 2.3 m up, and a 3 × 1.5 m balcony
   floor outside the façade with its railing. Select it all and choose **Architecture › Model ›
   Create Unit Type**: *King room*, 4.0 × 7.5 m, clicking the corridor wall's start as the bay's
   corner. Draw a second with the door further from the corner and make it *Accessible king*.
13. **Fill the floors** (**Unit Array**). *King room* on Levels 2–5, the right side of the line,
   corridor 2.5 m, first bay at 8 m, skipping 0–8 and 52–60, mirrored in pairs, *Accessible
   king* every 8th; click (0, 8.75) and (60, 8.75): 44 rooms 201–211 to 501–511 facing south.
   Again on the left of (0, 31.25)–(60, 31.25), numbered from 51: 44 rooms facing north. Each
   room's corridor and façade walls merge into the building's (its door and window are cut into
   them), neighbours share one demising wall, and each balcony is joined to its floor.
14. **Supply air.** On each guest floor draw a 400 mm supply duct down each corridor 2.9 m up
   (**Systems › Duct**, system type *Supply Air*). Select the eight mains and choose **Connect
   Units**: 88 branches, one into each room's diffuser.
15. **The roof pool.** On *Pool podium* draw a 15 × 18 m floor from (43, 11) to (58, 29) and a
   12 × 6 m pool from (45, 16) to (57, 22), 1.2 m deep falling to 1.35 m, with stairs at its west
   end and depth markings; a stair up from the terrace at (44.4, 32.4); a 4 ft pool fence round
   the podium with a self-closing gate latched at 54 in. **Pool Structure** weighs it (water
   885 kN, shell 807 kN); an analytical panel on the roof slab under the podium takes its loads.
16. **Sun control.** Set the location to Miami (25.8° N). **Interiors › Glazed Spaces › Baffles**:
   blades 5.6 m above the roof, running north–south, 0.25 m apart, 0.5 m deep, tilted 45°, from
   (18.5, 10.5) to (58.3, 37.5) under the whole glazed roof. **View › Coordination › Sun Check**
   with the pool selected, and the patio from (30, 30.5) to (42, 37.5): about a fifth of the
   direct sun reaches each, within the quarter allowed.
17. **The roof terrace's guard** round its edge.
18. **Roof plant and solar panels.** Place two *Air Handling Unit*s on the open roof at (9, 16)
   and (9, 24). **Systems › Electrical › PV Array**, clicking on the roof: 237 modules of 400 W in
   17 rows, 94.8 kWp, 4 ft from the edge and 1.2 m clear of the glazed roof, the units and the
   lift overruns.
19. **The site.** The ground in four toposolids round the building's footprint (the basement
   and lift pits stand in the hole): level 0.3 m below the ground floor for 16 m in front,
   falling 0.7 m to the street. **Site › Alignment**: the drive from (6, −49.5) north to (6, −6),
   east under the porte-cochère to (54, −6) and back south, with 10 m curves; its 7 m surface a
   sub-division of the ground 50 mm up, traced from the alignment's offsets. A car park east of
   the drive, a lawn to the north, trees either side, and the property line round the site.
20. **IFC.** Saved, the units are `IfcGroup`s of their rooms, walls, doors, windows, balconies and
   furniture, the atrium an `IfcSpace` with its shaft, guards and glazed `IfcRoof`, the baffles an
   `IfcShadingDevice`, the pool an `IfcElementAssembly`.
