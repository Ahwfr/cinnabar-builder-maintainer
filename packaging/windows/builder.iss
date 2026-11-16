#ifndef BuilderVersion
  #error BuilderVersion is required
#endif
#ifndef PackagePath
  #error PackagePath is required
#endif

[Setup]
AppId={{E093173F-0F6A-4ED0-BB72-3FD6B843EA96}
AppName=Cinnamatica
AppVersion={#BuilderVersion}
AppPublisher=Cinnamatica contributors
AppPublisherURL=https://github.com/Ahwfr/cinnamatica
DefaultDirName={localappdata}\Programs\Cinnamatica
DefaultGroupName=Cinnamatica
UninstallDisplayIcon={app}\Cinnamatica.exe
ArchitecturesAllowed=x64compatible
MinVersion=10.0
PrivilegesRequired=lowest
DisableProgramGroupPage=yes
Compression=lzma2/fast
SolidCompression=yes
OutputBaseFilename=Cinnamatica-v{#BuilderVersion}-windows-x64-setup
LicenseFile={#PackagePath}\App\LICENSE
CloseApplications=yes
RestartApplications=no

[Files]
Source: "{#PackagePath}\Cinnamatica.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackagePath}\App\*"; DestDir: "{app}\App"; Excludes: "Cinnamatica-Source.zip"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Cinnamatica"; Filename: "{app}\Cinnamatica.exe"; WorkingDir: "{app}"

[Run]
Filename: "{app}\Cinnamatica.exe"; Description: "Launch Cinnamatica"; Flags: nowait postinstall skipifsilent
