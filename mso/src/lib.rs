//! monspell-mso — Office Custom Proofing Tool (CSAPI / MSEI) "say-yes" stub.
//!
//! Microsoft Word 2016+ will not dispatch WSCAPI for a language unless there
//! is also a registered CSAPI proofing tool for it. Microsoft never ships a
//! CSAPI proofing tool for Mongolian, so the gate stays locked even when
//! our real spellchecker (windivvun.dll / WSCAPI) is registered correctly.
//!
//! This DLL is the minimum-viable CSAPI surface. Every exported function
//! returns success with zero results. Word loads it, probes its exports,
//! accepts it as valid, and unlocks the language in the "Set Proofing
//! Language" dialog. Actual spell-check squiggles then come from WSCAPI.
//!
//! Export list and `__stdcall` argument counts were reverse-engineered from
//! `divvunspellmso.dll` v0.4.0 (from `pahkat.uit.no/tools/download/windivvun`,
//! dumpbin /EXPORTS + /DISASM, May 2022 build) and cross-checked against
//! Microsoft StyleCop's `SpellChecker.cs` (MS-PL). Signatures are not
//! copyrightable; the behaviour here is original.
//!
//! References for future maintainers:
//!   - `SpellChecker.cs` in github.com/StyleCop/StyleCop — authoritative
//!     struct layouts, options enums, magic version `0x03000000`.
//!   - `Microsoft-3D-Movie-Maker/kauai/SRC/CSAPI.H` (MIT) — legacy Office 95
//!     ANSI header; shows the command-code / error-code lineage.
//!   - KB 262605 confirms the modern CSAPI spec is NDA-gated and has never
//!     been published, so the above two public sources are the ceiling.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

mod engine;

use core::ffi::c_void;
use core::ptr;
use monspell_core::CheckResult;

// ---- Win32 basic types ----
type UINT = u32;
type BOOL = i32;
type WORD = u16;
type LPCWSTR = *const u16;
type LPWSTR = *mut u16;

// ---- CSAPI types ----
//
// PTEC = "Proofing Tool Error Code". 0 = success; non-zero = categorised error.
// We only ever return 0 — the plumbing is a cooperative contract with Word.
type PTEC = u32;

// CSAPI version handshake. Office 2000+ (and therefore every target we care
// about) negotiates at 0x03000000. Older Office 97 was 0x02000000.
const CSAPI_VERSION_OFFICE_2000_PLUS: UINT = 0x0300_0000;

// Opaque tokens we hand out and Word passes back. We do not dereference
// them; they just need to be non-null so Word does not interpret them as
// error sentinels.
const STUB_CONTEXT: *mut c_void = 1 as *mut c_void;
const STUB_LEX: *mut c_void = 2 as *mut c_void;

// ---- Structs (layouts per StyleCop's SpellChecker.cs) ----

#[repr(C)]
pub struct PROOFPARAMS {
    pub VersionApi: UINT,
}

#[repr(C)]
pub struct PROOFLEXIN {
    pub pwszLex: LPCWSTR,
    pub create: BOOL,
    pub lxt: UINT,
    pub lidExpected: WORD,
}

#[repr(C)]
pub struct PROOFLEXOUT {
    pub pwszCopyright: LPCWSTR,
    pub lex: *mut c_void,
    pub CchCopyright: UINT,
    pub version: UINT,
    pub readOnly: BOOL,
    pub lid: WORD,
}

#[repr(C)]
pub struct WSIB {
    pub pwsz: LPCWSTR,
    pub prglex: *mut *mut c_void,
    pub cch: usize,
    pub clex: usize,
    pub sstate: UINT,
    pub ichStart: UINT,
    pub cchUse: usize,
}

/// Layout per StyleCop's `SpellerSuggestion`: a pointer into the CALLER's
/// `WSRB.pwsz` buffer plus the (offset, length) of the same span, and a
/// rating (higher = better).
#[repr(C)]
pub struct SPELLERSUGGESTION {
    pub pwsz: LPCWSTR,
    pub ichSugg: UINT,
    pub cchSugg: UINT,
    pub rating: UINT,
}

