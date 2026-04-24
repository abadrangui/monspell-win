; monspell Inno Setup script.
;
; Forked from the upstream `install.iss` (WinDivvun) but kept as a parallel
; file so upstream rebases do not conflict. See docs/fork-divergence.md.
;
; Phase 0 scope:
;  - 32-bit only (matches the unmodified fork's i686-pc-windows-msvc target).
;  - Ships windivvun.dll + a stub mn.bhfst. No divvunspellmso.dll
;    (that would be needed for Office 2010-2019 Custom Proofing registration,
;    and we are not building it in Phase 0 — Office 365 uses WSCAPI directly).
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
ArchitecturesInstallIn64BitMode=
ArchitecturesAllowed=x86 x64

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
; 32-bit windivvun DLL — loaded by Word / Outlook / UWP via WSCAPI.
Source: "target\release\windivvun.dll"; DestDir: "{app}\i686\"; Flags: ignoreversion restartreplace uninsrestartdelete
; Stub Mongolian speller — flags every word, suggests the input back.
; Will be replaced by a real Mongolian FST in Phase 1.
Source: "Spellers\mn.bhfst"; DestDir: "{app}\Spellers\"; Flags: ignoreversion

[Dirs]
Name: "{app}\Spellers"

[Registry]
; WSCAPI provider discovery. Windows reads this key to find our
; ISpellCheckProviderFactory and instantiate it via the CLSID below.
; Rename from upstream's "Divvun" to "monspell" so we do not clobber a
; WinDivvun install on the same machine.
Root: HKLM; Subkey: "SOFTWARE\Microsoft\Spelling\Spellers\monspell"; ValueType: string; ValueName: "CLSID"; ValueData: "{#Clsid}"; Flags: uninsdeletekey

; COM class registration. InProcServer32 points Windows at the DLL to load
; when an app asks for this CLSID.
Root: HKLM; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}"; ValueType: string; ValueName: ""; ValueData: "monspell Spell Checking Service"; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}"; ValueType: string; ValueName: "AppId"; ValueData: "{#Clsid}"; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}\Version"; ValueType: string; ValueName: ""; ValueData: "{#MyAppVersion}"; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}\InProcServer32"; ValueType: string; ValueName: ""; ValueData: "{app}\i686\windivvun.dll"; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\CLSID\{#Clsid}\InProcServer32"; ValueType: string; ValueName: "ThreadingModel"; ValueData: "Both"; Flags: uninsdeletekey

; Our DLL scans two hard-coded locations for .bhfst files (see
; windivvun-service/src/lib.rs::SPELLER_REPOSITORY). The second one is
; derived from the DLL's own install path: <install_root>\Spellers\.
; The [Dirs] section above already created that folder; [Files] above
; puts mn.bhfst in it. So no extra per-language registry entries are
; required for Phase 0.
