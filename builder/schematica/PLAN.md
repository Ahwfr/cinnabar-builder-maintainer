# Builder implementation and acceptance plan

The fork keeps the upstream runtime and adds generic native capabilities used by the Schematica component.

| Feature | Current implementation | Acceptance |
| --- | --- | --- |
| Creative supplies | Confirmed background inventory window, catalog take to cursor, hotbar transfer, close, select and native placement | User confirmed Creative pickup and hotbar selection on cf3cd54 |
| Carried inventory | Main-inventory moves now open the server inventory and wait for confirmation before using the regular hotbar swap | Needs a new live Creative and Survival check |
| Orientation | Optional server-facing aim, bounded to four input ticks. If the support ray cannot also give the planned yaw, try a separate reported yaw and recheck the original support click. Camera pose stays local | Needs live server acceptance, especially opposite-facing stairs and stricter interaction validation |
| State mismatch | Matching block identifiers may place in their native orientation even if the planned state differs | Regression fixture updated. Native server acceptance still applies |
| Bridge assist | Selects a nearby missing ghost and searches for a real support within reach, including hidden faces when ordinary picks miss. Uses inventory in Survival and Creative | Hidden-face acceptance depends on server validation and needs a live check. Isolated ghosts need support. Bulk operator paste is pending. |
| Saved placements | Optional restore of the active library entry and transform for each launcher-selected local world and dimension | Remote servers have no placement restore. Live relog needs verification |
| Import | In-game Mods page runs the matching bundled converter on its worker and reloads enabled components | Needs native picker check from the packaged runtime |
| Guest upgrades | Portable launcher copies the old library payload into the new template, backs up the old component, and stamps the name and metadata version | Nested payload preservation regression and actual library smoke check |
| UI | Minecraft-style gray/green panels, button feedback, full viewport or compact mode | Requires rendered checks at small and large window sizes |
| Bindings | Schematica has normal Keyboard & Mouse rows. The guest observes their physical assignments | Remap and unbind regression. Legacy Alt+J/I/K remain |
| Packaging | One Cinnabar Builder.exe entry point beside runtime, template and persistent profile. Includes asset preparation kit and exact source ZIP | Needs extracted package startup check |

The UI uses the existing JSON-UI mod surface with Minecraft-style controls. Import and binding edits use native OreUI Settings. Every release keeps the same mod filename so its enabled state, permissions and settings can follow the upgraded code.

Remaining upstream parity work includes large schematic streaming, capture/export, complete Java-to-Bedrock state mappings and multi-block placement fixtures. Current import bounds remain documented in README. A successful compilation does not establish server placement acceptance.
