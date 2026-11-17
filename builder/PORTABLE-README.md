# Cinnamatica

Extract the whole ZIP into a folder you can write to. Open **Cinnamatica.exe**.
Keep the **App/** folder beside it. It contains the required runtime, importer template and documentation. The first launch prepares Minecraft assets.

Your worlds, login, options and schematic library live in **profile/**.
For an update, close Builder and extract the new ZIP into the same folder.
Keep profile/. The launcher upgrades the library to the new build and backs up its old component.
The Mods page shows the guest build revision.

Press **M** for the schematic menu. **Import schematic…** opens the file picker.
Choose your Java or Bedrock files, then select a schematic in the library.
Enabled libraries reload automatically. Each import replaces the library selection and backs up the previous component.
Original input files stay at their original paths.
Click **Delete** twice to remove a schematic from the library and clear its active guide.
Original files and recovery backups are preserved. Reimport to restore a removed entry.
Materials show the native inventory artwork: isometric block textures or item sprites as appropriate.

**M** opens and closes the unified window. Use its **Materials**, **Import**,
**Settings** and **Controls** tabs. Edit the menu, rendering, layer and movement
bindings in Minecraft’s **Settings → Keyboard & Mouse**.

Turn **Fast Place** on to select carried blocks, move main-inventory blocks to the hotbar,
or obtain a Creative catalog block. Right-click a real support beside the ghost.
**Auto orientation** optionally adjusts the reported aim without moving your camera.
If a valid aim cannot produce the exact state, ordinary placement of the matching block can still proceed.

With **Bridge Assist** and **Auto Orientation** on, aim near the next missing ghost
beside a real block and right-click while standing still. Builder searches up to
five blocks off the crosshair and can try a hidden support face within normal
reach. In V2 Strict, aim at an isolated ghost to attempt an air placement
within reach. Survival consumes inventory, and exact orientation depends on
the block's placement rule. Paste build is a separate operator action in Creative.

Turn on **Remember local world placement** to reopen the active schematic at its saved position, rotation and mirror when returning to a local world and dimension. Unloading clears that saved placement. Reimporting a different library starts fresh.
Cinnamatica-Source.zip contains the editable fork source for this exact build.
