# Cinnamatica for Windows

Download the setup EXE or portable ZIP from [Cinnamatica Releases](https://github.com/Ahwfr/cinnamatica/releases). Extract the entire ZIP before opening **Cinnamatica.exe**. Keep the profile when replacing a portable build.

For a source build, install Rust through rustup, Go, GNU Make, Git for Windows, and the MSVC build tools. From the repository root run:

    .\builder\windows\Build-Cinnamatica.ps1
    .\builder\windows\Start-Cinnamatica.ps1

The CMD launchers in this folder run the same scripts. See the [portable guide](../PORTABLE-README.md) and [Windows packaging](../../packaging/windows).
