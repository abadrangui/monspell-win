#![cfg(windows)]
#![allow(non_snake_case)]

use winapi::ctypes::c_void;
use winapi::shared::guiddef::{IsEqualGUID, GUID};
use winapi::shared::ntdef::ULONG;
use winapi::shared::winerror::{E_INVALIDARG, E_OUTOFMEMORY, E_POINTER, S_FALSE, S_OK};
use winapi::shared::wtypesbase::LPOLESTR;
use winapi::um::combaseapi::CoTaskMemFree;
use winapi::um::objidlbase::IEnumString;
use winapi::um::winnt::{HRESULT, LPCWSTR};

use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::spellcheckprovider::{IEnumSpellingError, ISpellCheckProvider, ISpellCheckProviderVtbl};

use com_impl::{implementation, interface, ComInterface};

use divvunspell::archive;

use std::collections::HashMap;

use crate::engine::Engine;
use crate::guard;
use crate::monspell_bridge;
use crate::speller_cache::SpellerCache;
use crate::util;
use crate::wordlists::Wordlists;
use crate::SPELLER_REPOSITORY;

use super::EnumSpellingError::DivvunEnumSpellingError;
use super::EnumString::EnumString;

ENUM! {enum WORDLIST_TYPE {
  WORDLIST_TYPE_IGNORE = 0,
  WORDLIST_TYPE_ADD = 1,
  WORDLIST_TYPE_EXCLUDE = 2,
  WORDLIST_TYPE_AUTOCORRECT = 3,
}}

#[interface(ISpellCheckProvider)]
pub struct DivvunSpellCheckProvider {
    refs: AtomicU32,
    language_tag: String,
    engine: Arc<Engine>,
    wordlists: Arc<Wordlists>,
}

IMPL_UNKNOWN!(ISpellCheckProvider, DivvunSpellCheckProvider);

/// Hand `text` to the caller through a COM `[out]` string parameter.
fn com_str_out(value: *mut LPCWSTR, text: &str) -> HRESULT {
    if value.is_null() {
        return E_POINTER;
    }
    let allocated = unsafe { util::alloc_com_str(text) };
    unsafe {
        *value = allocated.unwrap_or(std::ptr::null_mut());
    }
    match allocated {
        Some(_) => S_OK,
        None => E_OUTOFMEMORY,
    }
}

#[implementation(ISpellCheckProvider)]
impl DivvunSpellCheckProvider {
    fn get_LanguageTag(&mut self, value: *mut LPCWSTR) -> HRESULT {
        guard::hresult("ISpellCheckProvider::get_LanguageTag", || {
            info!("get_LanguageTag");
            com_str_out(value, &self.language_tag)
        })
    }

    fn Check(&mut self, text: LPCWSTR, value: *mut *mut IEnumSpellingError) -> HRESULT {
        guard::hresult("ISpellCheckProvider::Check", || {
            if text.is_null() || value.is_null() {
                return E_POINTER;
            }
            // Keep the caller's UTF-16 as it is: error positions are reported
            // in its units, and text that is not valid UTF-16 still gets its
            // valid words checked.
            let text = unsafe { util::u16_ptr_to_slice(text) };

            info!("Check {}", String::from_utf16_lossy(text));

            let enum_err = DivvunEnumSpellingError::new(
                self.engine.to_owned(),
                self.wordlists.to_owned(),
                text,
            );
            unsafe {
                *value = enum_err as *mut _;
            }

            S_OK
        })
    }

    fn Suggest(&mut self, word: LPCWSTR, value: *mut *mut IEnumString) -> HRESULT {
        guard::hresult("ISpellCheckProvider::Suggest", || {
            if value.is_null() {
                return E_POINTER;
            }
            unsafe {
                *value = std::ptr::null_mut();
            }
            let word = com_wstr_ptr!(word);

            info!("Suggest {}", word);

            let mut suggestions: Vec<String> = vec![];
            let mut result: Option<HRESULT> = None;

            // Check ignore wordlist
            if self.wordlists.contains_ignore(&word) {
                info!("wordlist ignore");
                suggestions.push(word.clone());
                result = Some(S_FALSE);
            }

            // Check auto correct wordlist. TODO: should the speller's suggestions be appended to this perhaps?
            if result.is_none() {
                if let Some(replacement) = self.wordlists.get_replacement(&word) {
                    info!("wordlist replacement {}", replacement);
                    suggestions.push(replacement);
                    result = Some(S_OK);
                }
            }

            // Check add wordlist
            if result.is_none() && self.wordlists.contains_add(&word) {
                info!("wordlist add");
                suggestions.push(word.clone());
                result = Some(S_OK);
            }

            // Check speller result
            let result = match result {
                Some(result) => result,
                None => {
                    // `true` once the speller has given its final list for a
                    // misspelled word, even if that list is empty.
                    let mut answered = false;
                    if !self.engine.is_correct(&word) {
                        if let Some(speller_suggestions) = self.engine.suggest(&word) {
                            suggestions = speller_suggestions;
                            answered = true;
                            info!(
                                "speller {} suggestions: {:?}",
                                suggestions.len(),
                                suggestions
                            );
                        } else {
                            // No results available yet but word is incorrect
                            info!("no speller suggestions yet");
                        }
                    }

                    // No results, word is correct if no excludes in wordlist
                    if suggestions.is_empty()
                        && !answered
                        && !self.wordlists.contains_exclude(&word)
                    {
                        info!("wordlist exclude");
                        suggestions.push(word.clone());
                        S_FALSE
                    } else {
                        S_OK
                    }
                }
            };

            let enum_if = EnumString::new(suggestions);
            unsafe {
                *value = enum_if as *mut _;
            }

            result
        })
    }

