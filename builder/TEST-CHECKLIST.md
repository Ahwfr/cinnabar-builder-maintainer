# Cinnamatica 0.3.34 merge checks

Passed in the 0.3.34 test copy on November 19. Keep them for later upstream merges.

M1. **First launch:** the ownership check accepts your account; the menu, skins and worlds load.
M2. **Local world:** create and open a local world; it starts and saves.
M3. **Hold to Place:** with logs, stripped logs, slabs and stairs, hold right-click along a guide. It should be as smooth as before 0.3.33, with correct orientation.
M4. **Fast Place V1/V2, Placement Lock, Auto Orientation, plants, doors, trapdoors:** as in 0.3.32.
M5. **Paste:** structure paste and grid paste in a local world, including cancel, and that no sections are skipped on a large schematic.
M6. **Export:** Export a schematic, then load it with a structure block by name; a schematic wider than 64 saves as parts.
M6b. **Ghosts:** textured ghosts, outlines on partial blocks (stairs, slabs, glass, water), lightness, layers, verification colours.
M7. **Window:** M opens it on release; Materials, Import, Settings, Controls tabs; per-world placement memory.
M8. **Stick tool:** T1 to T12 below, especially Ctrl+scroll and Alt+scroll.
M9. **General:** Creative block breaking speed, chat, inventory, servers you play on.

# Cinnamatica 0.3.33 tool item checks

Reply with each number and **works / still broken**. None of these has been run in game yet.

T1. **Menu key:** M opens the window when released. Holding M and pressing T, R or G must not open chat, reload shaders or the window.
T2. **Toggle:** M+T shows "Tool item off", then "Tool item on". The stick then shows the Tool line on the HUD; another item restores normal clicks.
T3. **Clicks and wheel withheld:** With the stick, left/right/middle click must not break, place or pick blocks. Ctrl+scroll and Alt+scroll must not change the hotbar slot; plain scrolling still does.
T4. **Modes:** Ctrl+scroll cycles all nine modes in both directions; the HUD shows "Tool n/9".
T5. **Area Selection:** Left/right click set the corners, drawn red and blue inside a white box. Alt+scroll moves the box; Ctrl+Alt+S copies it into the guide.
T6. **Placement:** In Schematic Placement and Paste, left/right click move the guide next to the aimed block; Alt+scroll nudges it.
T7. **Execute:** Bind Cinnamatica Execute Operation. Paste pastes once (Ctrl+M switches keep/replace). Grid Paste pastes the chosen grid (Ctrl+M cycles it).
T8. **Fill/Replace/Delete:** Alt+middle picks the fill block, Shift+middle the block to replace; Execute fills, replaces or clears the box. Try a box larger than 32 blocks.
T9. **Move:** Alt+scroll moves the selected blocks one step; the selection follows. A partly unloaded area must be refused.
T10. **Rebuild:** Without the stick, left-click removes the aimed guide block (both door halves), right-click with a block sets it. Materials should follow.
T11. **Info and layers:** Hold I over a ghost to read it. Page Up/Down change the layer; M+Page Up/Down change the layer mode.
T12. **Regression:** Without the stick, Fast Place, Hold to Place, Placement Lock, paste modes and per-world placement memory still work.

# Cinnamatica 0.3.32 user checks (still open)

Reply with each number and **works / still broken**, plus the item/mode if it fails.
These are acceptance checks, not claims that every item has been fixed.

1. **Common plants:** With Fast Place on, place poppy/dandelion, short grass and fern
   on grass or dirt. Test Creative and Survival with the required item available.
   Also try replacing natural short grass with a flower; stone should be refused.
2. **Saplings:** Place oak/spruce/birch (and the other normal saplings) on valid soil.
   Confirm Survival uses an item and does not overwrite the same correct sapling repeatedly.
3. **Tall plants:** Place tall grass and sunflower/rose bush/peony from either ghost
   half. One item should place both halves. A solid block above must prevent placement.
4. **Trapdoor state:** With Auto Orientation on, test a supported wooden trapdoor that
   should be open and one that should be closed. It should settle once, not repeatedly
   toggle, and should not place an unrelated held block. Test standing first; sneaking
   with an item normally bypasses block interaction and must not place an accidental block.
5. **Placement Lock:** On and off, try a correct block at a guide, in an air gap, and
   outside the build. Release the assisted hold before normal outside placement.
   Check a pane/fence/wall as well as a full block.
6. **Materials:** Open and scroll Materials immediately after loading a large guide.
   The menu and guide should remain present without a mod reload or whole-guide flash.
7. **Paste and Export:** Test structure paste and structure export with the required operator permissions.
   Completed areas should lose ghosts while Paste is still running; cancellation
   should stop new work and leave completed blocks intact. Check nearby and distant areas.
8. **Graphics/startup:** If the saved Vulkan failure notice appears, it should say it
   refers to a previous launch and explain the Video setting. DX12 should still work.
9. **Regression checks:** Hold to Place speed, logs/stripped-log orientation and
   lantern placement should still work as confirmed in 0.3.30.
10. **Unexpected exits:** Report any crash/menu reset, what you were doing, and whether
    a report appears. The earlier exit without a report has not been diagnosed.

## Still unresolved / outside this fix

- Unsupported upside-down stairs, upper slabs and upper trapdoor air placement still
  need a real suitable support on stock Bedrock. Do not count this build as a fix for #4.
- Plant support is an explicit common-soil subset. Specialized crops, mushrooms,
  aquatic plants and other species still need their own validated rules.
- Client-only headless checks do not prove server acceptance or real GPU frame pacing.
