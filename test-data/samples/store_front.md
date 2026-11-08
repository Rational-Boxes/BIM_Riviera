# A store front: building it by hand

How to build `store_front.ifc` in the window. Each step names the UI path; **[gap]** marks a step
the window cannot do yet, and the [gaps](#gaps) section lists them. The script `store_front.lua`
is the reference: the same commands with exact values.

Start with **File › New**. In **Manage › Model Properties**:
- set the **Building code** to the *IBC* and the **Accessibility rule set** to *2010 ADA Standards*;
- under **Retail rules**, tick *IBC Group M: aisles* and *Store planning practice*.

1. **The main bank.** In a plan, choose **Architecture › Build › Checkout Bank**, keep the
   carousel checkstand, *Aisle on the right*, *Checkout* and an 84 in pitch, and leave the lane
   count empty. Click (0, 3) and (20, 3) for the line the exit ends stand on, then a point north
   of it. Nine lanes fit; lanes 2, 5 and 8 (from the west) are accessible, as Table 227.2 asks of
   nine, and each has a 36 in aisle beside it (§904.3.1).
2. **Express lanes.** Choose **Checkout Bank** again: *Tandem pairs*, *Express*, 5 lanes, from
   (21, 3) to (32, 3). Two of the five are accessible.
3. **Self-checkout.** Choose **Checkout Bank**: *Self-checkout lane*, 4 lanes at a pitch of
   89.5 in (the 1359 mm lane and a 36 in aisle), from (33, 3) to (43, 3). The bank is its own
   function: one accessible lane.
4. **Check.** **View › Coordination › Check Checkouts** lists the three functions and finds Table
   227.2 met. **Check Rules** finds the accessible aisles and the queues 3 m beyond each
   checkstand's in-feed clear.
5. **Estimate.** **View › Coordination › Checkout Lanes Estimate** with 300 customers an hour and
   20 items a basket at 900 items a lane-hour gives about eight lanes: an estimate from a
   queueing model, not a requirement.

## Gaps

- **[gap]** Queue zones are use zones, not `IfcSpatialZone` QUEUE in IFC; exit gates, the egress
  checks at the front end (half the exits reachable without passing the checkouts) and lane
  circuits come later.
- **[gap]** Self-checkout pods round a courtyard, attendant sightlines, the check-writing surface
  and the ISA sign are not made yet.