#[repr(C)]
pub struct WSRB {
    pub pwsz: LPWSTR,
    pub prgsugg: *mut SPELLERSUGGESTION,
    pub ichError: UINT,
    pub cchError: UINT,
    pub ichProcess: UINT,
    pub cchProcess: UINT,
    pub sstat: i32,
    pub csz: UINT,
    pub cszAlloc: UINT,
    pub cchMac: UINT,
    pub cchAlloc: UINT,
}

// SpellerCommand values (StyleCop SpellChecker.cs).
const SCMD_VERIFY_BUFFER: UINT = 2;
const SCMD_SUGGEST: UINT = 3;
const SCMD_SUGGEST_MORE: UINT = 4;
const SCMD_VERIFY_BUFFER_AUTOREPLACE: UINT = 10;

// SpellerStatus values (StyleCop SpellChecker.cs).
const SSTAT_NO_ERRORS: i32 = 0;
const SSTAT_UNKNOWN_INPUT_WORD: i32 = 1;
const SSTAT_NO_MORE_SUGGESTIONS: i32 = 8;

// ---- Exports ----
//
// `extern "system"` = stdcall on i686, plain x64 ABI on x86_64. That matches
// Divvun's reverse-engineered calling convention; arg counts were confirmed
// via `dumpbin /DISASM` on divvunspellmso.dll's `ret NN` epilogues.
//
// `#[no_mangle]` + the `.def` file in monspell_mso.def suppress both Rust
// name mangling AND MSVC `_Name@NN` stdcall decoration. Resulting exports
// are plain (e.g. `SpellerInit`), matching Divvun.

/// `ret 4` → 1 arg. Takes an out-ptr for the version; we write our supported
/// API level. Safe no-op if the caller passed null.
#[no_mangle]
pub unsafe extern "system" fn SpellerVersion(pVersion: *mut UINT) -> PTEC {
    if !pVersion.is_null() {
        *pVersion = CSAPI_VERSION_OFFICE_2000_PLUS;
    }
    0
}

/// `ret 8` → 2 args: `(out pid, in params)`.
/// StyleCop signature: `SpellerInit(void** pid, PROOFPARAMS* p)`.
/// We hand back a non-null opaque handle so Word's subsequent calls have
/// something to pass as `id`.
#[no_mangle]
pub unsafe extern "system" fn SpellerInit(
    pid: *mut *mut c_void,
    _params: *mut PROOFPARAMS,
) -> PTEC {
    if !pid.is_null() {
        *pid = STUB_CONTEXT;
    }
    0
}

/// `ret 8` → 2 args: `(id, fForce)`. Cleanup; we have nothing to free.
#[no_mangle]
pub extern "system" fn SpellerTerminate(_id: *mut c_void, _fForce: BOOL) -> PTEC {
    0
}

/// `ret 0Ch` → 3 args. StyleCop: `(id, optSelect, optVal)`.
#[no_mangle]
pub extern "system" fn SpellerSetOptions(
    _id: *mut c_void,
    _optSelect: UINT,
    _optVal: UINT,
) -> PTEC {
    0
}

/// `ret 0Ch` → 3 args. Mirror of SetOptions: `(id, optSelect, outOptVal*)`.
/// Write 0 to the out-param if non-null; Word interprets "0" as "default".
#[no_mangle]
pub unsafe extern "system" fn SpellerGetOptions(
    _id: *mut c_void,
    _optSelect: UINT,
    pOptVal: *mut UINT,
) -> PTEC {
    if !pOptVal.is_null() {
        *pOptVal = 0;
    }
    0
}

