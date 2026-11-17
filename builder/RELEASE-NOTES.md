# Cinnamatica 0.3.23

- Send Structure Paste fragments within the Bedrock command packet limit. Schedule section preparation as bounded server jobs and cancel queued sections when the user cancels.
- Check Setblock replacement against the actual server block, and keep paste feedback visible.
- Target the floor under V2 doors and select real support faces suited to upper slabs and upside-down stairs. Unsupported upper placements remain subject to stock Bedrock placement rules.

## 0.3.22

- Choose Setblock or Structure under Controls. Structure converts the guide into native Bedrock templates and loads sections on the host. Cinnamatica-managed Bedrock worlds activate the host pack at startup.
- Replace wrong blocks preserves correct blocks and leaves schematic air alone. An active paste offers Cancel Paste.
- Improve Fast Place for doors bordered by partial blocks. Change ghost opacity on the retained preview and keep drawing it through temporary rendering budget failures.

## 0.3.21

- Count the visible Materials rows across frames so opening the tab stays within the mod callback budget.
- Resolve door items for Fast Place and let either ghost half select the lower placement cell.
- Correct the predicted facing of fence gates. Try direct V2 air clicks for gates and bottom trapdoors when the surrounding support cannot produce the planned state. Upper slabs, upside-down stairs, and upper trapdoors use a real support face because stock Bedrock places their lower variant on an air click.

## 0.3.20

- Bring ThsSilentKnight’s native Ore UI window to Windows, with the same Materials, Import, Settings and Controls tabs, search, placement tools and two-click deletion as Linux.
- Retire the separate legacy panels and shortcuts. M opens the unified window by default. Game controls remain editable in Keyboard & Mouse.
- Cache material lookups and send only changed panel values to keep large schematics responsive.

## 0.3.19

- Add a Placement lock toggle under Cinnamatica Settings. With Fast Place enabled and the guide visible, it prevents ordinary block placement from falling through to an unrelated cell. V1 and V2 retain their separate placement behavior.
- Predict fence gate placement and let Bedrock resolve the open and in-wall states.
- Keep the previous ghost mesh on screen until the replacement is ready to draw after a block edit.

## 0.3.18

- Publish the entire textured guide in a single retained native mesh. Local world comparisons still update individual cells.
- Retry opening menus in fresh callbacks when the frame that prepares a panel is time-sliced.
- Dim storage slots that cannot supply needed schematic materials. Matching item stacks and shulkers with matching contents stay bright.

## 0.3.17

- Restore conservative ghost refresh scheduling and keep visible tiles during temporary terrain read failures. Large schematics use smaller tiles in a bounded window around the player. Dense mismatches show a limited number of 3D markers per tile, while the verifier still lists every scanned mismatch.
- Keep P and the default M shortcut available if the client cannot supply the menu binding.
- Preserve crosshair-only Hold to Place from 0.3.16.

## 0.3.16

- Open the schematic window on the P key press. A press during schematic loading is queued.
- Hold to Place requires a ghost directly under the crosshair and stops at solid terrain. Nearby placement assist remains available with Hold to Place off.
- Prepare the ghost palette faster and overlap terrain reads. Keep the last targeted block under review when it breaks, so the mismatch can clear even when the crosshair moves to the block behind it.

## 0.3.15

- Brought ThsSilentKnight’s tabbed window to Windows. P and M open the shared window. F6 opens its Settings tab during play. The Controls tab manages placement, layers, the verifier, and paste.
- Fixed an unreachable fuel check for container material highlights. Needed stacks have green outlines, and shulker boxes with matching contents have cyan outlines when the server supplies their item data.
- Added placement prediction for leaves, observers, chests, and several redstone parts. Fast Place ignores piston heads and moving piston blocks.
- Prioritized tiles near the camera and the current target. Ghosts appear while terrain is still loading and existing tile meshes are edited when their block coverage changes.

## 0.3.14

- Renamed the app, launcher, bundled mod and packaging to Cinnamatica. Existing worlds and schematic settings use the same profile.

## 0.3.13

- Highlight needed materials in open containers. Green outlines mark matching stacks and cyan marks shulker boxes whose item data lists a needed block.
- Target Info Off clears the schematic HUD readout. Fast Place ignores auto-connected fence arms, places trapdoors closed before they are opened by hand, and stops substituting different logs for planned logs.
- The import picker identifies Bedrock structure-block exports alongside Java schematic files.

## 0.3.12

- Added Hold to Place and remappable camera-relative numpad movement and rotation. The Controls panel remains available.
- Added Destroy Overlaps on Paste, separate per-schematic local-world positions, and an Auto Next Layer switch.
- Target Info Off hides the Fast Place target coordinates. Bridge Assist is now Nearby Placement Assist.
- V2 tries direct air placement for normal stairs when there is no direct native support hit. Upper slabs and upside-down stairs use a support face.
- Organized Builder build entry points by platform and preserved ThsSilentKnight’s Linux port.

## 0.3.11

- V2 Strict can attempt to fill an empty schematic cell within reach when there is no usable support face. It reports the actual air runtime ID in a normal block-placement transaction. Bedrock confirms the block and Survival inventory update.
- Verified a direct air placement on stock Bedrock 1.26.60.6 in Survival with cheats disabled. Also verified opposite stair facings. Upside-down stairs, upper slabs and horizontal pillars use real support because the direct air click cannot be relied on to preserve the placement face. Server acceptance can vary.

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
- Added Remember local world placement in Cinnamatica Settings. It restores the active library entry and its coordinates, rotation and mirror per local world and dimension.

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

- Added a portable Cinnamatica.exe entry point with a persistent profile and first-run asset preparation.
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
