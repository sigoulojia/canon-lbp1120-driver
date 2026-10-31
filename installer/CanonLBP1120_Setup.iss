; Inno Setup Commercial Installer Script for Canon LaserShot LBP-1120 Modern x64 Driver Suite
[Setup]
AppId={{8F9E4A21-1120-4C89-A0A0-CANONLBP1120}
AppName=Canon LaserShot LBP-1120 Modern x64 Driver & Management Suite
AppVersion=2.0.0
AppPublisher=Open CAPT Hardware Preservation Project
DefaultDirName={autopf}\CanonLBP1120
DefaultGroupName=Canon LaserShot LBP-1120
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
OutputBaseFilename=Canon_LBP1120_x64_Setup_v2.0.0
Compression=lzma2/ultra64
SolidCompression=yes
UninstallDisplayIcon={app}\capt-poc.exe

[Files]
Source: "..\target\release\capt-poc.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\Cap3ht.bin"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "..\assets\sounds\*.wav"; DestDir: "{app}\assets\sounds"; Flags: ignoreversion
Source: "..\fixtures\test-3-pages.pdf"; DestDir: "{app}\fixtures"; Flags: ignoreversion

[Run]
Filename: "{app}\capt-poc.exe"; Parameters: "install"; StatusMsg: "Registering Canon LBP-1120 Print Queues, TCP/Named Pipe Ports, and Background Service..."; Flags: runhidden waituntilterminated
Filename: "{app}\capt-poc.exe"; Parameters: "probe"; Description: "Verify USB Hardware Connection (04A9:262B)"; Flags: postinstall nowait skipifsilent

[UninstallRun]
Filename: "{app}\capt-poc.exe"; Parameters: "uninstall"; StatusMsg: "Removing Print Queues, Ports, and Background Service..."; Flags: runhidden waituntilterminated
