; Inno Setup script. Build from the repo root:
;   iscc /DAppVersion=0.1.0 installer\easy-handoff.iss

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif
#define AppName "easy-handoff"
#define AppExe "easy-handoff.exe"

[Setup]
AppId={{B3B4C8A5-608E-45E1-B9B9-60E64591C989}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=tattsun
AppPublisherURL=https://github.com/tattsun/easy-handoff
LicenseFile=..\LICENSE
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
; Per-user install; no UAC prompt.
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename={#AppName}-setup-{#AppVersion}
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
japanese.StartupTask=Windows のログイン時に起動する
english.StartupTask=Launch at Windows sign-in

[Tasks]
Name: "startup"; Description: "{cm:StartupTask}"

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"

[Registry]
; Same value the app's "スタートアップに登録" menu item reads and writes.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#AppName}"; ValueData: """{app}\{#AppExe}"""; Tasks: startup

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[Code]
procedure KillApp;
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM {#AppExe}', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  KillApp;
  Result := '';
end;

function InitializeUninstall: Boolean;
begin
  KillApp;
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    // The app may have registered itself via its menu even if the task was unchecked.
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', '{#AppName}');
    // Settings written by the app (selected device).
    RegDeleteKeyIncludingSubkeys(HKCU, 'Software\{#AppName}');
  end;
end;
