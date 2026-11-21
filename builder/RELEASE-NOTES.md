# Cinnamatica 0.3.52

- **Aiming passes through the thin part of a ghost.** Fast Place used to treat every ghost as a whole cube, so a ghost trapdoor, slab, pane, sign or fence stopped the aim even where you were looking past it. The aim now follows the block's real box for slabs, trapdoors, doors, fence gates, panes, iron bars, fences, walls, ladders, signs, buttons, torches, flat rails, carpets, pressure plates, repeaters, comparators, redstone dust, beds, lanterns and chests, so you can click under a trapdoor or past a pane to the ghost behind it. Other blocks still count as the whole cell. The boxes come from the game's own block table where it has them. Signs, buttons, torches, rails and chests are written from vanilla sizes, and a wall torch, a pane or fence saved with no connections, and every rail that climbs use a rough or generous box. It is one table inside the mod, so there is nothing to load per schematic.
- **Missing rails and chests get an outline.** A thin line is drawn around the real shape of a ghost rail or chest (a rail that climbs gets a rough half-block box), so they no longer disappear against the ground.
- **Why a placement was slow.** Fast Place now writes "slow placement" to the log when a block took more than a quarter of a second from the click to the packet, with whether it waited on an inventory swap, a turn of the view, or a bridge click. If hold-to-place is still slow, that line says where the time goes.

- **Not done:** the slow reappearance of ghosts and slow detection of wrong blocks. Your Play logs from today show the game turning a ghost around in about 30 ms, with two slow outliers, so I could not tell where seconds are lost without a log of the mod's own checks. Nothing was changed for it. The Fast Place failure in your friend's world has no `blocker=` line in your logs, so it is also unread. The structure export (0.3.47 and 0.3.48) is still untested.

Verified: the mod's unit tests (86, including new ones for the shape table, aiming past a slab, and the outline frame) and a compile check of the game crate for the log line. The mod's WASM file and the full client were not built here. Not tried in the game: everything above.

# Cinnamatica 0.3.51

- **The material list now shows the right numbers.** "Left" was wrong: it counted only the blocks the guide had already checked and found missing, so a block in a part of the guide that had not been checked yet showed 0 left. You had placed nothing and had nothing in your inventory, and a material with 6 in the schematic showed 0 where it should show 6. I told you in 0.3.50 that your screenshot was correct and that only the wording was the problem. That was wrong, and I am sorry. "Left" is now the schematic's total for that block minus the blocks the guide has checked and found placed as planned, so anything not yet checked counts as left, never as done. The "% built" line at the top uses the same rule, so it can no longer read higher than it should.

Verified: the mod's unit tests (79, including two new ones: nothing built shows the full total as left, and unchecked parts of the guide never count as built) and the OreUI window tests. The mod's WASM file was not built here. Not tried in the game.

# Cinnamatica 0.3.50

- **The material list says what it means.** The column that said "Needed" now says "Left", and a material with nothing left to place shows "Done" instead of 0. "Left" is how many of that block still have to be placed, and "Total" is how many the schematic has, so "Done" next to a total of 6 means all six are in the world. Your screenshot (soul sand 0 of 6, with 9,013 of 9,309 blocks built) was correct; the wording was the problem.
- **Material counts load faster and stay loaded.** A page's counts used to start again from nothing every time you changed page or typed in the search, and only four tiles were counted per frame, so many rows stayed on "…". Counts are now kept for every page you have seen, as long as the schematic and its placed totals have not changed, and as many tiles are counted per frame as the frame has fuel for. They start again when you place or break a block.

Verified: the mod's unit tests (78, including a rewritten test of the material counts over several frames, their memory and their restart) and the OreUI window tests. The mod's WASM file was not built here. Not tried in the game.

# Cinnamatica 0.3.49

