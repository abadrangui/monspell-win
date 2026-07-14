//! In-process CSAPI contract tests: exercise SpellerCheck exactly the way
//! Word does (caller-allocated WSIB/WSRB buffers), without needing Word.
//! Engine data: MONSPELL_ENGINE_DIR, or ../../dict-mn/mn_MN relative to this
//! crate (the layout used by monspell's CI and the dev machines).

use monspell_mso::{SPELLERSUGGESTION, SpellerCheck, WSIB, WSRB};

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Stage a temp engine dir mirroring the installer layout: dict-mn files
/// plus a freq.tsv (suggestion ranking is part of the shipped behaviour).
fn ensure_engine_dir() {
    if std::env::var("MONSPELL_ENGINE_DIR").is_ok() {
        return;
    }
    let dict = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dict-mn/mn_MN");
    assert!(
        dict.join("mn_MN.aff").exists(),
        "dict-mn not found at {dict:?}; set MONSPELL_ENGINE_DIR"
    );
    let staged = std::env::temp_dir().join("monspell-mso-contract-engine");
    std::fs::create_dir_all(&staged).expect("temp engine dir");
    for f in ["mn_MN.aff", "mn_MN.dic"] {
        let dst = staged.join(f);
        if !dst.exists() {
            std::fs::copy(dict.join(f), &dst).expect("stage dict file");
        }
    }
    std::fs::write(staged.join("freq.tsv"), "60812\tхуулийн\n27939\tхууль\n")
        .expect("stage freq");
    // SAFETY: set before any engine access; tests in this binary run in one process.
    unsafe { std::env::set_var("MONSPELL_ENGINE_DIR", &staged) };
}

fn check_buffer(text: &[u16], ich_start: u32, cch_use: usize) -> WSRB {
    let mut lex: [*mut core::ffi::c_void; 0] = [];
    let wsib = WSIB {
        pwsz: text.as_ptr(),
        prglex: lex.as_mut_ptr(),
        cch: text.len(),
        clex: 0,
        sstate: 0,
        ichStart: ich_start,
        cchUse: cch_use,
    };
    let mut wsrb = WSRB {
        pwsz: core::ptr::null_mut(),
        prgsugg: core::ptr::null_mut(),
        ichError: 0,
        cchError: 0,
        ichProcess: 0,
        cchProcess: 0,
        sstat: -1,
        csz: 0,
        cszAlloc: 0,
        cchMac: 0,
        cchAlloc: 0,
    };
    let ptec = unsafe { SpellerCheck(core::ptr::null_mut(), 2, &wsib, &mut wsrb) };
    assert_eq!(ptec, 0, "SpellerCheck must return success PTEC");
    wsrb
}

#[test]
fn csapi_contract() {
    ensure_engine_dir();

    // 1. Clean formal text → NoErrors, whole buffer processed.
    let clean = utf16("Монгол хэлний тухай хуулийн зорилт");
    let out = check_buffer(&clean, 0, clean.len());
    assert_eq!(out.sstat, 0, "clean text must report NoErrors");
    assert_eq!(out.cchProcess as usize, clean.len());
    assert_eq!(out.cchError, 0);

    // 2. Typo mid-buffer → UnknownInputWord with the exact word span.
    let text = "Монгол хэлний тухай хуулын зорилт";
    let buf = utf16(text);
    let out = check_buffer(&buf, 0, buf.len());
    assert_eq!(out.sstat, 1, "хуулын must be flagged");
    let expected_start = text
        .split(' ')
        .take_while(|w| *w != "хуулын")
        .map(|w| w.encode_utf16().count() + 1)
        .sum::<usize>() as u32;
    assert_eq!(out.ichError, expected_start, "error offset must point at хуулын");
    assert_eq!(out.cchError as usize, "хуулын".encode_utf16().count());
    // progress ends after the flagged word so the caller can resume
    assert_eq!(out.cchProcess, out.ichError + out.cchError);

    // 3. ichStart is honoured: same text embedded after a prefix.
    let prefixed = utf16("xx хуулын");
    let out = check_buffer(&prefixed, 3, prefixed.len() - 3);
    assert_eq!(out.sstat, 1);
    assert_eq!(out.ichError, 3, "ichError is absolute (ichStart + relative)");

    // 4. Digit-glued suffix fragments are not words: "5-ны" must not flag.
    let digits = utf16("5-ны хуулийн");
    let out = check_buffer(&digits, 0, digits.len());
    assert_eq!(out.sstat, 0, "digit-adjacent fragment must be skipped");

    // 5. Suggest: caller-allocated arena + suggestion array get filled.
    let word = utf16("хуулын");
    let mut lex: [*mut core::ffi::c_void; 0] = [];
    let wsib = WSIB {
        pwsz: word.as_ptr(),
        prglex: lex.as_mut_ptr(),
        cch: word.len(),
        clex: 0,
        sstate: 0,
        ichStart: 0,
        cchUse: word.len(),
    };
    let mut arena = [0u16; 256];
    let mut suggs: [SPELLERSUGGESTION; 5] = core::array::from_fn(|_| SPELLERSUGGESTION {
        pwsz: core::ptr::null(),
        ichSugg: 0,
        cchSugg: 0,
        rating: 0,
    });
    let mut wsrb = WSRB {
        pwsz: arena.as_mut_ptr(),
        prgsugg: suggs.as_mut_ptr(),
        ichError: 0,
        cchError: 0,
        ichProcess: 0,
        cchProcess: 0,
        sstat: -1,
        csz: 0,
        cszAlloc: suggs.len() as u32,
        cchMac: 0,
        cchAlloc: arena.len() as u32,
    };
    let ptec = unsafe { SpellerCheck(core::ptr::null_mut(), 3, &wsib, &mut wsrb) };
    assert_eq!(ptec, 0);
    assert!(wsrb.csz >= 1, "must return at least one suggestion");
    assert!(wsrb.csz <= wsrb.cszAlloc);
    assert!(wsrb.cchMac <= wsrb.cchAlloc);
    let first = &suggs[0];
    let text = String::from_utf16_lossy(unsafe {
        core::slice::from_raw_parts(first.pwsz, first.cchSugg as usize)
    });
    assert_eq!(text, "хуулийн", "top suggestion must be the правильный form");
    assert_eq!(first.ichSugg, 0);
    assert!(first.rating > 0);

    // 6. Tiny arena: bounds respected, no overrun, graceful truncation.
    let mut small = [0xABABu16; 4];
    let mut wsrb_small = WSRB {
        pwsz: small.as_mut_ptr(),
        prgsugg: suggs.as_mut_ptr(),
        ichError: 0,
        cchError: 0,
        ichProcess: 0,
        cchProcess: 0,
        sstat: -1,
        csz: 0,
        cszAlloc: suggs.len() as u32,
        cchMac: 0,
        cchAlloc: small.len() as u32,
    };
    let ptec = unsafe { SpellerCheck(core::ptr::null_mut(), 3, &wsib, &mut wsrb_small) };
    assert_eq!(ptec, 0);
    assert_eq!(wsrb_small.csz, 0, "nothing fits in 4 units (хуулийн needs 8)");
    assert_eq!(wsrb_small.cchMac, 0);
    assert!(small.iter().all(|&u| u == 0xABAB), "arena must be untouched");
}
