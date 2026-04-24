; monspell Inno Setup script.
;
; Forked from the upstream `install.iss` (WinDivvun) but kept as a parallel
; file so upstream rebases do not conflict. See docs/fork-divergence.md.
;
; Phase 0 scope:
;  - windivvun.dll (WSCAPI provider) — both 32-bit and 64-bit. WSCAPI provider
;    DLLs must match the bitness of the app loading them (64-bit Word needs
;    64-bit DLL; 32-bit Office or UWP apps need 32-bit). Registered in both
;    HKLM32 + HKLM64 registry views so Windows finds the right one per caller.
;  - monspell_mso.dll (CSAPI "say-yes" stub) — both bitnesses. Required
;    because Word 2016+ will not dispatch WSCAPI for a language unless a
;    CSAPI proofing tool is also registered for it. Microsoft never shipped
;    a CSAPI proofing tool for Mongolian, so we ship our own minimum-viable
;    stub. Registered at both the legacy Override\<lang>\LEX/DLL path and
;    the Spelling\<LCID>\Normal\Engine/Dictionary path.
;  - Stub mn.bhfst (WSCAPI dictionary) and empty mn.lex (CSAPI dictionary
;    placeholder). The real Mongolian FST arrives in Phase 1 from lang-khk.
;  - No spelli.exe — this script does all the registry writes inline.
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
; 32-bit windivvun DLL (WSCAPI) — loaded by 32-bit apps (legacy Office, some UWP hosts).
Source: "target\release\windivvun.dll"; DestDir: "{app}\i686\"; Flags: ignoreversion restartreplace uninsrestartdelete
; 64-bit windivvun DLL (WSCAPI) — loaded by 64-bit Word / Outlook / most Office 365 installs.
Source: "target\x86_64-pc-windows-msvc\release\windivvun.dll"; DestDir: "{app}\x86_64\"; Flags: ignoreversion restartreplace uninsrestartdelete; Check: IsWin64
; 32-bit monspell_mso DLL (CSAPI "say-yes" stub) — unlocks Word's language gate for 32-bit Office.
Source: "mso\target\release\monspell_mso.dll"; DestDir: "{app}\i686\"; Flags: ignoreversion restartreplace uninsrestartdelete
; 64-bit monspell_mso DLL (CSAPI "say-yes" stub) — unlocks Word's language gate for 64-bit Office 365.
Source: "mso\target\x86_64-pc-windows-msvc\release\monspell_mso.dll"; DestDir: "{app}\x86_64\"; Flags: ignoreversion restartreplace uninsrestartdelete; Check: IsWin64
; WSCAPI speller data — stub, flags every word.
Source: "Spellers\mn.bhfst"; DestDir: "{app}\Spellers\"; Flags: ignoreversion
; CSAPI lexicon placeholder — empty; our CSAPI stub never reads it, but Word
; wants the registered LEX path to exist on disk before it will load the DLL.
Source: "Spellers\mn.lex"; DestDir: "{app}\Spellers\"; Flags: ignoreversion

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
; required for WSCAPI discovery.

; ====================================================================
; Office CSAPI (Custom Proofing Tool) registration.
;
; Required for Word 2016+ to dispatch WSCAPI for Mongolian. Without these
; keys Word's "Set Proofing Language" dialog greys out / shows a "MISSING
; PROOFING TOOLS" banner and refuses to call WSCAPI even when WSCAPI has a
; registered provider. See github.com/abadrangui/monspell-win
; docs/fork-divergence.md for the research that established this.
;
; Two paths are required, BOTH written:
;
;  - Override\mn-MN        — Word consults this for the language->DLL map.
;                            Writing here is what unlocks the language in
;                            the proofing-language picker. DLL64/LEX64 for
;                            x64 Word; DLL/LEX for x86 Office.
;  - Spelling\1104\Normal  — the engine-by-LCID path. 1104 = 0x0450 = mn-MN.
;                            Contains the actual Engine + Dictionary values
;                            the speller loader reads at runtime.
;
; Duplicated across HKLM64 and HKLM32 (Wow6432Node) because 32-bit Office
; and 64-bit Office look in different registry views.
; ====================================================================