- **Anvils now turn to the right side when Fast Place places them.** The client planned an anvil's facing as if it faced the player, but Bedrock turns an anvil a quarter turn to the left of the player, which is how the client's own prediction already places it. So the planned facing and the turn did not agree, and an anvil came out a quarter turn off. This is the one fix I am sure was a bug.
- **Barrels and command blocks (command, repeating and chain) now turn to face the player** when the guide plans a horizontal facing, the same way droppers and dispensers already did. Before, nothing turned the view for them, so their facing depended on where you happened to look. A barrel or command block planned to face up or down is still not turned, because that depends on where your eyes are compared with the block, not on which way you face.
- **Not changed, because the stairs' logic is mostly already there.** Furnaces, smokers, blast furnaces, chests, ender chests, trapped chests, fence gates, repeaters, comparators, carved pumpkins, stonecutters, lecterns, campfires, droppers, dispensers, observers and pistons already turn to the planned facing and can already be placed in the air. I checked the code for them and the new tests cover their quadrant table. I did not find a reason they would not work, but I have not seen it in the game.
- **Not done.** Grindstones, bells, beehives, bee nests, chiseled bookshelves, decorated pots, shelves, beds, rails, tripwire hooks and coral fans have no turning rule. I do not know their Bedrock facing values well enough to write them without testing, and a wrong table would make them wrong every time. Doing them properly needs the placement run against a real server that I offered, or you telling me what they do in your game.

Verified: the new test of every quadrant for the anvil, furnace, chest and similar families, the barrel and command blocks, and the existing orientation test; both pass, and the rest of the client compiles. The mod is unchanged since 0.3.48 and its tests were not re-run. Not tried in the game: any of the placements above.

# Cinnamatica 0.3.48

- **Export no longer blocks building.** In 0.3.47 an export counted as a paste in progress, which turned off Fast Place, the guide label and the stick tool until it finished. It now uploads in the background and none of those are affected. The Paste button waits while an export runs, and Export waits while a paste or its check runs, because both send commands and the host ends the mod if a frame sends too many.
- **Timing for ghost updates.** The client log now has, for each hologram update, `Hologram generation started` (how long it waited), `Hologram mesh built` (with the time since the update reached the host) and `Hologram generation shown` (the time since it reached the host, when it appeared). The guide label now ends with `ghosts waiting N` while ghost edits are queued in the mod. **These do not make anything faster.** You reported that ghosts go away quickly but come back slowly, all at once, and that wrong blocks are slow to show. The rebuild itself takes about 5 to 15 ms in your log, so the cause is probably in the mod's check cycle, which I have not confirmed. The label and the log are there to show it.

Verified: the mod's unit tests (78), the OreUI window tests (unchanged since 0.3.47), the render-model and mod-render tests, and the client compiles. Not tried in the game: the background export, the label, the log lines. The mod's WASM file was not built here.

# Cinnamatica 0.3.47

- **Setblock paste is gone.** Paste always uses Structure Paste now, and the Setblock/Structure switch is removed. Grid paste uses it too. This means paste needs the Structure Host addon in the world, which needs Creative and operator rights. On a server that does not have the addon there is no paste. The check that runs after a paste, and the Move tool, still use setblock and fill commands, so those permissions are still asked for.
- **New Export button**, beside Paste. It saves the guide, in its current rotation and mirror, as a structure in the world for a structure block's Load mode. Place a structure block, choose Load, and type the name (the schematic's name in lowercase letters, digits and underscores; the chat message tells you). Anyone in Creative with operator rights can then load it with the structure block's own offset, rotation, mirror and preview. A structure block loads at most 64 wide, so a wider schematic is saved as parts named `name_x_z` and the chat message says which. Exporting again under the same name replaces the earlier structure. Chest contents, sign text and other block-entity data are not carried, and blocks outside the vanilla namespace are left out and counted. Export is in both the JSON-UI window and OreUI.
- **Structure Paste no longer skips sections.** The addon used to throw away any section that arrived while eight were already being placed, and the whole section when one block in it could not be made. It now queues sections instead, and skips only the unsupported block, counting it in the chat message ("N unsupported blocks left out"). This is the cause I found for "N sections skipped". Whether it is the only cause is not known.
- The client log now says why Fast Place cannot open the Creative inventory (`blocker=...`). On a friend's server it was failing every attempt, and the log did not say why. This does not fix it. Send me the new `blocker=` text.

