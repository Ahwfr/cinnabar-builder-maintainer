# Builder for Windows

Download the setup EXE or portable ZIP from [Builder Releases](https://github.com/Ahwfr/cinnabar-builder/releases). Extract the entire ZIP before opening **Cinnabar Builder.exe**. Keep the profile when replacing a portable build.

For a source build, install Rust through rustup, Go, GNU Make, Git for Windows, and the MSVC build tools. From the repository root run:

    .\builder\windows\Build-Builder.ps1
    .\builder\windows\Start-Builder.ps1

The CMD launchers in this folder run the same scripts. See the [portable guide](../PORTABLE-README.md) and [Windows packaging](../../packaging/windows).
