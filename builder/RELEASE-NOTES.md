# Cinnabar Builder 0.3.5

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
