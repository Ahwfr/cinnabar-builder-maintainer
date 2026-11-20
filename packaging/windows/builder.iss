#ifndef BuilderVersion
  #error BuilderVersion is required
#endif
#ifndef PackagePath
  #error PackagePath is required
#endif

; One AppId for every release: running this setup over an existing Cinnamatica install updates that
; install in place (same folder, same Start menu entry, same uninstaller) instead of adding a second
; copy. The profile folder (worlds, account, settings, schematic library) is not part of [Files], so
; an update never touches it.
[Setup]
AppId={{E093173F-0F6A-4ED0-BB72-3FD6B843EA96}
AppName=Cinnamatica
AppVersion={#BuilderVersion}
AppVerName=Cinnamatica {#BuilderVersion}
AppPublisher=Cinnamatica contributors
AppPublisherURL=https://github.com/Ahwfr/cinnamatica
DefaultDirName={localappdata}\Programs\Cinnamatica
UsePreviousAppDir=yes
DisableDirPage=auto
DisableWelcomePage=no
DefaultGroupName=Cinnamatica
UsePreviousGroup=yes
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

[InstallDelete]
; The launcher used to be copied under its old name as well; the Start menu entry points at Cinnamatica.exe.
Type: files; Name: "{app}\Cinnabar Builder.exe"

[Icons]
Name: "{autoprograms}\Cinnamatica"; Filename: "{app}\Cinnamatica.exe"; WorkingDir: "{app}"

[Run]
Filename: "{app}\Cinnamatica.exe"; Description: "Launch Cinnamatica"; Flags: nowait postinstall skipifsilent

[Code]
// An existing install is updated, not duplicated. Inno finds it through the AppId; when it does, the
// folder page is skipped (DisableDirPage=auto) and the previous folder is reused. Tell the user.
function PreviousInstall(): Boolean;
begin
  Result := RegKeyExists(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{E093173F-0F6A-4ED0-BB72-3FD6B843EA96}_is1')
         or RegKeyExists(HKLM, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{E093173F-0F6A-4ED0-BB72-3FD6B843EA96}_is1');
end;

procedure InitializeWizard();
begin
  if PreviousInstall() then
    WizardForm.WelcomeLabel2.Caption :=
      'Setup found an existing Cinnamatica and will update it in place to {#BuilderVersion}. ' +
      'Your worlds, account, settings and schematic library are kept.' + #13#10#13#10 +
      'Close Cinnamatica before you continue.';
end;
