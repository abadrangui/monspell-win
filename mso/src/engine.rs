//! Engine bootstrap for the CSAPI shim: locate dictionary data, build one
//! process-wide monspell_core::Speller, hand out synchronized access.

use monspell_core::{Speller, SpellerConfig};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static ENGINE: OnceLock<Mutex<Speller>> = OnceLock::new();
/// When the last load attempt failed, and why. Also the lock that makes
/// loading single-file.
static LAST_FAILURE: Mutex<Option<(Instant, String)>> = Mutex::new(None);
/// A failed load is retried, but not on every call: each attempt parses an
/// 18 MB dictionary.
const RETRY_AFTER: Duration = Duration::from_secs(5);

/// Engine data directory, in priority order:
/// 1. `MONSPELL_ENGINE_DIR` (tests, debugging)
/// 2. `<dir of this DLL>\Spellers\` (installer layout, mirrors windivvun)
/// 3. `%APPDATA%\monspell\engine\`
fn engine_dir() -> Option<PathBuf> {
    let has_dict = |p: &PathBuf| p.join("mn_MN.aff").exists() && p.join("mn_MN.dic").exists();
    if let Ok(dir) = std::env::var("MONSPELL_ENGINE_DIR") {
        let dir = PathBuf::from(dir);
        if has_dict(&dir) {
            return Some(dir);
        }
    }
    if let Some(dll) = dll_dir() {
        // The installer puts DLLs in {app}\i686\ / {app}\x86_64\ and data in
        // {app}\Spellers\ — check both the sibling and the parent's Spellers.
        let dir = dll.join("Spellers");
        if has_dict(&dir) {
            return Some(dir);
        }
        if let Some(parent) = dll.parent() {
            let dir = parent.join("Spellers");
            if has_dict(&dir) {
                return Some(dir);
            }
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = PathBuf::from(appdata).join("monspell").join("engine");
        if has_dict(&dir) {
            return Some(dir);
        }
    }
    None
}

#[cfg(windows)]
fn dll_dir() -> Option<PathBuf> {
    // Resolve the on-disk location of THIS module (the DLL), not the host exe.
    const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 0x0000_0004;
    const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: u32 = 0x0000_0002;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleExW(
            flags: u32,
            addr: *const core::ffi::c_void,
            out: *mut *mut core::ffi::c_void,
        ) -> i32;
        fn GetModuleFileNameW(
            module: *mut core::ffi::c_void,
            filename: *mut u16,
            size: u32,
        ) -> u32;
    }
    unsafe {
        let mut module = core::ptr::null_mut();
        let ok = GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            dll_dir as *const core::ffi::c_void,
            &mut module,
        );
        if ok == 0 {
            return None;
        }
        let mut buf = [0u16; 32768];
        let len = GetModuleFileNameW(module, buf.as_mut_ptr(), buf.len() as u32) as usize;
        if len == 0 || len >= buf.len() {
            return None;
        }
        let path = PathBuf::from(String::from_utf16_lossy(&buf[..len]));
        path.parent().map(|p| p.to_path_buf())
    }
}

#[cfg(not(windows))]
fn dll_dir() -> Option<PathBuf> {
    None
}

/// Run `f` against the shared engine. Returns None when the engine is
/// unavailable (no dictionary found / failed to load) — callers degrade to
/// the say-yes stub behaviour, never crash Word.
pub fn with_engine<R>(f: impl FnOnce(&Speller) -> R) -> Option<R> {
    // A poisoned lock only means an earlier caller panicked mid-query. `f`
    // gets `&Speller`, so there is no half-written state to protect; keep
    // serving instead of going silent for the rest of the process.
    engine().map(|mutex| {
        f(&mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()))
    })
}

/// Why the engine is unavailable, or `None` when it is loaded. Tries to load
/// it first if that is due.
pub fn load_error() -> Option<String> {
    if engine().is_some() {
        return None;
    }
    let failure = LAST_FAILURE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    failure.as_ref().map(|(_, reason)| reason.clone())
}

/// The loaded engine. A failed load is not final: a host process can live as
/// long as some app keeps a spell checker open, and one bad moment at its
/// start must not cost that whole lifetime.
fn engine() -> Option<&'static Mutex<Speller>> {
    if let Some(engine) = ENGINE.get() {
        return Some(engine);
    }
    let mut failure = LAST_FAILURE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(engine) = ENGINE.get() {
        return Some(engine);
    }
    if matches!(failure.as_ref(), Some((when, _)) if when.elapsed() < RETRY_AFTER) {
        return None;
    }
    match build() {
        Ok(speller) => {
            *failure = None;
            Some(ENGINE.get_or_init(|| Mutex::new(speller)))
        }
        Err(reason) => {
            *failure = Some((Instant::now(), reason));
            None
        }
    }
}