Verified: the mod's unit tests (78), including new ones for export names and for cutting a large schematic into parts, the OreUI window tests, the addon script's syntax, and the client compiles. Not tried in the game, and not tried on any server: Paste through the queue, Export and loading the result with a structure block (including that a structure saved as `mystructure:name` loads when typed as `name`), splitting into parts, and the addon on the server's script version (`@minecraft/server` 2.1.0). The mod's WASM file was not built here; the PC build is its first build.

# Cinnamatica 0.3.46

- Ghost chests and shulker boxes are tinted blue and a little darker, so you can tell them from real ones.
- A ghost chest or shulker box now disappears at once when the world has any block in its cell, and comes back when the cell is empty or water or lava again. Before, it waited for the whole hologram to be rebuilt and uploaded, which is why it stayed after you placed a chest.
- The client log now records how long each hologram update takes: a line `Hologram mesh built` (cells, sections, triangles, milliseconds) and a line `Hologram GPU arena built` (sections and milliseconds for the build, face and sort steps). **This does not make the hologram faster.** The slow load and the slow ghost removal are not fixed, and I do not yet know the cause. These lines are there to find it.

Verified: the hologram unit tests (7) and the block-entity render tests (161), and the client and render crates compile. The mod is unchanged since 0.3.45. Not tried in the game: the tint, the instant hide, and the log lines.

# Cinnamatica 0.3.45

- Chests, ender chests, copper chests and shulker boxes in the hologram are drawn as their real models instead of a plank or colored cube. Two chests side by side with the same kind and direction are drawn as one double chest, because that is what the game makes of them when they are placed. Shulker lids always face up and chest lids stay closed. Beds, signs, banners and heads are still stand-in cubes.
- A chest of the right kind now counts as built whichever way it faces, because it faces the player who places it. Before, a chest facing the other way left its ghost on screen.
- Placing or breaking a block in the hologram remeshes only the sections it can touch when the schematic has no leaves. Before it remeshed every section in nine columns. Schematics with leaves still remesh whole columns, because leaves read shelter from the full column. This should shorten the wait for a ghost to go away or come back; the wait has not been measured.
- Fast Place now sends the click for blocks the game has no support rule for (mangrove roots, and likewise mushrooms and similar) and lets the server decide. Before, it refused them with "could not predict this placement".
- The aim label now shows what the guide reads in the cell, for example `chest | -100, 73, 184 | world: empty | Hold right-click`. It reads `built`, `empty`, `other block`, `wrong state`, `extra` or `not loaded`. This is there to find out why a ghost stays or a chest is not seen.

Verified: the mod's unit tests (75 in the guest), the hologram unit tests (pairing and direction) and the block-grid mesh tests, which now check that chests leave a hole in the terrain mesh. Not tried in the game: the real chest models, the double-chest pairing, the roots fix and the shorter remesh. Nothing was measured on screen, so the stutter and slow-ghost improvements are unconfirmed.

# Cinnamatica 0.3.44

- Fixed: 0.3.43 stopped the mod during the check for missing blocks after a paste. The check sent up to eight commands in one frame, and the game ends the mod for good when a frame asks for more than four. It now sends at most three per frame, which also keeps it clear of other senders. If the mod already shows as failed, press Reload on it in the Mods list or restart the game; logging out and back in does not clear it.
- The check also no longer gives up on the whole build when part of it cannot be read (for example a part more than 128 blocks away). After thirty failed tries a box is counted as not checked and the check goes on; the final message says how many cells were not checked. This is a mod change only.