    fn GetOptionValue(&mut self, optionId: LPCWSTR, value: *mut u8) -> HRESULT {
        guard::hresult("ISpellCheckProvider::GetOptionValue", || {
            info!("GetOptionValue");
            // nope
            E_INVALIDARG
        })
    }

    fn SetOptionValue(&mut self, optionId: LPCWSTR, value: u8) -> HRESULT {
        guard::hresult("ISpellCheckProvider::SetOptionValue", || {
            info!("SetOptionValue");
            // nope
            E_INVALIDARG
        })
    }

    fn get_OptionIds(&mut self, value: *mut *mut IEnumString) -> HRESULT {
        guard::hresult("ISpellCheckProvider::get_OptionIds", || {
            if value.is_null() {
                return E_POINTER;
            }
            info!("get_OptionIds");
            let enum_if = EnumString::new(vec![]);
            unsafe {
                *value = enum_if as *mut _;
            }
            S_OK
        })
    }

    fn get_Id(&mut self, value: *mut LPCWSTR) -> HRESULT {
        guard::hresult("ISpellCheckProvider::get_Id", || {
            info!("get_Id");
            // divvun or so
            com_str_out(value, "windivvun")
        })
    }

    fn get_LocalizedName(&mut self, value: *mut LPCWSTR) -> HRESULT {
        guard::hresult("ISpellCheckProvider::get_LocalizedName", || {
            info!("get_LocalizedName");
            // Divvun Spell Thing
            com_str_out(value, "WinDivvun")
        })
    }

    fn GetOptionDescription(&mut self, optionId: LPCWSTR, value: *mut *mut c_void) -> HRESULT {
        guard::hresult("ISpellCheckProvider::GetOptionDescription", || {
            info!("GetOptionDescription");
            // nope
            E_INVALIDARG
        })
    }

    fn InitializeWordlist(
        &mut self,
        wordlistType: WORDLIST_TYPE,
        words: *const IEnumString,
    ) -> HRESULT {
        guard::hresult("ISpellCheckProvider::InitializeWordlist", || {
            info!("InitializeWordlist {}", wordlistType);
            if words.is_null() {
                return E_POINTER;
            }
            let elem_count: u32 = 50;
            let mut words_vec: Vec<String> = vec![];
            let mut vec: Vec<LPOLESTR> = vec![std::ptr::null_mut(); elem_count as usize];
            loop {
                let mut fetched: ULONG = 0;
                let res = unsafe { (*words).Next(elem_count, vec.as_mut_ptr(), &mut fetched) };
                if res == S_OK || res == S_FALSE {
                    // `take` bounds a miscounting enumerator to our buffer.
                    for item in vec.iter().take(fetched as usize) {
                        if item.is_null() {
                            continue;
                        }
                        let word = unsafe { util::u16_ptr_to_string(*item) };
                        unsafe {
                            CoTaskMemFree(*item as *mut c_void);
                        }
                        // An entry that is not valid UTF-16 can never equal a
                        // checked word; drop it instead of failing the list.
                        if let Ok(word) = word.into_string() {
                            words_vec.push(word);
                        }
                    }
                }

                if res != S_OK {
                    break;
                }
            }

            match wordlistType {
                WORDLIST_TYPE_ADD => self.wordlists.set_add(words_vec),
                WORDLIST_TYPE_EXCLUDE => self.wordlists.set_exclude(words_vec),
                WORDLIST_TYPE_AUTOCORRECT => {
                    let mut map: HashMap<String, String> = HashMap::new();
                    for word in words_vec {
                        let tokens = word.split('\t').collect::<Vec<&str>>();
                        if tokens.len() == 2 {
                            map.insert(tokens[0].to_string(), tokens[1].to_string());
                        } else {
                            error!("Invalid auto correct pair: {:?}", tokens);
                        }
                    }
                    self.wordlists.set_auto_correct(map)
                }
                WORDLIST_TYPE_IGNORE => self.wordlists.set_ignore(words_vec),
                _ => info!("invalid wordlist type {}", wordlistType),
            };

            S_OK
        })
    }
}

impl DivvunSpellCheckProvider {
    pub fn new(language_tag: &str) -> Option<*mut DivvunSpellCheckProvider> {
        let engine = if SPELLER_REPOSITORY.is_monspell_language(language_tag) {
            info!("Instantiating monspell engine for {}", language_tag);
            Engine::Monspell(monspell_bridge::bridge()?)
        } else {
            let archive_path = SPELLER_REPOSITORY.get_speller_archive(language_tag)?;
            // let archive_path = archive_path.to_str()?;
            // let archive_path = archive_path.to_string();

            info!(
                "Instantiating speller {} at {}",
                language_tag,
                archive_path.display()
            );

            let archive = match archive::open(&archive_path) {
                Ok(v) => v,
                Err(err) => {
                    guard::report(&format!(
                        "failed to load speller archive {}: {:?}",
                        archive_path.display(),
                        err
                    ));
                    return None;
                }
            };

            Engine::Divvun {
                cache: SpellerCache::new(archive.speller()),
                _archive: archive,
            }
        };

        let s = Self {
            __vtable: Box::new(Self::create_vtable()),
            refs: AtomicU32::new(1),
            language_tag: language_tag.to_string(),
            engine: Arc::new(engine),
            wordlists: Wordlists::new(),
        };

        let ptr = Box::into_raw(Box::new(s));

        Some(ptr as *mut _)
    }
}
