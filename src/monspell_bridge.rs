//! Runtime link to the monspell engine (monspell-core + dict-mn).
//!
//! The engine needs a current Rust toolchain; this crate is held on a 2023
//! one by upstream's dependency tree. So the engine is not linked in: it
//! lives in the sibling `monspell_mso.dll`, loaded here by full path, and is
//! reached through the small C ABI that DLL exports for this purpose
//! (`mso/src/wsc.rs`). UTF-16 in and out, caller-allocated buffers, no memory
//! changing owner across the boundary.

use std::collections::HashMap;
use std::ffi::CString;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use parking_lot::{Mutex, RwLock};
use winapi::shared::minwindef::HMODULE;
use winapi::um::libloaderapi::{GetProcAddress, LoadLibraryExW, LOAD_WITH_ALTERED_SEARCH_PATH};

use crate::guard;
use crate::util;

/// Language stem served by the engine, as it appears in `HARDCODED_TAG_TABLE`.
pub const LANGUAGE: &str = "mn";
/// dict-mn files the engine reads; shipped as data, never compiled in.
const DICTIONARY_FILES: [&str; 2] = ["mn_MN.aff", "mn_MN.dic"];

const BRIDGE_DLL: &str = "monspell_mso.dll";
const ABI_VERSION: u32 = 1;
const LOAD_ERROR_UNITS: usize = 512;
const SUGGEST_BUFFER_UNITS: usize = 2048;
const SUGGEST_CACHE_ENTRIES: usize = 512;

type AbiVersionFn = unsafe extern "system" fn() -> u32;
type LoadFn = unsafe extern "system" fn() -> i32;
type LoadErrorFn = unsafe extern "system" fn(*mut u16, usize) -> usize;
type NextTokenFn =
    unsafe extern "system" fn(*const u16, usize, usize, *mut usize, *mut usize, *mut usize) -> i32;
type IsCorrectFn = unsafe extern "system" fn(*const u16, usize) -> i32;
type SuggestFn = unsafe extern "system" fn(*const u16, usize, *mut u16, usize, *mut usize) -> i32;

pub struct MonspellBridge {
    next_token: NextTokenFn,
    is_correct: IsCorrectFn,
    suggest: SuggestFn,
    /// A suggestion list costs the engine up to ~0.2 s and some clients ask
    /// again on every context menu, so keep the recent ones.
    suggestions: RwLock<HashMap<String, Vec<String>>>,
}

lazy_static! {
    static ref BRIDGE: RwLock<Option<&'static MonspellBridge>> = RwLock::new(None);
    static ref LAST_REPORTED: Mutex<String> = Mutex::new(String::new());
}

/// The process-wide engine, loaded on first use. `None` when the bridge DLL
/// or the dictionary is missing or refused to load; the reason is logged.
/// A failure is not final: this host process can outlive the cause (it stays
/// up as long as any app holds a spell checker), so the next call tries
/// again. The bridge itself spaces out the expensive part of a retry.
pub fn bridge() -> Option<&'static MonspellBridge> {
    if let Some(bridge) = *BRIDGE.read() {
        return Some(bridge);
    }
    let mut slot = BRIDGE.write();
    if let Some(bridge) = *slot {
        return Some(bridge);
    }
    match MonspellBridge::load() {
        Ok(bridge) => {
            let bridge: &'static MonspellBridge = Box::leak(Box::new(bridge));
            *slot = Some(bridge);
            Some(bridge)
        }
        Err(reason) => {
            // Retries repeat the same reason; log it once.
            let mut last = LAST_REPORTED.lock();
            if *last != reason {
                guard::report(&format!("monspell engine unavailable: {}", reason));
                *last = reason;
            }
            None
        }
    }
}

/// Whether the bridge DLL and the dictionary are installed where the bridge
/// looks for them (`Spellers\` beside this DLL or one level up). Cheap, no
/// dictionary load: this decides what the factory advertises.
pub fn is_installed() -> bool {
    let dir = match util::module_dir() {
        Some(dir) => dir,
        None => return false,
    };
    dir.join(BRIDGE_DLL).is_file()
        && (has_dictionary(&dir.join("Spellers"))
            || dir
                .parent()
                .map_or(false, |root| has_dictionary(&root.join("Spellers"))))
}

fn has_dictionary(dir: &Path) -> bool {
    DICTIONARY_FILES.iter().all(|name| dir.join(name).is_file())
}