Verified: the mod's unit tests (74, including a new one that keeps the check under the game's command limit). Not tried in the game: the paste check itself has still not been seen to finish; this fixes the crash that stopped it.

# Cinnamatica 0.3.43

- After a paste, the mod now checks for blocks the server did not keep and places only those again. Blocks that depend on another block (hanging signs and lanterns, wall signs, sand, torches, rails, plants) break when the paste places them before their support exists, so they were missing after a paste. When the paste has finished (command paste or Structure Paste) the mod waits a few seconds for the server, reads the pasted area a few cells at a time, lists the missing blocks, sends them again from the bottom up, waits, and checks again, up to three re-sends. The label shows `Paste check` with the round and progress, and the last message says whether every block is in place or how many are still missing. Water, lava and powder snow are not counted, cells in unloaded chunks are reported as not checked, and pressing Paste again cancels the check. With Paste Destroy on, wrong blocks are replaced as well. This is a mod change only.
- The hologram no longer drops out and comes back every second or so. The log of the last run showed the mod running out of its per-frame compute budget about every 1.3 seconds; each time the game restarted the mod and the hologram was rebuilt from nothing, which looked like constant flicker. The ghost updates now measure what a cell edit costs and send fewer when the frame is busy, they keep a larger reserve, and the background checks around the camera and across the build start only when more of the frame is left. The cost is that a very large paste may take a little longer to show its ghosts. This is a mod change only.
- Still not fixed: Structure Paste can still skip blocks when the server is busy or the chunks are not loaded (the paste check above now sends those again), and chests are still a stand-in cube. The 0.3.42 depth step ran in the game: you reported that ghosts no longer glitch through blocks; its speed gain is not measured.

Verified: the mod's unit tests (73, including new ones for the batch sizing and the paste check's box order, run lengths and send order). Not tried in the game: whether the budget traps are gone, and how the paste check behaves against a real server (how long it waits, how fast it reads, whether a re-sent block stays). The check's own speed on a large build is a guess. Check the next log for `fuel exhausted`; there should be none.

# Cinnamatica 0.3.42

- Hologram flicker on existing builds: the hologram (and any other tinted block grid) is now drawn a small step behind the world in the depth buffer. Where a ghost face lay in the same plane as a real block face, the two used to tie and flicker; now the real block always wins. This is a client change, and the strength of the step is a first guess that depends on the graphics driver (DirectX 12 and Vulkan scale it differently).
- Hologram speed: sections of the hologram that are outside the camera view are no longer drawn. Before, every section of a large build was drawn every frame, which is a heavy cost on an integrated GPU. This is a client change.
- Not fixed yet: Structure Paste still skips blocks when the server is busy or the chunks are not loaded, the hologram still sometimes disappears for about a second, and chests are still a stand-in cube.

Verified: the view check (seven cases) passed in a separate small test program. The render change itself was not compiled before this release (the build container has Rust 1.97 and the client needs 1.99), so the full build is its first compile. The mod itself is unchanged since 0.3.41. Not tried in the game: whether the depth step stops the flicker on your graphics card, and how much faster large builds draw.

# Cinnamatica 0.3.41

- Pasting: the ghost of a cell is hidden the moment the paste sends it to the server (both the command paste and Structure Paste), instead of waiting for the verifier to see the block. While the world still shows air there, a stale reading does not bring the ghost back; it holds for about four seconds, so a paste the server refuses gets its ghost back after that. Large batches of ghost changes also go to the hologram in bigger groups (up to 1536 cells at a time instead of 192) when the frame has fuel.
- Blocks that were already built: the hologram checks the whole 16-block box around the camera once, a few hundred cells a frame, as soon as you are near, instead of leaving their ghosts to the slow lap of the tile checker. Ghost changes a tile owes no longer wait for that tile's next turn, and are sent to the hologram even on frames short of fuel.
- Chests and other blocks that the client draws with a block-entity model (shulker boxes, ender chests, copper chests, banners, beds, pots, heads) are still a stand-in cube in the hologram, but it is now a block that looks like them: a chest is spruce-brown, an ender chest obsidian, a shulker box its own colour, a copper chest its copper. It falls back to the old white cube for anything not listed. This is a client change.
- Water, lava and powder snow are no longer counted as missing, wrong or extra and get no red or yellow box, and a water, lava or powder snow cell that is not right does not hold back Auto next layer. Water, lava or powder snow where the schematic has air is not an extra block. They are still counted in the material list. Their ghost is still drawn.