; --- 64-bit Office proofing registration ---
Root: HKLM64; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\1.0\Override\mn-MN"; ValueType: string; ValueName: "DLL64"; ValueData: "{app}\x86_64\monspell_mso.dll"; Flags: uninsdeletekey; Check: IsWin64
Root: HKLM64; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\1.0\Override\mn-MN"; ValueType: string; ValueName: "LEX64";  ValueData: "{app}\Spellers\mn.lex";            Flags: uninsdeletekey; Check: IsWin64
Root: HKLM64; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\1.0\Override\mn-MN"; ValueType: string; ValueName: "DLL";   ValueData: "{app}\i686\monspell_mso.dll";   Flags: uninsdeletekey; Check: IsWin64
Root: HKLM64; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\1.0\Override\mn-MN"; ValueType: string; ValueName: "LEX";   ValueData: "{app}\Spellers\mn.lex";            Flags: uninsdeletekey; Check: IsWin64

Root: HKLM64; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\Spelling\1104\Normal"; ValueType: string; ValueName: "Engine";     ValueData: "{app}\x86_64\monspell_mso.dll"; Flags: uninsdeletekey; Check: IsWin64
Root: HKLM64; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\Spelling\1104\Normal"; ValueType: string; ValueName: "Dictionary"; ValueData: "{app}\Spellers\mn.lex";            Flags: uninsdeletekey; Check: IsWin64

; --- 32-bit Office proofing registration (Wow6432Node) ---
Root: HKLM32; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\1.0\Override\mn-MN"; ValueType: string; ValueName: "DLL64"; ValueData: "{app}\x86_64\monspell_mso.dll"; Flags: uninsdeletekey; Check: IsWin64
Root: HKLM32; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\1.0\Override\mn-MN"; ValueType: string; ValueName: "LEX64";  ValueData: "{app}\Spellers\mn.lex";            Flags: uninsdeletekey; Check: IsWin64
Root: HKLM32; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\1.0\Override\mn-MN"; ValueType: string; ValueName: "DLL";   ValueData: "{app}\i686\monspell_mso.dll";   Flags: uninsdeletekey
Root: HKLM32; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\1.0\Override\mn-MN"; ValueType: string; ValueName: "LEX";   ValueData: "{app}\Spellers\mn.lex";            Flags: uninsdeletekey

Root: HKLM32; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\Spelling\1104\Normal"; ValueType: string; ValueName: "Engine";     ValueData: "{app}\i686\monspell_mso.dll"; Flags: uninsdeletekey
Root: HKLM32; Subkey: "SOFTWARE\Microsoft\Shared Tools\Proofing Tools\Spelling\1104\Normal"; ValueType: string; ValueName: "Dictionary"; ValueData: "{app}\Spellers\mn.lex";           Flags: uninsdeletekey

[Code]
// ========================================================================
// Office User Settings + Create + Count propagation.
//
// Direct writes to Override\mn-MN / Spelling\1104\Normal above work on
// classic MSI Office installs, but Office 365 Click-to-Run virtualises
// those paths into its own private registry tree at
//   HKLM\SOFTWARE\Microsoft\Office\ClickToRun\REGISTRY\MACHINE\Software\...
// Direct writes into that virtualised tree are also ignored — Word does
// not pick them up.
//
// The documented workaround (used by Divvun's `spelli`) is User Settings
// propagation: write the target keys under
//   <OfficeRoot>\User Settings\<app>\Create\<target_subpath>
// plus a `Count` DWORD on the User Settings\<app> key. When Word starts
// it reads pending Create subtrees and applies them to their real targets,
// which Office's registry layer routes to the right place regardless of
// whether the install is MSI or C2R.
//
// We cover four Office 16.0 roots so the install works on Office 365 C2R
// (both 64-bit and 32-bit) and classic MSI Office 2016/2019 (both views):
//   - SOFTWARE\Microsoft\Office\16.0
//   - SOFTWARE\Wow6432Node\Microsoft\Office\16.0
//   - SOFTWARE\Microsoft\Office\ClickToRun\REGISTRY\MACHINE\Software\Microsoft\Office\16.0
//   - SOFTWARE\Microsoft\Office\ClickToRun\REGISTRY\MACHINE\Software\Wow6432Node\Microsoft\Office\16.0
// Office 14.0 / 15.0 (2010 / 2013) are out of scope for Phase 0.
// ========================================================================