unsafe fn export<T: Copy>(module: HMODULE, name: &str) -> Result<T, String> {
    debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<usize>());
    let symbol = CString::new(name).map_err(|_| format!("bad export name {}", name))?;
    let address = GetProcAddress(module, symbol.as_ptr());
    if address.is_null() {
        return Err(format!("{} does not export {}", BRIDGE_DLL, name));
    }
    Ok(std::mem::transmute_copy(&address))
}

impl MonspellBridge {
    fn load() -> Result<Self, String> {
        let path = util::module_dir()
            .ok_or_else(|| "cannot resolve this module's directory".to_string())?
            .join(BRIDGE_DLL);
        if !path.is_file() {
            return Err(format!("{} not found", path.display()));
        }
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // Full path plus LOAD_WITH_ALTERED_SEARCH_PATH: never resolved through
        // the host's DLL search order.
        let module = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_WITH_ALTERED_SEARCH_PATH,
            )
        };
        if module.is_null() {
            return Err(format!(
                "LoadLibraryExW({}) failed: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }

        // The module stays loaded for the life of the process, which keeps
        // these function pointers valid.
        let (abi_version, load, load_error, bridge) = unsafe {
            let abi_version: AbiVersionFn = export(module, "MonspellWscAbiVersion")?;
            let load: LoadFn = export(module, "MonspellWscLoad")?;
            let load_error: LoadErrorFn = export(module, "MonspellWscLoadError")?;
            let bridge = MonspellBridge {
                next_token: export(module, "MonspellWscNextToken")?,
                is_correct: export(module, "MonspellWscIsCorrect")?,
                suggest: export(module, "MonspellWscSuggest")?,
                suggestions: RwLock::new(HashMap::new()),
            };
            (abi_version, load, load_error, bridge)
        };

        let version = unsafe { abi_version() };
        if version != ABI_VERSION {
            return Err(format!(
                "{} speaks bridge ABI {}, this DLL speaks {}",
                BRIDGE_DLL, version, ABI_VERSION
            ));
        }
        if unsafe { load() } != 1 {
            let mut reason = vec![0u16; LOAD_ERROR_UNITS];
            let written = unsafe { load_error(reason.as_mut_ptr(), reason.len()) };
            return Err(format!(
                "{} could not load the engine: {}",
                BRIDGE_DLL,
                String::from_utf16_lossy(&reason[..written.min(reason.len())])
            ));
        }
        Ok(bridge)
    }

    /// Word spans in `text` as `(start, length)` in UTF-16 units.
    pub fn tokens(&self, text: &[u16]) -> Vec<(usize, usize)> {
        let mut spans = Vec::new();
        let mut from = 0usize;
        while from < text.len() {
            let (mut start, mut len, mut next) = (0usize, 0usize, 0usize);
            let found = unsafe {
                (self.next_token)(
                    text.as_ptr(),
                    text.len(),
                    from,
                    &mut start,
                    &mut len,
                    &mut next,
                )
            };
            // Trust nothing that came back across the boundary: a span outside
            // the text or a cursor that does not advance ends the scan.
            let end = start.checked_add(len);
            if found != 1 || len == 0 || end.map_or(true, |end| end > text.len()) || next <= from {
                break;
            }
            spans.push((start, len));
            from = next;
        }
        spans
    }

    /// `true` unless the engine positively says the word is misspelled; an
    /// engine failure must not turn into a squiggle.
    pub fn is_correct(&self, word: &str) -> bool {
        let wide: Vec<u16> = word.encode_utf16().collect();
        unsafe { (self.is_correct)(wide.as_ptr(), wide.len()) != 0 }
    }

    pub fn suggest(&self, word: &str) -> Vec<String> {
        if let Some(cached) = self.suggestions.read().get(word) {
            return cached.clone();
        }

        let wide: Vec<u16> = word.encode_utf16().collect();
        let mut buffer = vec![0u16; SUGGEST_BUFFER_UNITS];
        let mut used = 0usize;
        let count = unsafe {
            (self.suggest)(
                wide.as_ptr(),
                wide.len(),
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut used,
            )
        };
        if count < 0 {
            return Vec::new();
        }
        let result: Vec<String> = buffer[..used.min(buffer.len())]
            .split(|&unit| unit == 0)
            .filter(|chunk| !chunk.is_empty())
            .map(String::from_utf16_lossy)
            .collect();

        let mut cache = self.suggestions.write();
        if cache.len() >= SUGGEST_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(word.to_string(), result.clone());
        result
    }
}