Verified: the mod's unit tests (68 for the public guest, new ones for the paste hold, the liquid rules and the next-layer check) and the 14 block-grid tests of the render crate (new ones for the stand-in blocks). Not tried in the game: how pasting looks now, how the stand-in cubes look, and whether the bigger batches stay inside the frame fuel budget (watch for `fuel exhausted` in the log).

# Cinnamatica 0.3.40

- The hologram now updates single blocks right away around you. A guide watches a 16-block box centred on the camera every frame. When a block in it changes, only that one ghost cell is edited: a correct block hides its ghost, a broken block (wrong or right) brings the ghost back, a right block in the wrong orientation or a different block shows its yellow or red box at once. The hologram itself is never hidden, rebuilt or toggled, and the tiles still do the full check right after.
- Far sweep: the guide sweeps the part of a large hologram outside the tile window in 8-block regions instead of 4-block ones, and gives it two of every four frames instead of two of every six, so a stale ghost far away is corrected sooner.
- Cells further than that box from you update the same way as before.

Verified: the mod's unit tests (62 for the public guest, new ones for the change detection, the watched box and the sweep timing). Not tried in the game: how fast it feels, and whether the per-frame terrain read and the single-cell edits stay inside the frame fuel budget, are still to be checked with the `fuel exhausted` log line.

# Cinnamatica 0.3.39

- Fast Place now finds the item for a guide block from the game's own data instead of its name: wall and standing signs, banners, heads, coral fans and hanging signs are clicked on the right face, double slabs are built as their full block, and the head half of a bed is skipped (the foot places both).
- Hoppers ignore the planned facing (the server decides it). A lever on a ceiling is placed by clicking the underside of the block above it, and is skipped when there is no such block.
- Holding sneak while placing against an interactive block (a chest, a crafting table, a door) so the click places instead of opening it; sneak is released after the click is sent.
- The guide ignores states the world controls (for example powered or water level) and liquids when it checks a block against the schematic, so those no longer show as wrong.
- Holograms draw blocks that have block entities (chests, signs, banners and the like) as a placeholder box instead of nothing.
- Credit: these Fast Place changes are ThSsilentKnight's (commit e004ab262, merged as 2adc4873f on GitHub main).

Verified: the mod's unit tests (73 for the Personal guest). The client changes were not compiled or tried in the game when this was prepared; the Update Main build is their first real compile.

# Cinnamatica 0.3.36

- Fast Place V2 Strict now places blocks that have no placement rule (copper bulbs, scaffolding, rails, containers and the like) by clicking into air. Before, it refused each one with "could not predict this placement". The server decides their state, as it already did for a click on a real neighbour.
- Droppers, dispensers, observers and pistons: the client predicts their facing with the server's eye-height rule (within one block of you in x and z, a cell more than two blocks below your eye points up and a cell above your eye points down). The old rule used your body box, which differs while you hover or fly; standing on the ground gives the same result under both. Verified only against the local server's source, not against other servers.
- Fast Place no longer offers water, lava and other liquid cells, which have no item, so holding use on a guide with water does not make the client refuse it dozens of times a second.
- A powered-off redstone torch in the guide is placed with the redstone torch item.

Verified: the mod unit tests and the new facing rule in isolation. The client change is compiled only by the full build, and none of this has been tried in the game.

# Cinnamatica 0.3.35

