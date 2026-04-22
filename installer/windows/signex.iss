; Signex — Windows installer (InnoSetup)
;
; Invoked from CI via:
;   ISCC.exe installer\windows\signex.iss /DVersion=0.6.3 /DArch=x64
;
; /DVersion  — version string without the leading "v" (tag minus "v")
; /DArch     — "x64" or "arm64" — picks the right binary source path
; /DBinary   — absolute path to the compiled signex.exe; overrides the
;              target-default when testing locally.

#ifndef Version
  #define Version "0.0.0"
#endif

#ifndef Arch
  #define Arch "x64"
#endif

#if Arch == "x64"
  #define TargetTriple "x86_64-pc-windows-msvc"
  #define ArchSuffix "x86_64"
  #define ArchAllowed "x64compatible"
  #define ArchInstallIn64Bit "x64compatible"
#elif Arch == "arm64"
  #define TargetTriple "aarch64-pc-windows-msvc"
  #define ArchSuffix "aarch64"
  #define ArchAllowed "arm64"
  #define ArchInstallIn64Bit "arm64"
#endif

#ifndef Binary
  #define Binary "..\..\target\" + TargetTriple + "\release\signex.exe"
#endif

[Setup]
AppId={{C4E84F2F-1D41-4FA9-9D8F-67D37CA0F7FC}
AppName=Signex
AppVersion={#Version}
AppVerName=Signex {#Version}
AppPublisher=alpCaner
AppPublisherURL=https://github.com/alplabai/signex
AppSupportURL=https://github.com/alplabai/signex/issues
AppUpdatesURL=https://github.com/alplabai/signex/releases
DefaultDirName={autopf}\Signex
DefaultGroupName=Signex
AllowNoIcons=yes
LicenseFile=
OutputDir=.
OutputBaseFilename=signex-setup-{#ArchSuffix}-{#Version}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
; The .ico is produced by installer/build-icons.sh and lives next to this script.
; Silently skipped if the file is absent (e.g. a first clone before running the script).
#if FileExists("signex.ico")
  SetupIconFile=signex.ico
#endif
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed={#ArchAllowed}
ArchitecturesInstallIn64BitMode={#ArchInstallIn64Bit}
UninstallDisplayIcon={app}\signex.exe
UninstallDisplayName=Signex {#Version}
DisableProgramGroupPage=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Binary}"; DestDir: "{app}"; Flags: ignoreversion
#if FileExists("signex.ico")
Source: "signex.ico"; DestDir: "{app}"; Flags: ignoreversion
#endif

[Icons]
#if FileExists("signex.ico")
  #define IconOpt "; IconFilename: ""{app}\signex.ico"""
#else
  #define IconOpt ""
#endif
Name: "{group}\Signex"; Filename: "{app}\signex.exe"{#IconOpt}
Name: "{group}\{cm:UninstallProgram,Signex}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Signex"; Filename: "{app}\signex.exe"; Tasks: desktopicon{#IconOpt}

[Run]
Filename: "{app}\signex.exe"; Description: "{cm:LaunchProgram,Signex}"; Flags: nowait postinstall skipifsilent
