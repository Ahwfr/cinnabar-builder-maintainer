# Cinnamatica 0.3.31 user checks

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
7. **Paste:** Test setblock/fill and structure modes with the required operator permissions.
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
