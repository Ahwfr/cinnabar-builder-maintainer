# Cinnamatica Structure Paste

Open **M → Controls** and choose **Setblock** or **Structure** beside **Paste**. Click **Paste** and confirm. Reopen Controls and choose **Cancel Paste** to stop sending new sections. Sections the host has received may already be placed.

Both modes need Creative and operator commands. **Replace wrong blocks** updates occupied cells whose block or placement state differs from the schematic. Correct blocks and cells where the schematic is empty are preserved. With it off, only empty destinations are filled.

Cinnamatica-managed Bedrock Dedicated Server worlds enable the bundled behavior pack when opened with cheats enabled. If you turn on cheats after opening, leave and reopen the world once. In another player's world, the host must activate this behavior pack. If the world uses a dedicated server, allow only `@minecraft/server` in the script-module config `config/cd8f1a22-ebd0-4d22-8bbc-a41cad532380/permissions.json`. Run with an account that can use `/scriptevent`. If the host can't activate the pack, select **Setblock**.

The host constructs temporary native Bedrock structures and places them in sections. The destination chunks must be loaded. This uses the block states in the schematic library. Inventories, entities and other block-entity data are outside that library format.