/// `ret 0Ch` → 3 args. StyleCop: `(id, lexIn, lexOut)`.
/// Populate the out-struct with a non-null opaque lex handle + advertise
/// our API version so Word's compat check passes.
#[no_mangle]
pub unsafe extern "system" fn SpellerOpenLex(
    _id: *mut c_void,
    _lexIn: *const PROOFLEXIN,
    lexOut: *mut PROOFLEXOUT,
) -> PTEC {
    if !lexOut.is_null() {
        (*lexOut).pwszCopyright = ptr::null();
        (*lexOut).lex = STUB_LEX;
        (*lexOut).CchCopyright = 0;
        (*lexOut).version = CSAPI_VERSION_OFFICE_2000_PLUS;
        (*lexOut).readOnly = 1;
        (*lexOut).lid = 0x0450; // mn-MN; Word overrides as needed.
    }
    0
}

/// `ret 0Ch` → 3 args: `(id, lex, fForce)`.
#[no_mangle]
pub extern "system" fn SpellerCloseLex(
    _id: *mut c_void,
    _lex: *mut c_void,
    _fForce: BOOL,
) -> PTEC {
    0
}

/// `ret 10h` → 4 args. StyleCop: `(id, scmd, wsib_in, wsrb_out)`.
///
/// **This is the function that decides whether Word renders squiggles from
/// CSAPI.** For a CSAPI-gated language, Word uses CSAPI exclusively — no
/// WSCAPI fallthrough. Phase 1: real per-word checking + suggestions via
/// monspell-core (dict-mn Hunspell + optional lang-khk FST).
///
/// Output-buffer contract (StyleCop SpellChecker.cs): the CALLER owns
/// `wsrb.pwsz` (capacity `cchAlloc` UTF-16 units) and `wsrb.prgsugg`
/// (capacity `cszAlloc` entries). We only ever write inside those bounds —
/// the Phase 0 crash came from declaring results without valid buffers.
/// If the engine is unavailable, everything reports "no errors" (degrade
/// silently rather than crash or false-flag).
#[no_mangle]
pub unsafe extern "system" fn SpellerCheck(
    _id: *mut c_void,
    scmd: UINT,
    wsib: *const WSIB,
    wsrb: *mut WSRB,
) -> PTEC {
    if wsib.is_null() || wsrb.is_null() {
        return 0;
    }
    let wsib = &*wsib;
    let out = &mut *wsrb;

    // Slice of the input actually under check: pwsz[ichStart .. ichStart+cchUse].
    let (units, ich_start): (&[u16], u32) = if wsib.pwsz.is_null() || wsib.cchUse == 0 {
        (&[], 0)
    } else {
        let start = wsib.ichStart as usize;
        (
            core::slice::from_raw_parts(wsib.pwsz.add(start), wsib.cchUse),
            wsib.ichStart,
        )
    };

    // Defaults: clean result over the whole input.
    out.ichError = 0;
    out.cchError = 0;
    out.ichProcess = ich_start;
    out.cchProcess = units.len() as UINT;
    out.sstat = SSTAT_NO_ERRORS;
    out.csz = 0;
    out.cchMac = 0;

    match scmd {
        SCMD_VERIFY_BUFFER | SCMD_VERIFY_BUFFER_AUTOREPLACE => {
            let hit = engine::with_engine(|speller| {
                engine::tokens_utf16(units).into_iter().find(|t| {
                    matches!(speller.check_word(&t.text), CheckResult::Misspelled { .. })
                })
            })
            .flatten();
            if let Some(token) = hit {
                out.ichError = ich_start + token.start;
                out.cchError = token.len;
                // Report progress up to the end of the flagged word so the
                // caller resumes after it and later errors surface too.
                out.cchProcess = token.start + token.len;
                out.sstat = SSTAT_UNKNOWN_INPUT_WORD;
            }
        }
        SCMD_SUGGEST => {
            // Word passes the misspelled word as the buffer; suggestions are
            // written into the caller's pwsz arena, described by prgsugg.
            let word = String::from_utf16_lossy(units);
            let word = word.trim();
            let suggestions = engine::with_engine(|speller| speller.suggest(word))
                .unwrap_or_default();
            out.sstat = SSTAT_UNKNOWN_INPUT_WORD;
            if suggestions.is_empty() || out.pwsz.is_null() || out.prgsugg.is_null() {
                out.sstat = SSTAT_NO_MORE_SUGGESTIONS;
                return 0;
            }
            let arena_cap = out.cchAlloc as usize;
            let sugg_cap = out.cszAlloc as usize;
            let mut used = 0usize;
            let mut count = 0usize;
            for (i, s) in suggestions.iter().enumerate() {
                if count >= sugg_cap {
                    break;
                }
                let utf16: Vec<u16> = s.encode_utf16().collect();
                // +1 for a terminating NUL between entries.
                if used + utf16.len() + 1 > arena_cap {
                    break;
                }
                let dst = out.pwsz.add(used);
                core::ptr::copy_nonoverlapping(utf16.as_ptr(), dst, utf16.len());
                *dst.add(utf16.len()) = 0;
                let entry = out.prgsugg.add(count);
                (*entry).pwsz = dst;
                (*entry).ichSugg = used as UINT;
                (*entry).cchSugg = utf16.len() as UINT;
                (*entry).rating = 255u32.saturating_sub(i as u32);
                used += utf16.len() + 1;
                count += 1;
            }
            out.csz = count as UINT;
            out.cchMac = used as UINT;
        }
        SCMD_SUGGEST_MORE => {
            out.sstat = SSTAT_NO_MORE_SUGGESTIONS;
        }
        _ => {}
    }
    0
}