- Importing: a Bedrock `.mcstructure` whose structure section is stored as compressed bytes (format version 2, `compression` 1, with the block layers as int arrays) is now read. Before, such a file failed with "expected an NBT compound". The section may be zlib, gzip, raw deflate or uncompressed, and is limited to 64 MiB once decompressed.
- Everything else in Cinnamatica is unchanged from 0.3.34.

Verified: importer tests, including the real file that failed (kept as a test fixture), and the mod and core tests.

## 0.3.34

- Catch up with Cinnabar: merge the 143 upstream commits from November 10 to 19 (upstream `dev` at d2195d658). They include Creative block breaking at vanilla speed, faster chunk decoding and terrain drawing, NPC dialogue, armor trims and server cosmetics, NetherNet fixes, per-user Windows installs, and local worlds that always run on the Bedrock Dedicated Server.
- Upstream now requires proof that your Microsoft account owns Minecraft before a packaged launch. Cinnamatica keeps that check.
- The tool's Ctrl+scroll and Alt+scroll use Cinnabar's new mouse-wheel API (`read-scroll` / `reserve-scroll`) in place of the 0.3.33 wheel reservation.
- The tool reads the held item through `read-revisions` and `read-equipment`, because upstream removed `read-equipment-matches`. It copies the inventory only when the selected slot or the equipment changes, and reads nothing while the use control is held without the stick, so Hold to Place keeps its full frame budget. This removes one cause of the Hold to Place stutter 0.3.33 added; a remaining stutter with logs, slabs and stairs is still under test.
- Importing: read Java `.nbt` structures (plain or gzipped) and `.gz` wrappers. A file with an unknown or wrong extension is now tried against every supported format by its contents, and the file pickers offer an All files choice.
- Importing: a Bedrock structure is no longer refused with "Expected Bedrock structure format version 1" when its version is stored as a byte, short or long, is missing, or is another number. Any file with the structure layout is read.
- Ghost blocks: a block state the client's registry does not know (redstone wire connection states, for example) now draws that block's default state. Before, one unknown state turned every ghost in the guide into flat blue boxes.
- Fast Place: a dispenser or dropper planned as powered no longer blocks automatic rotation; droppers, furnaces, chests-like blocks and other blocks the client cannot predict now turn to the planned horizontal facing (observers and pistons face away from you, droppers and dispensers toward you) before they are placed. Facings that point up or down depend on where you stand and are not turned for.
- Blocks that point out of the face they are placed against (end rods, lightning rods, amethyst buds, ladders) now get the click sent at the neighbour's matching face, as stairs and logs already do, without turning your camera.
- A powered comparator or repeater, and lit furnaces, smokers, blast furnaces, redstone lamps and redstone ore, are placed as their unpowered or unlit block, since that is the item you hold; the world powers them afterwards.
- Removed the trapdoor open/close reconciliation from V2 Strict: it could place a second trapdoor above the first. Trapdoors place closed; open them by hand.
- Fast Place can now place blocks the client has no placement rule for (containers, rails, pots, hoppers, droppers, dispensers, target blocks and similar) when a neighbouring block is clicked at the guide cell. The client sends the click and the server decides the block's state, as it already did for beds.
- Hold to Place: refused-placement messages move from the info to the debug log, the hotbar slot that Fast Place reuses is the one used least recently instead of always the selected one, and the repeat clock is credited for the orientation tick so oriented blocks repeat sooner. These are untested in game.
- The schematic outline in the model shader moves to the varying location that upstream's packed texture slot frees.
- The Windows setup EXE now updates an existing Cinnamatica install in place (same folder, Start menu entry and uninstaller) and keeps its profile, instead of installing a second copy. It installs fresh only when none is found. Portable users extract the ZIP over their old folder and keep `profile`.
- Everything else in Cinnamatica is unchanged from 0.3.33.

Verified: mod, core and importer tests; gameplay, inventory, input, keyboard settings, UI, protocol and mod API tests; the Windows build; and the merge checks in TEST-CHECKLIST.md, which passed in game in the 0.3.34 test copy on November 19.

