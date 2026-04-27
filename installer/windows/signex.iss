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
; Per-file-type .ico files for Signex's native .snx*** extensions.
; Produced by installer/build-file-icons.sh into installer/windows/files/.
; Each `#if FileExists` branch is guarded so a fresh clone still builds
; even before `build-file-icons.sh` has been run.
#if FileExists("files\snxprj.ico")
Source: "files\snxprj.ico"; DestDir: "{app}\files"; Flags: ignoreversion
#endif
#if FileExists("files\snxsch.ico")
Source: "files\snxsch.ico"; DestDir: "{app}\files"; Flags: ignoreversion
#endif
#if FileExists("files\snxpcb.ico")
Source: "files\snxpcb.ico"; DestDir: "{app}\files"; Flags: ignoreversion
#endif
#if FileExists("files\snxfpt.ico")
Source: "files\snxfpt.ico"; DestDir: "{app}\files"; Flags: ignoreversion
#endif
#if FileExists("files\snxsim.ico")
Source: "files\snxsim.ico"; DestDir: "{app}\files"; Flags: ignoreversion
#endif
#if FileExists("files\snxlib.ico")
Source: "files\snxlib.ico"; DestDir: "{app}\files"; Flags: ignoreversion
#endif
#if FileExists("files\snxsym.ico")
Source: "files\snxsym.ico"; DestDir: "{app}\files"; Flags: ignoreversion
#endif
#if FileExists("files\snxpkg.ico")
Source: "files\snxpkg.ico"; DestDir: "{app}\files"; Flags: ignoreversion
#endif
#if FileExists("files\snxmat.ico")
Source: "files\snxmat.ico"; DestDir: "{app}\files"; Flags: ignoreversion
#endif
#if FileExists("files\snxcfg.ico")
Source: "files\snxcfg.ico"; DestDir: "{app}\files"; Flags: ignoreversion
#endif
#if FileExists("files\snxmod.ico")
Source: "files\snxmod.ico"; DestDir: "{app}\files"; Flags: ignoreversion
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

[Registry]
; File associations for Signex's native .snx*** extensions.
; Each extension: user-scope HKCU root (matches PrivilegesRequired=lowest),
; a ProgId keyed under `Signex.<ext>`, a DefaultIcon pointing at the
; per-type .ico shipped in {app}\files\, and an `open` verb that passes
; the clicked file path to signex.exe via "%1".
; Covers: snxprj, snxsch, snxpcb, snxfpt, snxsim, snxlib, snxsym.
Root: HKCU; Subkey: "Software\Classes\.snxprj"; ValueType: string; ValueData: "Signex.snxprj"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Signex.snxprj"; ValueType: string; ValueData: "Signex Project"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Signex.snxprj\DefaultIcon"; ValueType: string; ValueData: "{app}\files\snxprj.ico"
Root: HKCU; Subkey: "Software\Classes\Signex.snxprj\shell\open\command"; ValueType: string; ValueData: """{app}\signex.exe"" ""%1"""

Root: HKCU; Subkey: "Software\Classes\.snxsch"; ValueType: string; ValueData: "Signex.snxsch"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Signex.snxsch"; ValueType: string; ValueData: "Signex Schematic"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Signex.snxsch\DefaultIcon"; ValueType: string; ValueData: "{app}\files\snxsch.ico"
Root: HKCU; Subkey: "Software\Classes\Signex.snxsch\shell\open\command"; ValueType: string; ValueData: """{app}\signex.exe"" ""%1"""

Root: HKCU; Subkey: "Software\Classes\.snxpcb"; ValueType: string; ValueData: "Signex.snxpcb"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Signex.snxpcb"; ValueType: string; ValueData: "Signex PCB"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Signex.snxpcb\DefaultIcon"; ValueType: string; ValueData: "{app}\files\snxpcb.ico"
Root: HKCU; Subkey: "Software\Classes\Signex.snxpcb\shell\open\command"; ValueType: string; ValueData: """{app}\signex.exe"" ""%1"""

