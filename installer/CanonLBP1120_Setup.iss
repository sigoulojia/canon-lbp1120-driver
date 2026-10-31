; ==============================================================================
; Inno Setup 6/7 Script for Canon LaserShot LBP-1120 Open Source Driver Suite
; Generates standalone executable installer: CanonLBP1120_Setup_v1.0.0-rc.2_x64.exe
; ==============================================================================

#define MyAppName "Canon LaserShot LBP-1120 Driver Suite"
#define MyAppVersion "1.0.0-rc.2"
#define MyAppPublisher "Autonomous Hardware Research Team"
#define MyAppExeName "canon-lbp1120-service.exe"

[Setup]
AppId={{5C7F4349-D8D8-4BC4-B972-78A7E144DC3E}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={commonpf64}\CanonLBP1120
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
OutputDir=.
OutputBaseFilename=CanonLBP1120_Setup_v1.0.0-rc.2_x64
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=
ArchitecturesInstallIn64BitMode=x64compatible
DisableDirPage=no
DisableProgramGroupPage=yes
SetupLogging=yes

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

[UninstallRun]
; 1. Stop and remove SCM service
Filename: "{app}\{#MyAppExeName}"; Parameters: "uninstall-service"; RunOnceId: "UninstallLBP1120Service"; Flags: runhidden waituntilterminated
; 2. Remove printer queues and ports
Filename: "{app}\{#MyAppExeName}"; Parameters: "uninstall"; RunOnceId: "UninstallLBP1120Printer"; Flags: runhidden waituntilterminated

[UninstallDelete]
Type: files; Name: "C:\ProgramData\CanonLBP1120\service.log"
Type: dirifempty; Name: "C:\ProgramData\CanonLBP1120"

[Code]
function RunAppCmd(const AppPath, Params: String; var ExitCode: Integer): Boolean;
begin
  Log(Format('Executing: "%s" %s', [AppPath, Params]));
  Result := Exec(AppPath, Params, '', SW_HIDE, ewWaitUntilTerminated, ExitCode);
  Log(Format('Command Result: %d, ExitCode: %d', [Integer(Result), ExitCode]));
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ExePath: String;
  InfPath: String;
  PnpPath: String;
  ResultCode: Integer;
  ErrorMsg: String;
begin
  if CurStep = ssPostInstall then
  begin
    ExePath := ExpandConstant('{app}\{#MyAppExeName}');
    InfPath := ExpandConstant('{app}\driver\CanonLBP1120_WinUSB.inf');
    PnpPath := ExpandConstant('{sys}\pnputil.exe');

    // Step 1: Check existing USB driver binding
    Log('Step 1: Checking USB driver provenance and binding...');
    if RunAppCmd(ExePath, 'check-usb-driver', ResultCode) then
    begin
      if ResultCode = 2 then
      begin
        Log('No operational WinUSB driver found; staging CanonLBP1120_WinUSB.inf via pnputil...');
        RunAppCmd(PnpPath, Format('/add-driver "%s" /install', [InfPath]), ResultCode);
      end
      else
      begin
        Log('Existing WinUSB driver binding is active and operational. Preserving existing driver.');
      end;
    end;

    // Step 2: Configure printer queues and ports
    Log('Step 2: Configuring Windows printer queues and TCP RAW ports...');
    if (not RunAppCmd(ExePath, 'install', ResultCode)) or (ResultCode <> 0) then
    begin
      ErrorMsg := Format('Failed to configure printer queues and TCP ports (Exit code %d). Installation aborted.', [ResultCode]);
      SuppressibleMsgBox(ErrorMsg, mbError, MB_OK, IDOK);
      RaiseException(ErrorMsg);
    end;

    // Step 3: Register and start Windows Service
    Log('Step 3: Registering and starting Windows Service CanonLBP1120Service...');
    if (not RunAppCmd(ExePath, 'install-service', ResultCode)) or (ResultCode <> 0) then
    begin
      ErrorMsg := Format('Failed to register and start Windows Service "CanonLBP1120Service" (Exit code %d). Installation aborted.', [ResultCode]);
      SuppressibleMsgBox(ErrorMsg, mbError, MB_OK, IDOK);
      RaiseException(ErrorMsg);
    end;

    // Step 4: Verify post-installation state
    Log('Step 4: Running full post-installation verification audit...');
    if (not RunAppCmd(ExePath, 'verify-installation', ResultCode)) or (ResultCode <> 0) then
    begin
      ErrorMsg := Format('Post-installation verification failed (Exit code %d). Service or listeners inactive. Installation aborted.', [ResultCode]);
      SuppressibleMsgBox(ErrorMsg, mbError, MB_OK, IDOK);
      RaiseException(ErrorMsg);
    end;

    Log('Canon LaserShot LBP-1120 Driver Suite successfully installed and verified!');
  end;
end;