## 0.3.33

- Add Litematica's **tool item** (a stick), with its nine modes, controls and default keys taken from Litematica's source (26.2 branch). The tool is **on by default**, as in Litematica. **M+T** turns it off and on.
- Modes, in Litematica's order: Area Selection, Schematic Placement, Fill, Replace Block, Paste Schematic, Grid Paste, Move, Delete and Rebuild. **Ctrl+scroll** cycles them, and the HUD shows the mode, the selection and what each control does.
- With the stick held, **left/right click** set selection corner 1/2 in the area modes (Area Selection, Fill, Replace Block, Move, Delete) and move the guide to the aimed cell in the schematic modes. **Alt+scroll** nudges the selection or guide along the direction you look. The clicks and modified scrolling are withheld from the game, so they never break or place blocks or change the hotbar slot.
- **Alt+middle click** picks the Fill/Replace block from the world, and **Shift+middle click** picks the block to replace.
- **Execute Operation** runs Paste, Grid Paste, Fill, Replace Block and Delete. As in Litematica, it is **unbound by default**: bind **Cinnamatica Execute Operation** in Settings > Keyboard & Mouse. Fill, Replace and Delete send `/fill` in 32-block pieces.
- **Ctrl+M** switches Paste between keeping and replacing existing blocks, and cycles the Grid Paste size (2x1 to 8x8). **Ctrl+Alt+S** copies the selection into the guide (at most 64 blocks a side, within 128 blocks).
- **Move:** Alt+scroll moves the selected blocks one block at a time. It copies the area, clears the old and new boxes, then rebuilds the copy. A move stops if part of the area is not loaded.
- **Rebuild:** with the stick put away, left-click removes the aimed guide block (both halves of a door or tall plant) and right-click sets it to the held block. These edits change the guide and Paste for this session only.
- The selection is drawn as a white box, with corner 1 in red and corner 2 in blue.
- Litematica's other default keys: the **M** menu now opens when M is released on its own, so M chords work. **M+R** toggles all rendering, **M+G** shows or hides the guide, **M+L**/**M+V** open Materials, **M+P** Controls, **M+C** Settings and **M+S** the library. **M+Page Up/Down** change the layer mode. **Page Up/Page Down** change the layer, which is the new default for those rows; they were ] and [. Hold **I** to read the aimed planned block.
- New Keyboard settings rows, unbound like Litematica's: Cinnamatica Toggle Tool Item, Execute Operation, Next Tool Mode and Previous Tool Mode. The M chords are built in, because Bedrock's settings store one key with modifiers.
- The client lets a mod reserve the mouse wheel (`MouseWheel`) during gameplay. The mod requests the read-only `player_state` grant to see the selected hotbar slot; the launcher adds it to existing installs and keeps a value you already set.
- While the stick is not held, Fast Place, Paste, Placement Lock, Hold to Place and the ghost overlay work as in 0.3.32. Fast Place pauses while the stick is in your hand.
- Not included: several selection boxes and the selection manager, saving a selection as a file, the schematic verifier screen, sub-regions, schematic projects, pick block from the guide, overlay-through-blocks, and Litematica's Rebuild direction/all modifiers.

Verified: the guest, core, importer, input and keyboard-settings unit tests pass, and the client type-checks. Not verified: the WASM and client builds of this version, and every in-game behaviour above. See TEST-CHECKLIST.md.

## 0.3.32

- Include the 0.3.31 plant, trapdoor and startup-message changes.
- Offer an occupied wrong-material guide cell at the real crosshair obstruction so the native predictor can replace existing vegetation. Solid blocks remain rejected by the native validator.
- This guest update uses the unchanged native 0.3.31 runtime and the same mod API.

## 0.3.31