/// Build the speller from the data in the engine directory.
///
/// monspell-core keeps the user's custom dictionary under the roaming
/// profile and refuses to start when it cannot create that folder. A
/// sandboxed host cannot: `msspellcheckinghost.exe` runs in an AppContainer
/// with no access to the user's profile (and the container's own profile is
/// wiped whenever the host exits, so nothing can live there either). When
/// the normal constructor fails, build the engine from the same files with
/// no data directory at all. That path has no custom dictionary and no FST;
/// it costs one extra dictionary parse (~0.15 s) per sandboxed process.
fn build() -> Result<Speller, String> {
    let dir = engine_dir()
        .ok_or_else(|| "mn_MN.aff + mn_MN.dic not found in any engine directory".to_string())?;
    let aff = dir.join("mn_MN.aff");
    let dic = dir.join("mn_MN.dic");
    let freq = Some(dir.join("freq.tsv")).filter(|path| path.exists());

    let mut config = SpellerConfig::new(&aff, &dic);
    let fst = dir.join("mn.bhfst");
    if fst.exists() {
        config.fst_archive = Some(fst);
    }
    config.frequency_list = freq.clone();
    let max_suggestions = config.max_suggestions;

    let first_error = match Speller::new(config) {
        Ok(speller) => return Ok(speller),
        Err(error) => error.to_string(),
    };

    let read = |path: &Path| {
        std::fs::read_to_string(path).map_err(|error| {
            format!(
                "{first_error}; then failed to read {}: {error}",
                path.display()
            )
        })
    };
    let aff = read(&aff)?;
    let dic = read(&dic)?;
    let freq = freq.as_deref().map(read).transpose()?;
    Speller::from_memory(&aff, &dic, freq.as_deref(), None, max_suggestions)
        .map_err(|error| format!("{first_error}; then without a data directory: {error}"))
}

/// A token worth checking: contains Cyrillic and no digit contamination.
pub struct Token {
    /// Offset in UTF-16 units, relative to the checked slice.
    pub start: u32,
    /// Length in UTF-16 units.
    pub len: u32,
    pub text: String,
}

/// Tokenize a UTF-16 buffer into candidate words: maximal runs of Cyrillic
/// letters and hyphens. Runs adjacent to ASCII digits (5-ны, 10-р), and
/// hyphen-led runs hanging off a Latin word (Excel-д, PDF-ийг), are suffix
/// fragments, not words — dropped. Offsets are UTF-16 units so they map 1:1
/// onto CSAPI's ich*/cch* fields.
pub fn tokens_utf16(units: &[u16]) -> Vec<Token> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some((token, next)) = next_token(units, from) {
        out.push(token);
        from = next;
    }
    out
}

/// The first token at or after `from`, plus the offset to resume scanning
/// from (the end of the token's run, which can lie past the token itself
/// when trailing hyphens were trimmed). `from` must be 0 or a resume offset
/// from a previous call, so it never lands inside a run.
pub fn next_token(units: &[u16], from: usize) -> Option<(Token, usize)> {
    let is_cyr = |u: u16| (0x0400..=0x04FF).contains(&u);
    let is_word = |u: u16| is_cyr(u) || u == b'-' as u16;
    let is_digit = |u: u16| (0x30..=0x39).contains(&u);
    let is_latin = |u: u16| (0x41..=0x5A).contains(&u) || (0x61..=0x7A).contains(&u);

    let mut i = from;
    while i < units.len() {
        if !is_word(units[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < units.len() && is_word(units[i]) {
            i += 1;
        }
        let digit_before = start > 0 && is_digit(units[start - 1]);
        let digit_after = i < units.len() && is_digit(units[i]);
        // Only a hyphen-led run counts: Cyrillic glued straight onto Latin
        // (a Latin `c` typed for `с`) is a real error and stays checked.
        let latin_suffix = start > 0 && units[start] == b'-' as u16 && is_latin(units[start - 1]);
        // trim leading/trailing hyphens off the token
        let mut s = start;
        let mut e = i;
        while s < e && units[s] == b'-' as u16 {
            s += 1;
        }
        while e > s && units[e - 1] == b'-' as u16 {
            e -= 1;
        }
        if s < e
            && !digit_before
            && !digit_after
            && !latin_suffix
            && units[s..e].iter().any(|&u| is_cyr(u))
        {
            let token = Token {
                start: s as u32,
                len: (e - s) as u32,
                text: String::from_utf16_lossy(&units[s..e]),
            };
            return Some((token, i));
        }
    }
    None
}
