# Cinnamatica Structure Paste

Open **M → Controls** and click **Paste**, then confirm. Reopen Controls and choose **Cancel Paste** to stop sending new sections. Sections the host has received may already be placed.

Both modes need Creative and operator commands. **Replace wrong blocks** updates occupied cells whose block or placement state differs from the schematic. Correct blocks and cells where the schematic is empty are preserved. With it off, only empty destinations are filled.

Cinnamatica-managed Bedrock Dedicated Server worlds enable the bundled behavior pack when opened with cheats enabled. If you turn on cheats after opening, leave and reopen the world once. In another player's world, the host must activate the current Structure Host behavior pack. If the world uses a dedicated server, allow only `@minecraft/server` in the script-module config `config/cd8f1a22-ebd0-4d22-8bbc-a41cad532380/permissions.json`. Run with an account that can use `/scriptevent`. Without the pack there is no paste.

The host constructs temporary native Bedrock structures and places them in sections. The destination chunks must be loaded. This uses the block states in the schematic library. Inventories, entities and other block-entity data are outside that library format.

## Export for a structure block

Click **Export** beside Paste and confirm. The mod sends the guide, in its current rotation and mirror, to the host, which saves it as a structure named after the schematic (lowercase letters, digits and underscores, for example `my_house`). Nothing is placed in the world. Anyone in Creative with operator rights can then place a structure block, choose **Load**, type the name and load it, with the structure block's own offset, rotation, mirror and outline preview. A structure block loads at most 64 × 384 × 64, so a larger schematic is saved in parts named `name_x_z` (`name_x_y_z` when it is also taller than 384). Part `_1_0` goes 64 blocks east of part `_0_0`, and the second number counts south. Exporting again under the same name replaces the earlier structure.

Chests' contents, signs' text and other block-entity data are not carried, and blocks that are not in the vanilla `minecraft:` namespace are left out and counted in the chat message.