- Predict common flowers, grasses, ferns and saplings on the supported normal soils. Keep specialized crops/aquatic plants unresolved rather than inventing their survival rules.
- Place supported tall plants as one item producing lower and upper cells; aiming at the upper ghost resolves to the lower placement. Reject blocked headroom and water/lava.
- Allow supported plants to replace loaded replaceable vegetation and guard against placing an already correct plant again while the guide verifier catches up.
- Fix the held-use suppression that could prevent an explicitly requested trapdoor open/close interaction immediately after placement. Recheck the target and interaction before sending so reconciliation cannot place the held block by mistake.
- Distinguish a remembered Vulkan failure from a failure in the current launch and explain the existing Video setting retry route. Keep the working DX12 fallback.
- Preserve the 0.3.30 Hold to Place, log orientation, lantern and large-guide budget changes.

Stock Bedrock 1.26.60.6 Survival probes accepted normal-soil flowers and paired tall plants and rejected flowers on stone. Air clicks on trapdoors resolved to the same closed bottom-half state regardless of the tested face; arbitrary unsupported upper placement remains unresolved. The earlier exit without a completed crash report still has no confirmed cause. All live user checks remain open; see TEST-CHECKLIST.md.

## 0.3.30

- Opt-in Hold to Place uses a 50 ms assisted attempt interval and starts ready hotbar requests in the same frame. Inventory and reported-facing confirmation remain required.
- Keep assisted holds from falling back to the wrong held block when a ghost changes or temporarily disappears; ordinary placement remains available outside the guide after releasing the gesture.
- Scope Placement Lock to non-air schematic destinations.
- Choose log support faces by the planned pillar axis, allow corrections while moving, and require predicted orientation to match whenever Auto Orientation is enabled.
- Reconcile a wooden trapdoor's open/closed state by normal interaction after its placement state matches.
- Predict lantern attachment from the center of loaded vertical support geometry, including flush partial supports; unresolved or absent support is not invented.
- Keep ghost verification running during Paste with a reserved frame budget.
- Include ThsSilentKnight's Linux AppImage packaging and portable Linux runtime layout (b7abe3237 and 134eec312). No AppImage publication or Windows placement fixes were included in those commits.

Unsupported upper slabs/stairs and upper trapdoors still require a real suitable support on stock Bedrock. Isolated server probes rejected the requested upper state or resolved an air click to the lower state. This update does not claim to bypass that server behavior. Live in-world responsiveness and the earlier crash without a completed report still need acceptance evidence.

## 0.3.29 (previously staged)

- Scan distant blocks in smaller batches on dedicated frames, with fuel reserved for edits and other guide work. This avoids the guest restarts that made large guides disappear.
- Prepare material names and palette counts in batches so opening Materials on a large guide stays within its frame budget.
- Import legacy Java grass and chain names, including the Wizard Tower schematic.

## 0.3.28

- Verify distant loaded blocks in the full textured preview so already placed blocks lose their ghost outlines, even outside the nearby scan window.
- Predict vertical iron-chain air placement using the normal Bedrock item-use path.

## 0.3.27

- Fix a render-pipeline validation crash when a saved textured ghost appears. Both shader stages now enable the outline coordinates.

## 0.3.26

- Draw thinner outlines along the rendered faces of partial ghost blocks, including stairs, fences, walls, slabs, and anvils. Blended models and sloped liquid faces use their geometry too.

## 0.3.25

- Publish each complete ghost mesh with its glass and other translucent faces ready to draw. Keep using the existing sort when a camera update cannot allocate a new one.
- Retain the last known ghost visibility when a terrain sample is temporarily unavailable.
- Keep fallback ghost tiles and status markers visible while textured edits wait for the renderer.

## 0.3.24

- Sync the Windows and Linux holograms with textured, solid light-blue ghosts, thinner outlines, and a Ghost lightness setting.
- Reuse the schematic palette and unaffected section meshes when blocks change. Keep drawing the old guide until the complete replacement is ready.
- Raise the per-schematic import limit to 400,000 bounding-box cells. The library still has a shared size limit.

## 0.3.23

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