Root: HKCU; Subkey: "Software\Classes\.snxfpt"; ValueType: string; ValueData: "Signex.snxfpt"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Signex.snxfpt"; ValueType: string; ValueData: "Signex Footprint"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Signex.snxfpt\DefaultIcon"; ValueType: string; ValueData: "{app}\files\snxfpt.ico"
Root: HKCU; Subkey: "Software\Classes\Signex.snxfpt\shell\open\command"; ValueType: string; ValueData: """{app}\signex.exe"" ""%1"""

Root: HKCU; Subkey: "Software\Classes\.snxsim"; ValueType: string; ValueData: "Signex.snxsim"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Signex.snxsim"; ValueType: string; ValueData: "Signex Simulation"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Signex.snxsim\DefaultIcon"; ValueType: string; ValueData: "{app}\files\snxsim.ico"
Root: HKCU; Subkey: "Software\Classes\Signex.snxsim\shell\open\command"; ValueType: string; ValueData: """{app}\signex.exe"" ""%1"""

Root: HKCU; Subkey: "Software\Classes\.snxlib"; ValueType: string; ValueData: "Signex.snxlib"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Signex.snxlib"; ValueType: string; ValueData: "Signex Library"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Signex.snxlib\DefaultIcon"; ValueType: string; ValueData: "{app}\files\snxlib.ico"
Root: HKCU; Subkey: "Software\Classes\Signex.snxlib\shell\open\command"; ValueType: string; ValueData: """{app}\signex.exe"" ""%1"""

Root: HKCU; Subkey: "Software\Classes\.snxsym"; ValueType: string; ValueData: "Signex.snxsym"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Signex.snxsym"; ValueType: string; ValueData: "Signex Symbol"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Signex.snxsym\DefaultIcon"; ValueType: string; ValueData: "{app}\files\snxsym.ico"
Root: HKCU; Subkey: "Software\Classes\Signex.snxsym\shell\open\command"; ValueType: string; ValueData: """{app}\signex.exe"" ""%1"""

Root: HKCU; Subkey: "Software\Classes\.snxpkg"; ValueType: string; ValueData: "Signex.snxpkg"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Signex.snxpkg"; ValueType: string; ValueData: "Signex Package"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Signex.snxpkg\DefaultIcon"; ValueType: string; ValueData: "{app}\files\snxpkg.ico"
Root: HKCU; Subkey: "Software\Classes\Signex.snxpkg\shell\open\command"; ValueType: string; ValueData: """{app}\signex.exe"" ""%1"""

Root: HKCU; Subkey: "Software\Classes\.snxmat"; ValueType: string; ValueData: "Signex.snxmat"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Signex.snxmat"; ValueType: string; ValueData: "Signex PCB Material"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Signex.snxmat\DefaultIcon"; ValueType: string; ValueData: "{app}\files\snxmat.ico"
Root: HKCU; Subkey: "Software\Classes\Signex.snxmat\shell\open\command"; ValueType: string; ValueData: """{app}\signex.exe"" ""%1"""

Root: HKCU; Subkey: "Software\Classes\.snxcfg"; ValueType: string; ValueData: "Signex.snxcfg"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Signex.snxcfg"; ValueType: string; ValueData: "Signex Config"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Signex.snxcfg\DefaultIcon"; ValueType: string; ValueData: "{app}\files\snxcfg.ico"
Root: HKCU; Subkey: "Software\Classes\Signex.snxcfg\shell\open\command"; ValueType: string; ValueData: """{app}\signex.exe"" ""%1"""

Root: HKCU; Subkey: "Software\Classes\.snxmod"; ValueType: string; ValueData: "Signex.snxmod"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Signex.snxmod"; ValueType: string; ValueData: "Signex SPICE Model"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Signex.snxmod\DefaultIcon"; ValueType: string; ValueData: "{app}\files\snxmod.ico"
Root: HKCU; Subkey: "Software\Classes\Signex.snxmod\shell\open\command"; ValueType: string; ValueData: """{app}\signex.exe"" ""%1"""

[Run]
Filename: "{app}\signex.exe"; Description: "{cm:LaunchProgram,Signex}"; Flags: nowait postinstall skipifsilent
