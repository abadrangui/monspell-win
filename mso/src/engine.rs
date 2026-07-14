//! Engine bootstrap for the CSAPI shim: locate dictionary data, build one
//! process-wide monspell_core::Speller, hand out synchronized access.

use monspell_core::{Speller, SpellerConfig};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static ENGINE: OnceLock<Option<Mutex<Speller>>> = OnceLock::new();

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
    let slot = ENGINE.get_or_init(|| {
        let dir = engine_dir()?;
        let mut config = SpellerConfig::new(dir.join("mn_MN.aff"), dir.join("mn_MN.dic"));
        let fst = dir.join("mn.bhfst");
        if fst.exists() {
            config.fst_archive = Some(fst);
        }
        let freq = dir.join("freq.tsv");
        if freq.exists() {
            config.frequency_list = Some(freq);
        }
        Speller::new(config).ok().map(Mutex::new)
    });
    match slot {
        Some(mutex) => match mutex.lock() {
            Ok(guard) => Some(f(&guard)),
            Err(_) => None,
        },
        None => None,
    }
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
/// letters and hyphens. Runs adjacent to ASCII digits (5-ны, 10-р) are
/// suffix fragments, not words — dropped. Offsets are UTF-16 units so they
/// map 1:1 onto CSAPI's ich*/cch* fields.
pub fn tokens_utf16(units: &[u16]) -> Vec<Token> {
    let is_cyr = |u: u16| (0x0400..=0x04FF).contains(&u);
    let is_word = |u: u16| is_cyr(u) || u == b'-' as u16;
    let is_digit = |u: u16| (0x30..=0x39).contains(&u);

    let mut out = Vec::new();
    let mut i = 0usize;
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
        // trim leading/trailing hyphens off the token
        let mut s = start;
        let mut e = i;
        while s < e && units[s] == b'-' as u16 {
            s += 1;
        }
        while e > s && units[e - 1] == b'-' as u16 {
            e -= 1;
        }
        if s < e && !digit_before && !digit_after && units[s..e].iter().any(|&u| is_cyr(u)) {
            out.push(Token {
                start: s as u32,
                len: (e - s) as u32,
                text: String::from_utf16_lossy(&units[s..e]),
            });
        }
    }
    out
}
