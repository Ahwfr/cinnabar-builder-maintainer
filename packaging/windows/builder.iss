#ifndef BuilderVersion
  #error BuilderVersion is required
#endif
#ifndef PackagePath
  #error PackagePath is required
#endif

[Setup]
AppId={{E093173F-0F6A-4ED0-BB72-3FD6B843EA96}
AppName=Cinnabar Builder
AppVersion={#BuilderVersion}
AppPublisher=Cinnabar Builder contributors
AppPublisherURL=https://github.com/Ahwfr/cinnabar-builder
DefaultDirName={localappdata}\Programs\Cinnabar Builder
DefaultGroupName=Cinnabar Builder
UninstallDisplayIcon={app}\Cinnabar Builder.exe
ArchitecturesAllowed=x64compatible
MinVersion=10.0
PrivilegesRequired=lowest
DisableProgramGroupPage=yes
Compression=lzma2/fast
SolidCompression=yes
OutputBaseFilename=Cinnabar-Builder-v{#BuilderVersion}-windows-x64-setup
LicenseFile={#PackagePath}\App\LICENSE
CloseApplications=yes
RestartApplications=no

[Files]
Source: "{#PackagePath}\Cinnabar Builder.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackagePath}\App\*"; DestDir: "{app}\App"; Excludes: "Builder-Source.zip"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Cinnabar Builder"; Filename: "{app}\Cinnabar Builder.exe"; WorkingDir: "{app}"

[Run]
Filename: "{app}\Cinnabar Builder.exe"; Description: "Launch Cinnabar Builder"; Flags: nowait postinstall skipifsilent
