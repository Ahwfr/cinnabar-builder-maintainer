# Builder guide

Choose the [Windows](windows/README.md) or [Linux](linux/README.md) instructions to install or build the client. See [iOS](ios/README.md) and [Android](android/README.md) for port status.

## Use a schematic

Press **M**, choose **Import schematic…**, then select a file. Builder supports .schematic, .schem, .litematic, and .mcstructure. Imports add to the library. Choose an entry to load it, then use the Controls panel or numpad to move it.

**Fast Place V1 Classic** uses real support. **V2 Strict** checks the planned block state and can use direct air placement where Bedrock permits it. Upper slabs and upside-down stairs need a support face. **Nearby Placement Assist** helps find a reachable face when the crosshair misses. **Hold to Place** repeats Fast Place while you hold the use control.

**Paste build** is a Creative operator action. In Settings, **Destroy Overlaps on Paste** clears existing blocks at cells occupied by the schematic. With the setting off, paste keeps existing blocks.

**Auto Next Layer** works in Single Layer mode. **Remember World Positions** stores each schematic’s position, rotation, and mirror per local world and dimension. Edit the schematic keybinds in **Settings → Keyboard & Mouse**. The Controls panel stays available.

See [UPDATE.md](UPDATE.md) and [RELEASE-NOTES.md](RELEASE-NOTES.md).
