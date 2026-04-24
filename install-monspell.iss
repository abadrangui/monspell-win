; monspell Inno Setup script.
;
; Forked from the upstream `install.iss` (WinDivvun) but kept as a parallel
; file so upstream rebases do not conflict. See docs/fork-divergence.md.
;
; Phase 0 scope:
;  - Both 32-bit and 64-bit windivvun.dll. WSCAPI provider DLLs must match
;    the bitness of the app loading them (64-bit Word needs 64-bit DLL;
;    32-bit Office or UWP apps need 32-bit). Registered in both HKLM32 +
;    HKLM64 registry views so Windows finds the right one per caller.
;  - Ships a stub mn.bhfst. No divvunspellmso.dll (Office 2010-2019 Custom
;    Proofing registration is deferred past Phase 0 — Office 365 uses
;    WSCAPI directly).
;  - Ships no spelli.exe — this script does all the registry writes inline.
;  - Unsigned per ADR-0003.

#define MyAppName "monspell"
#define MyAppVersion "0.1.0-beta"
#define MyAppPublisher "abadrangui"
#define MyAppURL "https://github.com/abadrangui/monspell"

; UUIDs generated fresh for monspell so we never collide with WinDivvun's
; registrations if both products are on the same machine.
#define MonspellAppId "{{1BA62C41-FCD1-4350-B2E1-A959B77CA711}"
#define Clsid "{{AAC10BA6-2952-4734-AAE7-02DB0C9FE6DE}"

[Setup]
AppId={#MonspellAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={commonpf}\monspell
DisableDirPage=yes
DisableProgramGroupPage=yes
OutputBaseFilename=monspell-{#MyAppVersion}-setup
OutputDir=dist
Compression=lzma
SolidCompression=yes
MinVersion=6.3.9200
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x86 x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
; 32-bit windivvun DLL — loaded by 32-bit apps (legacy Office, some UWP hosts).
Source: "target\release\windivvun.dll"; DestDir: "{app}\i686\"; Flags: ignoreversion restartreplace uninsrestartdelete
; 64-bit windivvun DLL — loaded by 64-bit Word / Outlook / most Office 365 installs.
Source: "target\x86_64-pc-windows-msvc\release\windivvun.dll"; DestDir: "{app}\x86_64\"; Flags: ignoreversion restartreplace uninsrestartdelete; Check: IsWin64
; Stub Mongolian speller — flags every word, suggests the input back.
; Will be replaced by a real Mongolian FST in Phase 1.
Source: "Spellers\mn.bhfst"; DestDir: "{app}\Spellers\"; Flags: ignoreversion

[Dirs]
Name: "{app}\Spellers"

[Registry]
; ====================================================================
; 64-bit registrations (HKLM64 = native view on x64 Windows).
; Loaded by 64-bit Word / Outlook / 64-bit UWP apps.
; ====================================================================
Root: HKLM64; Subkey: "SOFTWARE\Microsoft\Spelling\Spellers\monspell"; ValueType: string; ValueName: "CLSID"; ValueData: "{#Clsid}"; Flags: uninsdeletekey; Check: IsWin64
Root: HKLM64; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}"; ValueType: string; ValueName: ""; ValueData: "monspell Spell Checking Service"; Flags: uninsdeletekey; Check: IsWin64
Root: HKLM64; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}"; ValueType: string; ValueName: "AppId"; ValueData: "{#Clsid}"; Flags: uninsdeletekey; Check: IsWin64
Root: HKLM64; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}\Version"; ValueType: string; ValueName: ""; ValueData: "{#MyAppVersion}"; Flags: uninsdeletekey; Check: IsWin64
Root: HKLM64; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}\InProcServer32"; ValueType: string; ValueName: ""; ValueData: "{app}\x86_64\windivvun.dll"; Flags: uninsdeletekey; Check: IsWin64
Root: HKLM64; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}\InProcServer32"; ValueType: string; ValueName: "ThreadingModel"; ValueData: "Both"; Flags: uninsdeletekey; Check: IsWin64

; ====================================================================
; 32-bit registrations (HKLM32 = WOW6432Node on x64 Windows, native on x86).
; Loaded by 32-bit Office or 32-bit apps calling WSCAPI.
; ====================================================================
Root: HKLM32; Subkey: "SOFTWARE\Microsoft\Spelling\Spellers\monspell"; ValueType: string; ValueName: "CLSID"; ValueData: "{#Clsid}"; Flags: uninsdeletekey
Root: HKLM32; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}"; ValueType: string; ValueName: ""; ValueData: "monspell Spell Checking Service"; Flags: uninsdeletekey
Root: HKLM32; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}"; ValueType: string; ValueName: "AppId"; ValueData: "{#Clsid}"; Flags: uninsdeletekey
Root: HKLM32; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}\Version"; ValueType: string; ValueName: ""; ValueData: "{#MyAppVersion}"; Flags: uninsdeletekey
Root: HKLM32; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}\InProcServer32"; ValueType: string; ValueName: ""; ValueData: "{app}\i686\windivvun.dll"; Flags: uninsdeletekey
Root: HKLM32; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}\InProcServer32"; ValueType: string; ValueName: "ThreadingModel"; ValueData: "Both"; Flags: uninsdeletekey

; Our DLL scans two hard-coded locations for .bhfst files (see
; windivvun-service/src/lib.rs::SPELLER_REPOSITORY). The second one is
; derived from the DLL's own install path: <install_root>\Spellers\.
; The [Dirs] section above already created that folder; [Files] above
; puts mn.bhfst in it. So no extra per-language registry entries are
; required for Phase 0.
