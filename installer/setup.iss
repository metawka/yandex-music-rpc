#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif

[Setup]
AppId={{8F3C2A6E-5B1D-4E7A-9C2F-1D6B7E4A9F30}
AppName=Yandex Music RPC
AppVersion={#AppVersion}
AppPublisher=metawka
AppPublisherURL=https://github.com/metawka/yandex-music-rpc
AppSupportURL=https://github.com/metawka/yandex-music-rpc/issues
DefaultDirName={localappdata}\Programs\YandexMusicRPC
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=YandexMusicRPC-Setup
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\YandexMusicRPC.exe
UninstallDisplayName=Yandex Music RPC
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
ru.Autostart=Запускать вместе с Windows
en.Autostart=Start with Windows
ru.LaunchNow=Запустить Yandex Music RPC
en.LaunchNow=Launch Yandex Music RPC

[Tasks]
Name: "autostart"; Description: "{cm:Autostart}"

[Files]
Source: "..\target\release\yandex-music-rpc.exe"; DestDir: "{app}"; DestName: "YandexMusicRPC.exe"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Yandex Music RPC"; Filename: "{app}\YandexMusicRPC.exe"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "YandexMusicRPC"; ValueData: """{app}\YandexMusicRPC.exe"""; Tasks: autostart
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "YandexMusicRPC"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\YandexMusicRPC.exe"; Description: "{cm:LaunchNow}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM YandexMusicRPC.exe"; Flags: runhidden; RunOnceId: "StopApp"

[Code]
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM YandexMusicRPC.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Result := '';
end;
