# Builder platform status and Linux port

The pinned game target is Bedrock 1.26.60, protocol 2225, with dedicated server
1.26.60.6. See `assets/bedrock-target.json`. Windows has a locally built client,
portable launcher and importer. Menu and material rendering, library import and
placement all need separate acceptance checks. The latest Air Place and orientation
changes passed focused checks and await in-world acceptance. macOS and mobile
Builder builds have no verified end-to-end result in this checkout.

## Linux

Install the pinned Rust and Go versions, Make, and the development packages from
upstream Ubuntu CI: libwayland-dev, libxkbcommon-dev, libasound2-dev and
libudev-dev. The file picker needs `zenity` or `kdialog`. Then run:

```sh
builder/linux/build-builder.sh
builder/linux/start-builder.sh
```

`start-builder.sh --check` reports the build state and file picker,
`--import-only [--demo] [FILE...]` installs a library without starting the game, and
`--profile` selects another Cargo profile. The launcher installs or upgrades the
library in `.local/builder-user` and enables it before starting the client.

Verified on Ubuntu 24.04 (X11 at 1.25 scale, Vulkan on Intel Iris Xe, Mesa 25.2)
against the pinned local server, driven through the developer-control client: the
client, Go core, importer and guest build without engine source changes; importer
tests pass; the launcher installs and enables the library; M, Alt+J and Alt+K open
the Schematica screens; the in-game **Import schematic…** button runs the zenity
picker with two `.mcstructure` files and reloads the new library; and Load and
Place here anchor a guide whose ghost blocks render. The picker was a scripted
zenity stand-in, so the real dialog's cancel path and kdialog are unverified.
Still unverified: sign-in, sound, Fast Place and Bridge Assist against a server,
and Wayland sessions.

1. Check the GPU backend, window input and sound in a fresh profile on more
   distributions and on Wayland.
2. Exercise the real zenity and kdialog pickers, including cancel and
   multi-file selection.
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
