#![cfg(windows)]
#![allow(non_snake_case)]

use winapi::shared::guiddef::{IsEqualGUID, GUID};
use winapi::shared::ntdef::ULONG;
use winapi::shared::winerror::{E_OUTOFMEMORY, E_POINTER, S_FALSE, S_OK};
use winapi::um::winnt::{HRESULT, LPCWSTR};

use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use crate::spellcheckprovider::{
    IEnumSpellingError, IEnumSpellingErrorVtbl, ISpellingError, ISpellingErrorVtbl,
    CORRECTIVE_ACTION_GET_SUGGESTIONS, CORRECTIVE_ACTION_REPLACE,
};

use com_impl::{implementation, interface, ComInterface};

use crate::engine::{Engine, Token};
use crate::guard;
use crate::util;
use crate::wordlists::Wordlists;
use std::sync::Arc;

#[interface(ISpellingError)]
pub struct DivvunSpellingError {
    refs: AtomicU32,
    start_index: u32,
    length: u32,
    corrective_action: u32,
    replacement: Option<String>,
}

IMPL_UNKNOWN!(ISpellingError, DivvunSpellingError);

#[implementation(ISpellingError)]
impl DivvunSpellingError {
    fn get_StartIndex(&mut self, value: *mut u32) -> HRESULT {
        guard::hresult("ISpellingError::get_StartIndex", || {
            if value.is_null() {
                return E_POINTER;
            }
            // info!("StartIndex, {:?}", self.start_index);
            unsafe {
                *value = self.start_index;
            }
            S_OK
        })
    }

    fn get_Length(&mut self, value: *mut u32) -> HRESULT {
        guard::hresult("ISpellingError::get_Length", || {
            if value.is_null() {
                return E_POINTER;
            }
            // info!("Length, {:?}", self.length);
            unsafe {
                *value = self.length;
            }
            S_OK
        })
    }

    fn get_CorrectiveAction(&mut self, value: *mut u32) -> HRESULT {
        guard::hresult("ISpellingError::get_CorrectiveAction", || {
            if value.is_null() {
                return E_POINTER;
            }
            // info!("CorrectiveAction, {:?}", self.corrective_action);
            unsafe {
                *value = self.corrective_action;
            }
            S_OK
        })
    }

    fn get_Replacement(&mut self, value: *mut LPCWSTR) -> HRESULT {
        guard::hresult("ISpellingError::get_Replacement", || {
            if value.is_null() {
                return E_POINTER;
            }
            unsafe {
                *value = std::ptr::null_mut();
            }
            if self.corrective_action != CORRECTIVE_ACTION_REPLACE {
                // info!("Replacement, null");
                return S_OK;
            }
            // The caller frees an [out] string with CoTaskMemFree, so it has
            // to be a COM allocation, never a pointer into this object.
            let replacement = self.replacement.as_deref().unwrap_or("");
            match unsafe { util::alloc_com_str(replacement) } {
                Some(text) => {
                    unsafe {
                        *value = text;
                    }
                    S_OK
                }
                None => E_OUTOFMEMORY,
            }
        })
    }
}

impl DivvunSpellingError {
    pub fn new(
        start_index: u32,
        length: u32,
        corrective_action: u32,
        replacement: Option<String>,
    ) -> *mut DivvunSpellingError {
        let s = Self {
            __vtable: Box::new(Self::create_vtable()),
            refs: AtomicU32::new(1),

            start_index,
            length,
            corrective_action,

            replacement,
        };

        let ptr = Box::into_raw(Box::new(s));

        ptr as *mut _
    }
}

#[interface(IEnumSpellingError)]
pub struct DivvunEnumSpellingError {
    refs: AtomicU32,
    engine: Arc<Engine>,
    /// Every word of the checked text, with UTF-16 offsets.
    tokens: Vec<Token>,
    next_token: AtomicUsize,
    wordlists: Arc<Wordlists>,
}

IMPL_UNKNOWN!(IEnumSpellingError, DivvunEnumSpellingError);

#[implementation(IEnumSpellingError)]
impl DivvunEnumSpellingError {
    fn Next(&mut self, value: *mut *mut ISpellingError) -> HRESULT {
        guard::hresult("IEnumSpellingError::Next", || {
            if value.is_null() {
                return E_POINTER;
            }
            unsafe {
                *value = std::ptr::null_mut();
            }
            info!("Next");

            loop {
                let index = self.next_token.fetch_add(1, Ordering::SeqCst);
                let token = match self.tokens.get(index) {
                    Some(token) => token,
                    None => return S_FALSE,
                };
                let word = token.text.as_str();
                info!("Token {}: {:?}", token.start, word);

                // Check ignore wordlist
                if self.wordlists.contains_ignore(word) {
                    info!("Wordlist ignore");
                    continue;
                }

                let mut action: Option<u32> = None;
                let mut replacement: Option<String> = None;

                // Check auto correct wordlist
                if let Some(r) = self.wordlists.get_replacement(word) {
                    info!("wordlist replace {}", r);

                    action = Some(CORRECTIVE_ACTION_REPLACE);
                    replacement = Some(r);
                }

                // Check exclude wordlist
                if action.is_none() && self.wordlists.contains_exclude(word) {
                    info!("wordlist incorrect");
                    action = Some(CORRECTIVE_ACTION_GET_SUGGESTIONS);
                    replacement = None;
                }

                // Check add wordlist
                if self.wordlists.contains_add(word) {
                    info!("wordlist added");
                    action = None;
                } else {
                    // Query speller API
                    if action.is_none() && !self.engine.is_correct(word) {
                        action = Some(CORRECTIVE_ACTION_GET_SUGGESTIONS);
                        replacement = None;
                    }
                }

                let action = match action {
                    Some(action) => action,
                    None => continue,
                };

                info!(
                    "word {:?}, error action: {:?}, replacement {:?}",
                    word, action, replacement
                );

                self.engine.prime(word);

                info!("start {}, length {}", token.start, token.length);

                let error =
                    DivvunSpellingError::new(token.start, token.length, action, replacement);

                unsafe {
                    *value = error as *mut _;
                }
                return S_OK;
            }
        })
    }
}

impl DivvunEnumSpellingError {
    /// `text` is the checked text in UTF-16, as the caller passed it; error
    /// positions are reported in the same units.
    pub fn new(
        engine: Arc<Engine>,
        wordlists: Arc<Wordlists>,
        text: &[u16],
    ) -> *mut DivvunEnumSpellingError {
        let s = Self {
            __vtable: Box::new(Self::create_vtable()),
            refs: AtomicU32::new(1),
            tokens: engine.tokens(text),
            engine,
            next_token: AtomicUsize::new(0),
            wordlists,
        };

        let ptr = Box::into_raw(Box::new(s));

        ptr as *mut _
    }
}

// Upstream test is bitrotted: references `Token::Word(..)` and a
// `.tokenize()` method, but `divvunspell::tokenizer::Tokenize` no longer
// exposes either (it now provides `word_bound_indices`, `word_indices`,
// etc.). Gated off so `cargo test` can compile; see the monspell fork's
// docs/fork-divergence.md for the full record.
#[cfg(any())]
#[test]
fn tokens() {
    let res: Vec<Token> = "Hello world how are you doing"
        .tokenize()
        .filter_map(|t| {
            if let Token::Word(_, _, _) = t {
                Some(t)
            } else {
                None
            }
        })
        .collect();

    for r in res {
        println!("{:?}", r);
    }
}
