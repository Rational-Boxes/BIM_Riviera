# Self-checkout pods: building it by hand

How to build `sco_pods.ifc` in the window. Each step names the UI path; **[gap]** marks a step
the window cannot do yet, and the [gaps](#gaps) section lists them. The script `sco_pods.lua` is
the reference: the same commands with exact values.

Start with **File › New**. In **Manage › Model Properties**:
- set the **Building code** to the *IBC* and the **Accessibility rule set** to *2010 ADA Standards*;
- under **Retail rules**, tick *IBC Group M: aisles* and *Store planning practice*.

1. **A pod of pedestals.** In a plan, choose **Architecture › Build › Self-checkout Pod**, keep
   the pedestal self-checkout, set 6 units and keep the 11 ft courtyard, the 1 ft spacing and
   *Attendant station* ticked. Click (4, 4) for the pod's open end, then a point north of it. Three
   units face three across the courtyard, the attendant station stands at the closed end facing
   the entry, and the shared queue reaches 3 m south of the entry. Table 227.2 asks for 2
   accessible units of 6: units 1 and 4, one in each row.
2. **A quad pod of lanes.** Choose **Self-checkout Pod** again: *Self-checkout lane*, 4 units,
   a 12 ft courtyard, from (12, 4) toward the north. One unit is accessible.
3. **State the accessible units' features.** Select the three accessible units and choose
   **Architecture › Build › Self-checkout Profile**: operable parts from 800 to 1150 mm, a
   forward approach reaching 200 mm over the deck, the screen at 1300 mm, and tick *Speech*,
   *Tactile keypad* and *Braille*.
4. **Check.** **View › Coordination › Check Self-checkouts** lists the two pods, each with its
   attendant and its accessible units meeting the profile, and finds nothing. Put a tall display
   in the courtyard and check again: it is a note that the display stands between the attendant
   and a screen. Clear the profile of a unit and its accessible units are advisory findings: the
   ADA has no rule for self-checkout yet, so the profile (EN 301 549 cl. 8.3 and the Access
   Board's 2022 ANPRM) is advice.
5. **Count the banks.** **Check Checkouts** counts each pod as its own self-checkout bank, Table
   227.2 met; **Check Rules** finds the 36 in in front of each accessible unit clear.

## Gaps

- **[gap]** The unit is one furniture element: its terminal, scanner and display are not separate
  parts, and the service clearances are not `IfcBuildingElementProxy` PROVISIONFORSPACE volumes.
- **[gap]** The shared queue is a rectangle stored with the pod, not an `IfcSpatialZone` QUEUE, and a
  serpentine queue is not drawn; the units' circuits come later with the front end's.
