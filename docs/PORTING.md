# Builder platform status and Linux port

The pinned game target is Bedrock 1.26.60, protocol 2225, with dedicated server
1.26.60.6. See `assets/bedrock-target.json`. Windows has a locally built client,
portable launcher and importer. Menu and material rendering, library import and
placement all need separate acceptance checks. The latest Air Place and orientation
changes passed focused checks and await in-world acceptance. Linux, macOS and mobile
Builder builds have no verified end-to-end result in this checkout.

1. Build upstream Cinnabar on Linux before changing Builder. Use the pinned Rust
   and Go versions, Make, and the development packages identified by upstream
   Ubuntu CI: libwayland-dev, libxkbcommon-dev, libasound2-dev and libudev-dev.
   The wider shader test job also installs spirv-tools. Check the selected GPU
   backend, window input and sound in a fresh profile.
2. Build and test the standalone Rust importer and WASM guest. Explicit file
   arguments in `builder/schematica/builder/src/main.rs` are the first port target.
   The Windows file dialog, no-argument launcher and profile registration are
   gated in `picker.rs` and `launch.rs`. Verify the CLI on Linux before selecting
   a file-picker backend. Reuse the component packing path.
3. Reuse `packaging/linux` and the upstream installed resource layout. Port the
   PowerShell Builder release wrapper only after the native executable, asset
   preparation and Go sidecars work together. Resolve paths using the native
   installation layout and preserve the existing profile during updates.
4. Verify a fresh launch, file import, WASM permissions, keyboard shortcuts,
   rendering and placement against the pinned local server. Record the display
   server, GPU backend, build revision and exact results. Add Linux CI for
   credential-free checks once these commands are exercised.

## Survival placement follow-up

Normal Fast Place selects a real inventory block and sends a placement against a
clicked block face. Bridge Assist also tries the ordinary downward support pick
in both Survival and Creative. Single-block placement uses the inventory path.
Bulk Creative paste with operator permission is separate future work.

A Survival bridge assist can extend from a real block by searching the ghost's
adjacent faces for a reachable support and using the normal inventory and
placement transaction. After the server confirms a placed block, prefer that
block's face for the next connected schematic cell. The player should be able
to keep looking along the guide and right-click once per block. Clear the last
block when the world, placement or selection changes. If several continuations
are possible, resolve the next cell from the direction the player is looking.
The current orientation solver already recasts and checks
candidate support faces in `app/src/block_use/orientation.rs`. A follow-up should
reuse that validation when the crosshair points at a ghost. If no support exists,
guide the player to add scaffolding first. Test one block per physical press,
actual inventory consumption, unreachable targets and isolated ghosts before
adding any repeated placement.
