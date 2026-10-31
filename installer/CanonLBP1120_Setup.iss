; ==============================================================================
; Inno Setup 6 Script for Canon LaserShot LBP-1120 Open Source Driver Suite
; Generates standalone executable installer: CanonLBP1120_Setup_v1.0.0-rc.1_x64.exe
; ==============================================================================

#define MyAppName "Canon LaserShot LBP-1120 Driver Suite"
#define MyAppVersion "1.0.0-rc.1"
#define MyAppPublisher "Autonomous Hardware Research Team"
#define MyAppExeName "canon-lbp1120-service.exe"

[Setup]
AppId={{5C7F4349-D8D8-4BC4-B972-78A7E144DC3E}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\CanonLBP1120
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
OutputDir=.
OutputBaseFilename=CanonLBP1120_Setup_v1.0.0-rc.1_x64
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64compatible
DisableDirPage=no
DisableProgramGroupPage=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
; Core binary and voice prompts
Source: "..\target\release\capt-poc.exe"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion
Source: "..\reference\drivers\LBP1120_WinXP\CAP3ZWAV.BIN"; DestDir: "{app}"; Flags: ignoreversion
Source: "CanonLBP1120_WinUSB.inf"; DestDir: "{app}\driver"; Flags: ignoreversion
Source: "Install-CanonLBP1120.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "Uninstall-CanonLBP1120.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "CanonLBP1120_Setup.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "CanonLBP1120_Uninstall.bat"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Printer Status Diagnostics"; Filename: "{app}\{#MyAppExeName}"; Parameters: "probe"; IconFilename: "{sys}\shell32.dll"; IconIndex: 22
Name: "{group}\Reset Printer State"; Filename: "{app}\{#MyAppExeName}"; Parameters: "reset"; IconFilename: "{sys}\shell32.dll"; IconIndex: 238
Name: "{group}\Print 6-Zone Calibration Page"; Filename: "{app}\{#MyAppExeName}"; Parameters: "generate-calibration"; IconFilename: "{sys}\shell32.dll"; IconIndex: 16
Name: "{group}\View Service Log"; Filename: "{cmd}"; Parameters: "/c start notepad.exe C:\ProgramData\CanonLBP1120\service.log"; IconFilename: "{sys}\shell32.dll"; IconIndex: 70
Name: "{group}\Uninstall Driver Suite"; Filename: "{uninstallexe}"

[Run]
; 1. Stage and install WinUSB driver package via PnPUtil
Filename: "{sys}\pnputil.exe"; Parameters: "/add-driver ""{app}\driver\CanonLBP1120_WinUSB.inf"" /install"; Flags: runhidden waituntilterminated
; 2. Configure printer queues and ports
Filename: "{app}\{#MyAppExeName}"; Parameters: "install"; Flags: runhidden waituntilterminated
; 3. Register and start SCM background Windows Service
Filename: "{app}\{#MyAppExeName}"; Parameters: "install-service"; Flags: runhidden waituntilterminated

[UninstallRun]
; 1. Stop and remove SCM service
Filename: "{app}\{#MyAppExeName}"; Parameters: "uninstall-service"; RunOnceId: "UninstallLBP1120Service"; Flags: runhidden waituntilterminated
; 2. Remove printer queues and ports
Filename: "{app}\{#MyAppExeName}"; Parameters: "uninstall"; RunOnceId: "UninstallLBP1120Printer"; Flags: runhidden waituntilterminated

[UninstallDelete]
Type: files; Name: "C:\ProgramData\CanonLBP1120\service.log"
Type: dirifempty; Name: "C:\ProgramData\CanonLBP1120"
