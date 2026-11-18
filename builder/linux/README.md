# Cinnamatica for Linux

ThsSilentKnight contributed the Linux build and launch scripts, the schematic picker and the AppImage.

## Download

**Incomplete:** no AppImage has been published yet. The **Cinnamatica Linux** workflow is disabled because GitHub never started its runs in this repository, so the packed image is untested. The staged AppDir passed a local launch test. Until a release carries the AppImage, build from source below.

Once published, download `Cinnamatica-<version>-x86_64.AppImage` from [Cinnamatica Releases](https://github.com/Ahwfr/cinnamatica/releases), then make it executable and run it:

    chmod +x Cinnamatica-*-x86_64.AppImage
    ./Cinnamatica-*-x86_64.AppImage

The first launch prepares Minecraft assets. Your worlds, login, options and schematic library live in `~/.local/share/Cinnamatica` (or `$XDG_DATA_HOME/Cinnamatica`). Set `CINNABAR_USER_ROOT` to keep them somewhere else. To update, replace the AppImage; the launcher upgrades the library and keeps your profile.

The AppImage needs a 64-bit distribution with glibc 2.35 or newer, such as Ubuntu 22.04, Debian 12 or Fedora 36. Install Zenity or KDialog for the schematic picker. If FUSE is unavailable, run it with `--appimage-extract-and-run`.

## Build from source

    git clone https://github.com/Ahwfr/cinnamatica.git
    cd cinnamatica
    builder/linux/build-builder.sh
    builder/linux/start-builder.sh

Install the pinned Rust and Go toolchains, GNU Make, and the development packages listed in the [Linux port notes](../../docs/PORTING.md).

To build the AppImage yourself, run `make package-binaries` and `builder/linux/build-builder.sh --importer-only`, then `builder/linux/build-appimage.sh` with `appimagetool` on the `PATH`. The **Cinnamatica Linux** workflow builds and publishes the same AppImage on GitHub.
