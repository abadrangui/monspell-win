//! Engine bridge for the WSCAPI provider (`windivvun.dll`).
//!
//! `windivvun.dll` is pinned to a 2023 toolchain by upstream's dependency
//! tree and cannot link monspell-core (edition 2024), so it loads this DLL
//! at runtime and calls the exports below. Word and every WSCAPI app
//! therefore get verdicts from the same engine, data and tokenizer.
//!
//! Contract:
//! - C types only, UTF-16 in and out, caller-allocated buffers. No memory
//!   crosses the boundary, so the two Rust runtimes never free each other's
//!   allocations.
//! - Every export catches panics and reports failure through its return
//!   value. The caller runs inside `msspellcheckinghost.exe`, which serves
//!   spell checking for every app on the machine; it must never go down.

use crate::engine;
use core::slice;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Bumped on any incompatible change to the exports below. `windivvun.dll`
/// refuses a bridge whose version it does not know.
pub const WSC_ABI_VERSION: u32 = 1;

fn guarded<R>(fallback: R, f: impl FnOnce() -> R) -> R {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}

#[no_mangle]
pub extern "system" fn MonspellWscAbiVersion() -> u32 {
    WSC_ABI_VERSION
}

/// Load the engine if it is not loaded yet (first call reads the 600k-entry
/// dictionary). Returns 1 = ready, 0 = unavailable. A failure is not final:
/// a later call tries again, at most once every few seconds.
#[no_mangle]
pub extern "system" fn MonspellWscLoad() -> i32 {
    guarded(0, || i32::from(engine::with_engine(|_| ()).is_some()))
}

/// Why `MonspellWscLoad` returned 0, as UTF-16 text in the caller's
/// `buf[..cap]` (truncated to fit, not NUL-terminated). Returns the units
/// written; 0 when the engine is loaded or on bad arguments.
#[no_mangle]
pub unsafe extern "system" fn MonspellWscLoadError(buf: *mut u16, cap: usize) -> usize {
    guarded(0, || {
        if buf.is_null() {
            return 0;
        }
        let Some(message) = engine::load_error() else {
            return 0;
        };
        let mut written = 0usize;
        for unit in message.encode_utf16().take(cap) {
            *buf.add(written) = unit;
            written += 1;
        }
        written
    })
}

/// Find the first word at or after `from` in `text[..len]`. On a hit returns
/// 1 and writes the word's span (`start`, `token_len`) and the offset to pass
/// as `from` next time (`next`), all in UTF-16 units. Returns 0 when there
/// are no more words, -1 on bad arguments.
#[no_mangle]
pub unsafe extern "system" fn MonspellWscNextToken(
    text: *const u16,
    len: usize,
    from: usize,
    start: *mut usize,
    token_len: *mut usize,
    next: *mut usize,
) -> i32 {
    guarded(-1, || {
        if text.is_null() || start.is_null() || token_len.is_null() || next.is_null() {
            return -1;
        }
        match engine::next_token(slice::from_raw_parts(text, len), from) {
            Some((token, resume)) => {
                *start = token.start as usize;
                *token_len = token.len as usize;
                *next = resume;
                1
            }
            None => 0,
        }
    })
}

/// Recognition only, no suggestions: 1 = correct, 0 = misspelled,
/// -1 = engine unavailable or bad arguments (callers must not flag).
#[no_mangle]
pub unsafe extern "system" fn MonspellWscIsCorrect(word: *const u16, len: usize) -> i32 {
    guarded(-1, || {
        if word.is_null() {
            return -1;
        }
        let word = String::from_utf16_lossy(slice::from_raw_parts(word, len));
        match engine::with_engine(|speller| speller.is_correct(&word)) {
            Some(correct) => i32::from(correct),
            None => -1,
        }
    })
}

/// Suggestions for `word`, best first, written into the caller's `buf[..cap]`
/// as consecutive NUL-terminated UTF-16 strings; `used` receives the units
/// written. Suggestions that do not fit are dropped. Returns the number
/// written (0 = none), or -1 when the engine is unavailable or on bad
/// arguments.
#[no_mangle]
pub unsafe extern "system" fn MonspellWscSuggest(
    word: *const u16,
    len: usize,
    buf: *mut u16,
    cap: usize,
    used: *mut usize,
) -> i32 {
    guarded(-1, || {
        if word.is_null() || buf.is_null() || used.is_null() {
            return -1;
        }
        *used = 0;
        let word = String::from_utf16_lossy(slice::from_raw_parts(word, len));
        let Some(suggestions) = engine::with_engine(|speller| speller.suggest(word.trim())) else {
            return -1;
        };
        let mut written = 0usize;
        let mut count = 0i32;
        for suggestion in &suggestions {
            let utf16: Vec<u16> = suggestion.encode_utf16().collect();
            if written + utf16.len() + 1 > cap {
                break;
            }
            let dst = buf.add(written);
            core::ptr::copy_nonoverlapping(utf16.as_ptr(), dst, utf16.len());
            *dst.add(utf16.len()) = 0;
            written += utf16.len() + 1;
            count += 1;
        }
        *used = written;
        count
    })
}