function GetMonspellCount(Param: String): String;
begin
  // `Count` is the signal to Office that User Settings\<app>\Create has
  // un-applied entries. We only need a non-zero DWORD that differs between
  // fresh installs. yyMMddHHmm packs into 10 digits → max ~9912312359, which
  // fits in u32 (max 4294967295) from year 2043 backwards; fine for our
  // lifetime. GetDateTimeString is Inno's Pascal-runtime-safe way to read
  // system time without TSystemTime bindings.
  Result := GetDateTimeString('yymmddhhnn', #0, #0);
end;

procedure WriteProofingOverride(RootKey: Integer; BasePath: String; InstallPath: String);
var
  CreatePath: String;
  UsKey: String;
  LangTag: String;
  LangTags: array of String;
  i: Integer;
begin
  UsKey := BasePath + '\User Settings\monspell';

  // Count + Order on the parent node — this is the signal to Office to
  // apply the Create subtree on next startup.
  RegWriteDWordValue(RootKey, UsKey, 'Count', StrToInt(GetMonspellCount('')));
  RegWriteDWordValue(RootKey, UsKey, 'Order', 1);

  // One Override\<tag> per BCP-47 variant Word might present.
  // Mongolian (Cyrillic) tags per docs/GLOSSARY.md § "Language tags".
  SetArrayLength(LangTags, 4);
  LangTags[0] := 'mn';
  LangTags[1] := 'mn-MN';
  LangTags[2] := 'mn-Cyrl';
  LangTags[3] := 'mn-Cyrl-MN';

  for i := 0 to GetArrayLength(LangTags) - 1 do begin
    LangTag := LangTags[i];
    CreatePath := UsKey + '\Create\SOFTWARE\Microsoft\Shared Tools\Proofing Tools\1.0\Override\' + LangTag;
    RegWriteStringValue(RootKey, CreatePath, 'DLL64', InstallPath + '\x86_64\monspell_mso.dll');
    RegWriteStringValue(RootKey, CreatePath, 'LEX64', InstallPath + '\Spellers\mn.lex');
    RegWriteStringValue(RootKey, CreatePath, 'DLL',   InstallPath + '\i686\monspell_mso.dll');
    RegWriteStringValue(RootKey, CreatePath, 'LEX',   InstallPath + '\Spellers\mn.lex');
  end;

  // Engine + Dictionary under Spelling\1104\Normal for LCID 0x0450 (mn-MN).
  CreatePath := UsKey + '\Create\SOFTWARE\Microsoft\Shared Tools\Proofing Tools\Spelling\1104\Normal';
  RegWriteStringValue(RootKey, CreatePath, 'Engine',     InstallPath + '\x86_64\monspell_mso.dll');
  RegWriteStringValue(RootKey, CreatePath, 'Dictionary', InstallPath + '\Spellers\mn.lex');
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  InstallPath: String;
begin
  if CurStep <> ssPostInstall then Exit;
  InstallPath := ExpandConstant('{app}');

  // HKLM 64-bit view, classic (MSI) Office 16.0
  WriteProofingOverride(HKLM64, 'SOFTWARE\Microsoft\Office\16.0', InstallPath);
  // HKLM 32-bit view (Wow6432Node), classic (MSI) Office 16.0
  WriteProofingOverride(HKLM64, 'SOFTWARE\Wow6432Node\Microsoft\Office\16.0', InstallPath);
  // Office 365 Click-to-Run virtual registry, 64-bit side
  WriteProofingOverride(HKLM64, 'SOFTWARE\Microsoft\Office\ClickToRun\REGISTRY\MACHINE\Software\Microsoft\Office\16.0', InstallPath);
  // Office 365 Click-to-Run virtual registry, 32-bit side
  WriteProofingOverride(HKLM64, 'SOFTWARE\Microsoft\Office\ClickToRun\REGISTRY\MACHINE\Software\Wow6432Node\Microsoft\Office\16.0', InstallPath);
end;

procedure CleanupProofingOverride(RootKey: Integer; BasePath: String);
begin
  // Delete the entire User Settings\monspell subtree on uninstall.
  RegDeleteKeyIncludingSubkeys(RootKey, BasePath + '\User Settings\monspell');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep <> usUninstall then Exit;
  CleanupProofingOverride(HKLM64, 'SOFTWARE\Microsoft\Office\16.0');
  CleanupProofingOverride(HKLM64, 'SOFTWARE\Wow6432Node\Microsoft\Office\16.0');
  CleanupProofingOverride(HKLM64, 'SOFTWARE\Microsoft\Office\ClickToRun\REGISTRY\MACHINE\Software\Microsoft\Office\16.0');
  CleanupProofingOverride(HKLM64, 'SOFTWARE\Microsoft\Office\ClickToRun\REGISTRY\MACHINE\Software\Wow6432Node\Microsoft\Office\16.0');
end;