/// `ret 0Ch` → 3 args. StyleCop: `(id, lex, word)`. Custom-dictionary add.
#[no_mangle]
pub extern "system" fn SpellerAddUdr(
    _id: *mut c_void,
    _lex: *mut c_void,
    _word: LPCWSTR,
) -> PTEC {
    0
}

/// `ret 10h` → 4 args. Not in StyleCop; best-fit guess: `(id, lex, from, to)`
/// — "auto-correct this word to that word" style entry.
#[no_mangle]
pub extern "system" fn SpellerAddChangeUdr(
    _id: *mut c_void,
    _lex: *mut c_void,
    _from: LPCWSTR,
    _to: LPCWSTR,
) -> PTEC {
    0
}

/// `ret 0Ch` → 3 args. StyleCop: `(id, lex, word)`. Custom-dictionary delete.
#[no_mangle]
pub extern "system" fn SpellerDelUdr(
    _id: *mut c_void,
    _lex: *mut c_void,
    _word: LPCWSTR,
) -> PTEC {
    0
}

/// `ret 8` → 2 args. StyleCop: `(id, lex)`. Wipe the custom dictionary.
#[no_mangle]
pub extern "system" fn SpellerClearUdr(_id: *mut c_void, _lex: *mut c_void) -> PTEC {
    0
}

/// `ret 0Ch` → 3 args. Not in StyleCop; likely `(id, lex, outSize*)` — number
/// of custom-dictionary entries.
#[no_mangle]
pub unsafe extern "system" fn SpellerGetSizeUdr(
    _id: *mut c_void,
    _lex: *mut c_void,
    pSize: *mut UINT,
) -> PTEC {
    if !pSize.is_null() {
        *pSize = 0;
    }
    0
}

/// `ret 10h` → 4 args. Not in StyleCop; likely enumerates UDR entries —
/// `(id, lex, outBuf, inOutCch)` or similar. We report the buffer was
/// "populated" with zero entries.
#[no_mangle]
pub unsafe extern "system" fn SpellerGetListUdr(
    _id: *mut c_void,
    _lex: *mut c_void,
    _arg3: UINT,
    pCch: *mut UINT,
) -> PTEC {
    if !pCch.is_null() {
        *pCch = 0;
    }
    0
}

/// `ret 8` → 2 args. StyleCop: `(id, lxt)`. Returns the built-in UDR handle
/// for a given user-dictionary type. We return a stub handle.
#[no_mangle]
pub extern "system" fn SpellerBuiltinUdr(
    _id: *mut c_void,
    _lxt: UINT,
) -> *mut c_void {
    STUB_LEX
}

/// Standard COM DLL lifecycle. `0` (S_OK) = "safe to unload now".
#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> i32 {
    0
}
