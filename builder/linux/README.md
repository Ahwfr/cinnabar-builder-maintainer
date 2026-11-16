# Cinnamatica for Linux

ThsSilentKnight contributed the Linux build and launch scripts and the schematic picker. The repository currently has no prebuilt Linux Cinnamatica download.

    git clone https://github.com/Ahwfr/cinnamatica.git
    cd cinnamatica
    builder/linux/build-builder.sh
    builder/linux/start-builder.sh

Install the pinned Rust and Go toolchains, GNU Make, and the development packages listed in the [Linux port notes](../../docs/PORTING.md). Install Zenity or KDialog for the schematic picker. See [Linux packaging](../../packaging/linux) for the AppImage tooling.
