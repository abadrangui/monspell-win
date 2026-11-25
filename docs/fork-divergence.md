# Fork divergence log

Every deliberate divergence between `abadrangui/monspell-win` and upstream `divvun/windivvun-service` is recorded here at the time of divergence. Entries are append-only. Retroactive edits are not acceptable — see `CONTRIBUTING.md` § "Rebase discipline."

Entry format:

```
## YYYY-MM-DD — short title
- **What:** the divergence in one sentence.
- **Where:** the file(s) and roughly where in the file.
- **Why not upstream:** the reason this cannot be submitted as a PR to `divvun/windivvun-service`.
- **Revisit trigger:** the event that would cause us to revisit whether this divergence should still exist (e.g. upstream accepting a related change, a registry-collision complaint, an API change).
- **Links:** commits, ADRs, or issues that bear on the divergence.
```

The related architectural decision lives in [`monspell`'s ADR-0002](https://github.com/abadrangui/monspell/blob/main/docs/decisions/0002-windows-architecture-close-fork.md).

---

## 2026-04-24 — monspell_mso.dll: CSAPI "say-yes" stub to unlock Word

- **What:** New sibling crate `mso/` producing `monspell_mso.dll` (both i686 and x86_64). Implements 16 Office Custom Speller Interface (CSAPI) exports as pass-through stubs: `SpellerVersion`, `SpellerInit`, `SpellerTerminate`, `SpellerSetOptions`, `SpellerGetOptions`, `SpellerOpenLex`, `SpellerCloseLex`, `SpellerCheck`, `SpellerAddUdr`, `SpellerAddChangeUdr`, `SpellerDelUdr`, `SpellerClearUdr`, `SpellerGetSizeUdr`, `SpellerGetListUdr`, `SpellerBuiltinUdr`, `DllCanUnloadNow`. Every function returns 0 (success) with zero results; `SpellerCheck` explicitly reports "buffer consumed, no errors found". The installer registers this DLL under both `HKLM\SOFTWARE\Microsoft\Shared Tools\Proofing Tools\1.0\Override\mn-MN` and `HKLM\SOFTWARE\Microsoft\Shared Tools\Proofing Tools\Spelling\1104\Normal`, mirrored across HKLM64 and HKLM32, pointing `DLL64`/`LEX64` at the x86_64 DLL + empty `Spellers\mn.lex` and `DLL`/`LEX` at the i686 variant.
- **Where:** `mso/Cargo.toml`, `mso/src/lib.rs`, `mso/build.rs`, `mso/monspell_mso.def`, `Spellers/mn.lex` (empty placeholder), additions to `install-monspell.iss` `[Files]` and `[Registry]` sections, `/mso/target/` excluded in `.gitignore`.
- **Why not upstream:** Upstream (`divvun/windivvun-service`) is strictly the WSCAPI side. The analogous Divvun component is `divvunspellmso.dll`, whose source lives in Divvun's private SVN `gtsvn.uit.no:divvunspell-mso` and has no public repo. Our implementation is original; the 16-export surface and `__stdcall` arg counts were reverse-engineered from Divvun's public x86 binary (`pahkat.uit.no/tools/download/windivvun`) via `dumpbin /EXPORTS` + `dumpbin /DISASM` (finding each `ret NN` epilogue), cross-checked against Microsoft's own StyleCop (`github.com/StyleCop/StyleCop`, MS-PL) for signature types and the magic API version `0x03000000`. Microsoft's own CSAPI spec is NDA-gated per KB 262605; neither the Word-loader behaviour nor the DLL surface is publicly documented beyond those two sources. This DLL is therefore permanently fork-local and inherently NOT upstream-candidate.
- **Revisit trigger:**
  - Upstream Divvun open-sources their `divvunspellmso` crate: we rebase our MSO crate onto theirs, or depend on it.
  - Phase 1 ships the real Mongolian FST: the `SpellerCheck` stub stays pass-through (let WSCAPI do the work), but `SpellerOpenLex` may want to hand Word a real lexicon handle for compat with older Office versions — revisit at that point.
  - Microsoft releases a native proofing pack for Mongolian, making our CSAPI registration redundant: uninstall cleanly via the `uninsdeletekey` flag and delete this component.
- **Links:** `https://github.com/StyleCop/StyleCop/blob/master/Project/Src/StyleCop/Spelling/SpellChecker.cs` (inverted P/Invoke is where the struct layouts and version constant came from); `https://github.com/microsoft/Microsoft-3D-Movie-Maker/blob/main/kauai/SRC/CSAPI.H` (MIT-licensed legacy header, confirms the `Speller*` naming lineage); `https://learn.microsoft.com/en-us/archive/msdn-technet-forums/7022763a-d39d-4d39-a850-9bc7e7ea0529` (archived MSDN thread confirming `DLL64`/`LEX64` requirement on x64 Office).

## 2026-04-24 — Parallel Inno Setup script for monspell branding + stub speller

- **What:** Added `install-monspell.iss` alongside upstream `install.iss` (kept pristine). Added `Spellers/mn.bhfst` (stub dictionary — flags every word, input-as-suggestion). Added `/dist/` to `.gitignore`.
- **Where:** fork root — `install-monspell.iss`, `Spellers/mn.bhfst`, `.gitignore`.
- **Why not upstream:**
  - Publisher / AppId / CLSID / install path are monspell-specific; patching upstream's file would guarantee collisions in every rebase.
  - Phase 0 intentionally skips `divvunspellmso.dll` (Office 2010–2019 Custom Proofing) and `spelli.exe` (the installer script writes the registry keys inline). Upstream's `install.iss` references both via hard-coded `artifacts\...` paths.
  - `Spellers/mn.bhfst` is a Phase 0 stub built from `divvunspell/tests/fixtures/{eps-lexicon,identity-mutator}.thfst` via `thfst-tools thfsts-to-bhfst`. The real Mongolian speller comes in Phase 1 from `giellalt/lang-khk`.
  - Fresh UUIDs: `AppId = {1BA62C41-FCD1-4350-B2E1-A959B77CA711}`, `CLSID = {AAC10BA6-2952-4734-AAE7-02DB0C9FE6DE}`. Picked new rather than reused from upstream so WinDivvun and monspell can co-install without clobbering each other's COM/WSCAPI registrations.
- **Revisit trigger:** Phase 1 replaces `Spellers/mn.bhfst` with a real FST built from `lang-khk`. Phase 6 (Native OS integrations) will likely need `divvunspellmso.dll` for Office 2013–2019 coverage — if we build our own or consume Divvun's, this script's `[Files]` section expands. Consider re-integrating `spelli` if the registry write needs grow beyond what Inno `[Registry]` can express.
- **Links:** commits adding the files on this branch; main `monspell` repo CHANGELOG "2026-04-24 — Phase 0 installer built" entry.

## 2026-04-24 — Gate off bitrotted upstream `tokens()` test

- **What:** Added `#[cfg(any())]` above the existing `#[test] fn tokens()` in `src/spell_impl/EnumSpellingError.rs` so it does not compile.
- **Where:** `src/spell_impl/EnumSpellingError.rs`, the `#[test]` block that previously started with `let res: Vec<Token> = "Hello world how are you doing".tokenize()`.
- **Why not upstream:** The test is bitrotted against the current `divvunspell::tokenizer` API. It references a `Token` enum and a `.tokenize()` method on `&str`, but the present `Tokenize` trait exposes `word_bound_indices`, `word_indices`, `word_bound_indices_with_alphabet`, and `words_with_alphabet` — and there is no `Token` enum (the closest type is `IndexedWord`). `cargo test` therefore refuses to compile the lib-test binary at all, which blocks *any* test from running in this crate. Rewriting the test is possible but requires deciding what it should actually assert; gating it off is the minimum change to unblock `cargo test` for the `HARDCODED_TAG_TABLE` tests added in `feature/mn-tag-table`. The rewrite is upstream-candidate work and should be submitted as a separate PR; this gate-off is fork-local until that lands.
- **Revisit trigger:** Either upstream PRs a rewritten `tokens()` test against the current API, or this fork rewrites it as a commons-contribution PR. Either way the `#[cfg(any())]` comes off in the rebase that picks up the rewrite.
- **Links:** commit on `feature/mn-tag-table` that added the gate; main `monspell` repo CHANGELOG.md "2026-04-24 — Phase 0 build unblocked" entry.

### 2026-07-14 — mso v0.2.0-beta: real CSAPI spell-check via monspell-core
- **What:** `mso/` rewritten from say-yes stub to real engine: per-word `SpellerCheck` (VerifyBuffer → first misspelled token span, UTF-16 offsets; Suggest → caller-arena suggestion marshalling per StyleCop's documented WSRB/SPELLERSUGGESTION contract). Engine = `monspell-core` (dict-mn Hunspell + optional lang-khk FST), located via `MONSPELL_ENGINE_DIR` → DLL-sibling/parent `Spellers\` → `%APPDATA%\monspell\engine`. Engine failure degrades to no-errors, never crashes Word. In-process contract test in `mso/tests/contract.rs` (runs on any OS). `install-monspell.iss` ships `mn_MN.aff/.dic` + `freq.tsv` into `{app}\Spellers\`.
- **Why fork-local:** CSAPI shim is monspell-specific; upstream has its own divvunspellmso.
- **Dependency note:** `monspell-core` git dep points at `abadrangui/monspell` (private as of today — CI patches to a local checkout; flips to plain git once the repo is public).
- **Revisit trigger:** Word VM verification of suggestion-menu behaviour (rating scale + sstat semantics for Suggest are best-effort from public sources).

## 2026-11-25 — WSCAPI provider: host crash fixed, Mongolian served by monspell-core

- **What happened:** `ISpellCheckerFactory::CreateSpellChecker("mn")` killed `msspellcheckinghost.exe` on every call (Application log Event 1000, module `windivvun.dll`, exception `0xc0000409`). Reproduced in-process: `memory allocation of 59007793542870016 bytes failed`. The Phase 0 stub `Spellers/mn.bhfst` is a BOX container **version 1** (written by box-format 0.4.0 through current `thfst-tools`). This crate's lock pins divvunspell `7949c6f` → box-format **0.3.2**, which only knows version 0 and never checks the version field. It parsed the v1 trailer with the v0 layout, read a record count of 567,382,630,219,904 and called `Vec::with_capacity` on it (`box-format-0.3.2/src/de.rs:27`). A failed allocation aborts; it is not a panic, so no `catch_unwind` can stop it. The host serves every WSCAPI app, so this is not a Mongolian-only failure.
- **What changed (upstream-candidate, each one is a bug upstream has too):**
  - `src/guard.rs` (new) + every COM method and DLL export: bodies run under `catch_unwind`; a panic becomes `E_UNEXPECTED` and a line in `%LOCALAPPDATA%\monspell\windivvun.log`. Null out-pointers return `E_POINTER`. All `unwrap()`/`expect()` on these paths are gone (`src/lib.rs`, `src/util.rs`, `src/spell_impl/*`).
  - `src/speller_repository.rs`: `is_readable_archive` refuses any `.bhfst` that is not a well-formed BOX version 0 container before divvunspell sees it. This is the only defence against the allocation abort above.
  - `src/spell_impl/EnumSpellingError.rs`: error positions are now UTF-16 units. The old code mixed byte offsets with character indexes, which gave wrong (in debug builds: panicking) lengths for any non-ASCII text. `get_Replacement` returns a `CoTaskMemAlloc` string; it used to hand out a pointer into the object, which the caller then freed.
  - `src/spell_impl/SpellCheckProvider.rs`: `InitializeWordlist` no longer panics on a custom-dictionary entry with invalid UTF-16 or on a miscounting enumerator.
- **What changed (fork-local):**
  - `src/monspell_bridge.rs`, `src/engine.rs` (new): for `mn` tags the provider uses the monspell engine instead of a divvunspell archive. The engine (monspell-core, edition 2024) cannot be linked into this crate (pinned to `nightly-2023-03-15`), so it is loaded at runtime from the sibling `monspell_mso.dll` by full path and called through six C exports (`mso/src/wsc.rs`, ABI version 1: UTF-16, caller-allocated buffers, every export panic-contained). Word and WSCAPI apps therefore get the same verdicts from one engine, one tokenizer and one copy of dict-mn.
  - `mso/src/engine.rs`: tokenizer is resumable (`next_token`); a hyphen-led suffix on a Latin word (`Excel-д`) is skipped like the existing digit case (`5-ны`); a poisoned engine lock no longer disables the engine; the reason for a failed engine load is kept and reported.
  - **The host is sandboxed.** `msspellcheckinghost.exe` runs in an AppContainer (`microsoft^windows^spellchecking^host`). It can read `Program Files` but not the user's profile, so monspell-core's default custom-dictionary folder (`%APPDATA%\monspell\data`) cannot be created and `Speller::new` fails. The container's own profile is no place for it either: Windows wipes it whenever the host exits, and a host that starts while the previous one is still being cleaned up cannot create folders there (seen as a provider that stayed dead for that host's whole life). So `mso/src/engine.rs::build` falls back to `Speller::from_memory`: same files, no data directory, no custom dictionary, no FST. A failed load is also no longer final on either side of the bridge; the next provider creation retries (the engine at most once every 5 s). The provider's error log, when written from the host, lands in `%LOCALAPPDATA%\Packages\microsoft^windows^spellchecking^host\AC\monspell\windivvun.log` and disappears with that profile: read it while the host is still running. Words in monspell's own custom dictionary are not visible to the host; Windows' own user dictionary reaches the provider through `InitializeWordlist` and works.
  - `install-monspell.iss`: 0.1.1-beta. Ships dict-mn with its README and license (LPPL clause 2), stops shipping `mn.bhfst` and deletes it on upgrade, takes the 32-bit `monspell_mso.dll` from `mso\target\i686-pc-windows-msvc\release`.
  - `Spellers/mn.bhfst` stays in git only as the regression fixture for `speller_repository::box_v1_archive_is_refused`.
- **Why not upstream (fork-local part):** the bridge exists only because of monspell-core; upstream has no second engine.
- **Known limits:** a client that calls `CreateSpellChecker` in the half second while Windows is replacing an exiting host with a new one can get a one-off `0x80070020`, `0x8000FFFF` or an empty language list from Windows itself; the next call works. An allocation failure inside divvunspell still aborts (stable Rust cannot catch it); the version check removes the one known trigger. The CSAPI exports of `monspell_mso.dll` (`Speller*`) are not panic-contained yet. Sentence position is not passed to the engine, so the proper-noun policy (`check_word_at`) is not used on this path either.
- **Revisit trigger:** upstream moves to a current toolchain (then link monspell-core directly and drop the bridge); divvunspell/box-format is bumped past 0.3.2 (then `BOX_VERSION` must follow); a second consumer needs the bridge ABI (then move it out of the CSAPI DLL).
- **Build (Windows, VS Build Tools + Windows SDK; `<core>` = a checkout of `abadrangui/monspell`):**
  1. Provider, both bitnesses, pinned toolchain (directory override `nightly-2023-03-15-i686-pc-windows-msvc`; `midl.exe` needs the VS environment):
     `vcvarsall.bat x86 && cargo build --release` → `target\release\windivvun.dll`
     `vcvarsall.bat x64 && cargo build --release --target x86_64-pc-windows-msvc`
  2. Engine DLL, both bitnesses, stable toolchain, from `mso\`:
     `cargo +stable build --release --target x86_64-pc-windows-msvc --target i686-pc-windows-msvc --config "patch.'https://github.com/abadrangui/monspell'.monspell-core.path='<core>/crates/monspell-core'"`
  3. Stage data into `Spellers\` (all git-ignored): `mn_MN.aff`, `mn_MN.dic`, `README_mn_MN.txt` from `abadrangui/dict-mn` `mn_MN\`, its `LICENSE` as `LICENSE-dict-mn.txt`, and `<core>/corpus/derived/lawcorpus-freq.tsv` as `freq.tsv`.
  4. `ISCC.exe install-monspell.iss` → `dist\monspell-0.1.1-beta-setup.exe`.
  5. Tests: `cargo test --target x86_64-pc-windows-msvc` here; in `mso\`, `cargo +stable test` with the same `--config` and `MONSPELL_ENGINE_DIR` pointing at the staged `Spellers\`.
- **Links:** `mso/tests/contract.rs::wsc_bridge_contract`; monspell `docs/phases/phase-1-done.md` § 4.

## 2026-11-25 — WSCAPI mn checker rejects Latin words (Edge English typos); installer 0.1.2-beta

- **What happened:** in Edge (Chromium uses the Windows spell checker on Windows) with Mongolian and English both enabled, English typos were never underlined. Chromium marks a word only when EVERY enabled language rejects it. The monspell tokenizer only makes Cyrillic tokens, so the mn checker accepted every Latin word, and `teh` passed. Windows' own en-US speller does the opposite: it rejects every Cyrillic word (action GET_SUGGESTIONS, no suggestions), which is why Mongolian typos still showed. Seen on ABA08, Edge 158, screenshots in the 2026-11-25 audit (`C:\Users\Badrangui\monspell-audit\edge-shot-1.png` vs `edge-shot-2-en-only.png`).
- **What changed (fork-local):**
  - `src/engine.rs` only, i.e. `windivvun.dll`, the WSCAPI path. For the monspell engine, `tokens()` adds Latin words (`latin_tokens`) to the engine's Cyrillic tokens. A Latin word is a maximal run of A-Z/a-z with optional inner `'`/`’` that does not touch any letter or digit (so `teh`, `don't`, `Excel` in `Excel-д` count; `5G`, `COVID19`, `café`, a Latin letter glued to Cyrillic do not). `is_correct()` rejects it and `suggest()` returns an empty list, both without calling the engine. 5 unit tests.
  - Not changed: `mso/` (Word's CSAPI path, the shared `next_token`, the bridge ABI, still version 1). `monspell_mso.dll` is not rebuilt.
  - `install-monspell.iss`: 0.1.2-beta. `GetMonspellCount` overflowed: `StrToInt('yymmddhhnn')` is above the signed 32-bit limit since 2021, so `Count` was written as -1 on every install and Office copied the `User Settings\monspell\Create` tree into HKCU only the first time. Now the last 9 digits (`ymmddhhnn`) are used.
- **Why not upstream:** upstream serves one language per archive through divvunspell; the mixed-script problem comes from monspell's Cyrillic-only tokenizer.
- **Known limits:** with ONLY Mongolian enabled in a multi-language client, English words now show as errors (accepted trade-off, same as en-US-only shows Mongolian as errors). In a client that checks per language run (RichEdit, Notepad), Latin text tagged Mongolian is now flagged; the Mongolian keyboard cannot type Latin, so this needs pasted or re-tagged text. URL and e-mail pieces are Latin words too.
- **Revisit trigger:** a client that asks one speller per text run with mixed scripts complains; or the engine learns to tokenize Latin itself (then drop `latin_tokens`).
- **Build:** as in the previous entry, steps 1, 3, 4 (step 2 only if `mso/` changes). Output `dist\monspell-0.1.2-beta-setup.exe`.
