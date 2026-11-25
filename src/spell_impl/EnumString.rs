#![cfg(windows)]
#![allow(non_snake_case)]

use winapi::um::winnt::HRESULT;

use winapi::shared::guiddef::{IsEqualGUID, GUID};
use winapi::shared::ntdef::ULONG;
use winapi::shared::winerror::{E_OUTOFMEMORY, E_POINTER, E_UNEXPECTED, S_FALSE, S_OK};
use winapi::um::combaseapi::CoTaskMemFree;

use winapi::shared::wtypesbase::LPOLESTR;
use winapi::um::objidlbase::{IEnumString, IEnumStringVtbl};
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};

use std::sync::atomic::{AtomicU32, Ordering};

use com_impl::{implementation, interface, ComInterface};

use crate::guard;
use crate::util;
use std::vec::Vec;

#[interface(IEnumString)]
pub struct EnumString {
    refs: AtomicU32,
    values: Vec<String>,
    offset: usize,
}

IMPL_UNKNOWN!(IEnumString, EnumString);

#[implementation(IEnumString)]
impl EnumString {
    fn Next(&mut self, celt: ULONG, rgelt: *mut LPOLESTR, pceltFetched: *mut ULONG) -> HRESULT {
        guard::hresult("IEnumString::Next", || {
            if rgelt.is_null() {
                return E_POINTER;
            }
            if !pceltFetched.is_null() {
                unsafe {
                    *pceltFetched = 0;
                }
            }
            let celt = celt as usize;
            info!("Next for {} values", celt);

            let values = self
                .values
                .iter()
                .skip(self.offset)
                .take(celt)
                .collect::<Vec<&String>>();

            info!("{} values fetched", values.len());

            if values.is_empty() {
                return S_FALSE;
            }

            if pceltFetched.is_null() && celt > 1 {
                return E_UNEXPECTED;
            }

            unsafe {
                for (i, item) in values.iter().enumerate() {
                    match util::alloc_com_str(item) {
                        Some(elem_str) => *rgelt.add(i) = elem_str,
                        None => {
                            // Hand back nothing rather than a half-filled array.
                            for j in 0..i {
                                CoTaskMemFree(*rgelt.add(j) as *mut _);
                                *rgelt.add(j) = std::ptr::null_mut();
                            }
                            return E_OUTOFMEMORY;
                        }
                    }
                }

                if !pceltFetched.is_null() {
                    *pceltFetched = values.len() as u32;
                }
            }

            self.offset += values.len();

            if values.len() == celt {
                S_OK
            } else {
                S_FALSE
            }
        })
    }

    fn Skip(&mut self, celt: ULONG) -> HRESULT {
        guard::hresult("IEnumString::Skip", || {
            info!("skip {}", celt);
            self.offset = self.offset.saturating_add(celt as usize);
            S_OK
        })
    }

    fn Reset(&mut self) -> HRESULT {
        guard::hresult("IEnumString::Reset", || {
            info!("reset");
            self.offset = 0;
            S_OK
        })
    }

    fn Clone(&mut self, ppenum: *mut *mut IEnumString) -> HRESULT {
        guard::hresult("IEnumString::Clone", || {
            if ppenum.is_null() {
                return E_POINTER;
            }
            info!("clone");
            unsafe {
                let val = EnumString::new_with_offset(self.values.clone(), self.offset);
                *ppenum = val as *mut _;
            }
            S_OK
        })
    }
}

impl EnumString {
    pub fn new(values: Vec<String>) -> *mut EnumString {
        Self::new_with_offset(values, 0)
    }

    pub fn new_with_offset(values: Vec<String>, offset: usize) -> *mut EnumString {
        info!("Create enum string {} values", values.len());
        let s = Self {
            __vtable: Box::new(Self::create_vtable()),
            refs: AtomicU32::new(1),
            values,
            offset,
        };

        let ptr = Box::into_raw(Box::new(s));

        ptr as *mut _
    }
}
