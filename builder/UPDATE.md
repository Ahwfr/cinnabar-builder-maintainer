# Installing and updating Builder

On Windows, run the setup EXE from a versioned release, then open **Cinnabar Builder** from Start. Setup installs for the current user. The portable ZIP also works when extracted into a writable folder.

For the next release, close Builder and extract the new package into the same folder. Keep **profile/**. It contains your worlds, account data, settings, mods and library. The launcher checks that the template matches the package revision, upgrades the library code and backs up the previous component. Confirm the release version and guest hash in the Schematica header or Mods page.

Choose **Import schematics** in the Schematica menu to add local files to the current library. The library supports up to eight entries. The importer reads local files with bounded size and decoding limits. Original input files remain in place.

In Schematica Settings, cycle **Fast Place** between **Off**, **V1 Classic**, and **V2 Strict**. V2 can fill an empty cell within reach using normal Bedrock placement packets, including when there is no usable support face. It only sends a placement it predicts will match the plan.

To paste a guide into a cheats-enabled world, be a Creative operator and allow Schematica's `setblock` and `fill` permissions in Mods. Open Schematica, choose **Paste build**, then **Confirm paste**. The server validates the resulting commands. Paste preserves occupied cells and reports any unsupported state encodings.

Only the Windows package, exact source ZIP and documentation belong in a release. Profiles and compiler caches stay on your computer. The archive ZIP can be removed from your computer after extraction if you keep another backup.

# Maintaining the fork

The shared version is in **builder/schematica/Cargo.toml**, under **workspace.package**. Increment the patch for a fix and the minor version for a feature update. Update RELEASE-NOTES.md for the same version. The guest and importer read this version automatically. Release tags use **builder-vVERSION**.

Use the existing editable checkout and one persistent Cargo build cache for subsequent builds. The **play** profile enables incremental compilation and omits debug information. UI-only changes need a guest/importer rebuild, and can use an unchanged native runtime only when its API matches.

The Windows workflow packages successful builds and publishes a versioned release. GitHub can pause hosted builds on a fork. A local build remains available while a maintainer resolves that restriction.

To publish an already staged and verified package, run **builder/Publish-BuilderRelease.ps1 -PackageDirectory PATH -Publish**. It excludes the private profile, editable source folder and compiler caches from the ZIP. It refuses to overwrite an existing version tag.

To produce the setup EXE from the staged package, run **builder/Build-BuilderInstaller.ps1 -PackageDirectory PATH -Compiler PATH_TO_ISCC** with Inno Setup. Game data in the install directory's **profile/** is retained on update and uninstall.
