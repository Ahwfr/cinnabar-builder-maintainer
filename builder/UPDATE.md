# Installing and updating Builder

0.3.60 changes only the mod. It is untested in the game.

0.3.59 changes only the mod. It is untested in the game.

0.3.57 changes only the mod. It is untested in the game.

0.3.55 changes only the mod. It is untested in the game.

0.3.54 changes only the mod. It is untested in the game.

0.3.53 changes the mod and the game, so it needs a full rebuild. It is untested in the game.

0.3.52 changes the mod and the game, so it needs a full rebuild. It is untested in the game.

0.3.51 changes the mod, so it needs a full rebuild. It is untested in the game.

0.3.50 changes the mod and the game, so it needs a full rebuild. It is untested in the game.

0.3.49 changes the game, so it needs a full rebuild. It is untested in the game.

0.3.48 changes the mod and the game, so it needs a full rebuild. It is untested in the game.

0.3.47 changes the mod, the Structure Host addon and the game, so it needs a full rebuild. It is untested in the game.

0.3.46 changes the game, so it needs a full rebuild. It is untested in the game.

0.3.45 changes the game as well as the mod, so it needs a full rebuild. It is untested in the game.

0.3.44 changes only the mod (a crash fix in the paste check). It is untested in the game.

0.3.43 changes only the mod (budget use of the ghost updates, and the paste check), so no new client features, but the build still runs in full. It is untested in the game.

0.3.42 changes only the client's drawing of the hologram, so it needs the full build. It is untested in the game. It also carries 0.3.41, which was never released.

0.3.41 changes the mod (guide, paste) and the hologram's stand-in blocks in the client, so it needs the full build. It is untested in the game.

0.3.40 changes only the mod (the guide), so the client build is unchanged in behaviour. It is untested in the game.

0.3.39 brings in ThSsilentKnight's Fast Place fixes (item lookup from game data, hoppers, ceiling levers, auto-sneak against interactive blocks, block-entity placeholders). It changes the game client and the mod, so it needs the full build. It is untested in the game.

0.3.36 changes how Fast Place (V2 Strict) places blocks, so it needs the full client build: lamps, rails,
scaffolding and other blocks with no placement rule can now be placed by clicking into air, and droppers,
dispensers, observers and pistons follow the server's eye-height rule instead of a body box. The mod skips
water and lava. It is untested in the game. The setup EXE updates an existing install in place.

0.3.35 fixes importing Bedrock structures saved with a compressed structure section. It changes the
importer only, but every commit changes the client's build stamp, so it needs the same full build as
0.3.34. The setup EXE updates an existing install in place.

0.3.34 merges Cinnabar up to November 19 and works on the Hold to Place stutter from 0.3.33. It changes
the game client, the Go core and the mod, so it needs a full build. First launch asks you to confirm
you own Minecraft.

0.3.33 adds Litematica's tool item with its default keys and nine modes. The tool is on by default
(M+T toggles it), and the M menu now opens when M is released. Execute Operation is unbound, as in
Litematica: bind it in Keyboard settings. It changes the guest, the launcher's mod registration (one
new read-only `player_state` grant) and the native client (new keyboard rows, Page Up/Down layer
defaults and mouse-wheel reservation). Existing saved key changes are kept.

0.3.32 completes the guest target selection for replacing existing vegetation. It
uses the unchanged native runtime from 0.3.31; its mod API is identical.

0.3.31 adds a supported common-plant placement path and paired tall plants, tightens
trapdoor state reconciliation, and clarifies the saved Vulkan failure notice. Test on
valid soil; it does not enable floating plants or specialized crop/aquatic survival
rules. Keep the working graphics API unless deliberately retrying it in Video settings.
Use TEST-CHECKLIST.md to report passes/failures before closing the tracked issues.

0.3.30 includes the previously staged large-guide/material budget fixes. Hold to Place now
requests assisted placement at 50 ms intervals; server inventory and facing confirmation
can still limit throughput. Auto Orientation validates the predicted state, rather than
placing a wrong-facing block when correction fails. Placement Lock applies only inside
the guide. Upper air-placement states remain a confirmed stock-server limitation.

On Windows, run the setup EXE from a versioned release, then open **Cinnamatica** from Start. Setup installs for the current user. The portable ZIP also works when extracted into a writable folder.

For the next release, close Builder and extract the new package into the same folder. Keep **profile/**. It contains your worlds, account data, settings, mods and library. The launcher checks that the template matches the package revision, upgrades the library code and backs up the previous component. Confirm the release version and guest hash in the Cinnamatica header or Mods page.

Choose **Import schematics** in the Cinnamatica menu to add local files to the current library. The library supports up to eight entries. The importer reads local files with bounded size and decoding limits. Original input files remain in place.

In Cinnamatica Settings, cycle **Fast Place** between **Off**, **V1 Classic**, and **V2 Strict**. Enable **Hold to Place** to repeat while the use control is held. V2 can fill an empty cell within reach using normal Bedrock placement packets, including when there is no usable support face. It only sends a placement it predicts will match the plan.

To paste a guide into a cheats-enabled world, be a Creative operator and allow Schematica's `setblock` and `fill` permissions in Mods. Open Schematica, choose **Paste build**, then **Confirm paste**. The server validates the resulting commands. Paste preserves occupied cells and reports any unsupported state encodings.

Only the Windows package, exact source ZIP and documentation belong in a release. Profiles and compiler caches stay on your computer. The archive ZIP can be removed from your computer after extraction if you keep another backup.

# Maintaining the fork

The shared version is in **builder/schematica/Cargo.toml**, under **workspace.package**. Increment the patch for a fix and the minor version for a feature update. Update RELEASE-NOTES.md for the same version. The guest and importer read this version automatically. Release tags use **builder-vVERSION**.

Use the existing editable checkout and one persistent Cargo build cache for subsequent builds. The **play** profile enables incremental compilation and omits debug information. UI-only changes need a guest/importer rebuild, and can use an unchanged native runtime only when its API matches.

The Windows workflow packages successful builds and publishes a versioned release. GitHub can pause hosted builds on a fork. A local build remains available while a maintainer resolves that restriction.

To publish an already staged and verified package, run **builder/Publish-BuilderRelease.ps1 -PackageDirectory PATH -Publish**. It excludes the private profile, editable source folder and compiler caches from the ZIP. It refuses to overwrite an existing version tag.

To produce the setup EXE from the staged package, run **builder/windows/Build-CinnamaticaInstaller.ps1 -PackageDirectory PATH -Compiler PATH_TO_ISCC** with Inno Setup. Game data in the install directory's **profile/** is retained on update and uninstall.

Numpad **8/2** moves the guide forward/back, **4/6** left/right, **7/9** down/up, and **1/3** rotates left/right. Adjust bindings in Keyboard & Mouse settings.
