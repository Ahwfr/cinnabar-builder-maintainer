# Cinnabar Builder 0.3.11

- V2 Strict can attempt to fill an empty schematic cell within reach when there is no usable support face. It reports the actual air runtime ID in a normal block-placement transaction. Bedrock confirms the block and Survival inventory update.
- Verified a direct air placement on stock Bedrock 1.26.60.6 in Survival with cheats disabled. Server acceptance can vary.

## 0.3.10

- Put Paste build on the main schematic screen and in Settings.
- Both Fast Place modes look past the ghost enclosing the camera. Direct targeting can start before the preview finishes scanning.
- Auto Orientation checks the planned stair or slab half first, tries a yaw-only correction before searching for another face, and can use an alternate valid support after inventory selection.

## 0.3.9

- In-game imports add files to the current library and preserve its existing entries and placement settings.
- Fast Place cycles through Off, V1 Classic and V2 Strict. V2 aims beyond the ghost containing the eye and requires the predicted stair or slab half and orientation to match.
- Fast Place recognizes string as the item for the Bedrock tripwire block.
- Large guides use larger tiles to display more of the build at once. A Cardiff-sized guide fits within the retained tile budget.
- Single Layer mode advances after every required block on the current layer has been verified.
- Paste build uses the server's block commands in Creative when operator cheats are available. It requires the `setblock` and `fill` mod permissions and preserves occupied cells.

## 0.3.8

- Import larger schematics through the in-game picker. Blueprints can contain up to 100,000 cells, with the original file and decompression limits still in effect.
- Show cyan outlines on textured ghosts. Large builds show the nearby part of the guide as you move around it.
- Let Fast Place place walls and empty cauldrons. With a full hotbar, swap the selected stack into the needed item's inventory slot. In Creative, catalog placement can replace the selected stack and stores the displaced stack when inventory space is available.

## 0.3.7

- Update the WebAssembly runtime to Wasmtime 36.0.20. This includes fixes for [a Cranelift sandbox escape](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-897f-g5gm-fmxm) and [unbounded host allocation](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-6fr9-xmw9-5352).
- Update Rustls, WebPKI and its certificate roots, Tokio, event-listener and rustix to patched versions.
- Retire old command, WASI, and packet grants for the bundled Schematica component. Schematic imports are local and use the file selected in the picker.
- Preserve the schematic picker request through the normal mod frame so Import opens the native picker. Show conversion errors from the picker.
- Show a stronger blue tint on textured holograms. Preserve authored pane arms in previews and ignore neighbor-derived pane links in verification.
- Skip redundant equipment selection and let held use repeat assisted placement using the normal timing. Recognize Bedrock's downward bridging pick as an eligible schematic destination.

## 0.3.6

- Open the schematic file picker directly from Schematica.
- Fixed dashboard nudges and mirror. Mirror cycles through X, Z, and Off.
- Scroll materials on the dashboard with the wheel or arrow buttons. Layer controls now have plus and minus buttons. New keyboard defaults are `[` and `]`.
- Textured ghosts use a subtle blue tint and retain the selected opacity.

## 0.3.5

- Auto Orientation sends separate player-facing and support-click rotations. Stair corrections prefer angles and hit heights clear of the facing and half-block boundaries.
- Bridge Assist accounts for depth on both sides of the block under the crosshair and can skip a filled native destination to find a missing ghost beyond it.

## 0.3.4

- Bridge Assist searches up to five blocks off the crosshair and checks actual support reach before sending.
- Auto Orientation samples the upper and lower parts of hidden support faces for slabs and stairs and tolerates small camera movements during a pending correction.
- Added Remember local world placement in Schematica Settings. It restores the active library entry and its coordinates, rotation and mirror per local world and dimension.

## 0.3.3

- Bridge Assist looks for missing ghost blocks up to 1.6 blocks off the crosshair, within normal placement reach.
- If a visible or downward support pick fails, Bridge Assist can submit a click on a nearby hidden face of a real support block. The server decides whether to accept it.
- Auto Orientation can predict all four anvil facings and correct yaw using a nearby support face without moving the camera.

## 0.3.2

- Auto Orientation can report an opposite facing while retaining the support click. It checks the planned state before choosing this fallback and leaves the camera in place.
- Bridge Assist continues to use normal inventory placement with a real support. Live acceptance of the separate facing still needs verification.

## 0.3.1

- Added the downward support pick to Bridge Assist and Auto Orientation in Survival and Creative.
- Replaced single-block command placement with inventory-backed Bridge Assist.
- Included opposite-facing aim candidates when the support and orientation can be validated.

Bridge Assist still requires a real support block. In-world acceptance is pending.
Bulk operator paste remains separate future work.

## 0.3.0

- Added a portable Cinnabar Builder.exe entry point with a persistent profile and first-run asset preparation.
- Grouped support files under App so the play folder has one obvious executable.
- Added in-game schematic importing through the native Mods page, with library backups and enabled-mod reload.
- Added a browser layout with a schematic list, placement controls, build details and material preview.
- Added beveled buttons with eased hover feedback and a depressed pressed state.
- Fixed browser button text padding and sizing, including hover and pressed labels.
- Added Delete on every library row. Removals persist for that import, preserve original files, and reset on reimport.
- Added native textured inventory icons to the dashboard and material list, including isometric artwork for blocks.
- Added full screen and compact layouts, authored outside the WASM callback budget.
- Added Schematica rows to the normal Keyboard & Mouse settings.
- Added a shared release version and guest revision in the mod name, metadata and menu.
- Updated carried-inventory transfer to wait for a confirmed background inventory window.
- Removed the mandatory exact-orientation placement restriction.
- Retained optional server-facing orientation assistance without moving the camera.
- Added optional keep-only command placement for floating cells.

Air command placement needs server cheats/operator permission and the setblock mod grant. It does not consume inventory items. Ordinary Survival placement needs a real support. Orientation correction remains subject to server acceptance.

Focused importer, library, inventory and binding tests pass. Client test targets compile. Rendered UI checks cover compact/full screen layouts and button interaction states. Live server placement and the packaged import dialog still need confirmation on this release.
