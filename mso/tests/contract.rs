//! In-process CSAPI contract tests: exercise SpellerCheck exactly the way
//! Word does (caller-allocated WSIB/WSRB buffers), without needing Word.
//! Engine data: MONSPELL_ENGINE_DIR, or ../../dict-mn/mn_MN relative to this
//! crate (the layout used by monspell's CI and the dev machines).

use monspell_mso::wsc::{
    MonspellWscAbiVersion, MonspellWscIsCorrect, MonspellWscLoad, MonspellWscLoadError,
    MonspellWscNextToken, MonspellWscSuggest, WSC_ABI_VERSION,
};
use monspell_mso::{SPELLERSUGGESTION, SpellerCheck, WSIB, WSRB};

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Stage a temp engine dir mirroring the installer layout: dict-mn files
/// plus a freq.tsv (suggestion ranking is part of the shipped behaviour).
fn ensure_engine_dir() {
    // Tests run on parallel threads; stage once.
    static STAGE: std::sync::Once = std::sync::Once::new();
    STAGE.call_once(stage_engine_dir);
}

fn stage_engine_dir() {
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

    // 4b. So is a suffix hanging off a Latin word: "Excel-д" must not flag "д".
    let latin = utf16("Excel-д PDF-ийг хуулийн");
    let out = check_buffer(&latin, 0, latin.len());
    assert_eq!(out.sstat, 0, "suffix on a Latin word must be skipped");

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

/// The exports windivvun.dll (WSCAPI provider) calls, driven the way it
/// drives them: caller-owned UTF-16 buffers in, plain integers out.
#[test]
fn wsc_bridge_contract() {
    ensure_engine_dir();
    assert_eq!(MonspellWscAbiVersion(), WSC_ABI_VERSION);
    assert_eq!(MonspellWscLoad(), 1, "engine must load from the staged dir");
    let mut reason = [0u16; 64];
    let written = unsafe { MonspellWscLoadError(reason.as_mut_ptr(), reason.len()) };
    assert_eq!(written, 0, "a loaded engine has no load error to report");

    // 1. Tokens come back as UTF-16 spans, in order, resumable. Suffix
    //    fragments on digits and on Latin words are skipped exactly as on
    //    the CSAPI path; Cyrillic glued straight onto a Latin letter (Latin
    //    `c` typed for `с`) is still a token, so the typo gets checked.
    let text = utf16("5-ны өдөр Excel-д cайн хуулын төсөл.");
    let mut words = Vec::new();
    let mut from = 0usize;
    loop {
        let (mut start, mut len, mut next) = (0usize, 0usize, 0usize);
        let rc = unsafe {
            MonspellWscNextToken(
                text.as_ptr(),
                text.len(),
                from,
                &mut start,
                &mut len,
                &mut next,
            )
        };
        match rc {
            1 => {
                assert!(next >= start + len && next <= text.len());
                words.push(String::from_utf16_lossy(&text[start..start + len]));
                from = next;
            }
            0 => break,
            other => panic!("unexpected NextToken result {other}"),
        }
    }
    assert_eq!(words, ["өдөр", "айн", "хуулын", "төсөл"]);

    // 2. Recognition.
    let good = utf16("хуулийн");
    let bad = utf16("хуулын");
    assert_eq!(
        unsafe { MonspellWscIsCorrect(good.as_ptr(), good.len()) },
        1
    );
    assert_eq!(unsafe { MonspellWscIsCorrect(bad.as_ptr(), bad.len()) }, 0);

    // 3. Suggestions land in the caller's buffer, NUL-separated, best first.
    let mut buf = [0u16; 256];
    let mut used = 0usize;
    let count = unsafe {
        MonspellWscSuggest(
            bad.as_ptr(),
            bad.len(),
            buf.as_mut_ptr(),
            buf.len(),
            &mut used,
        )
    };
    assert!(count >= 1, "must return at least one suggestion");
    assert!(used <= buf.len());
    let listed: Vec<String> = buf[..used]
        .split(|&u| u == 0)
        .filter(|s| !s.is_empty())
        .map(String::from_utf16_lossy)
        .collect();
    assert_eq!(listed.len(), count as usize);
    assert_eq!(listed[0], "хуулийн");

    // 4. Too-small buffer: nothing fits, nothing is written.
    let mut small = [0xABABu16; 4];
    let mut used_small = usize::MAX;
    let count = unsafe {
        MonspellWscSuggest(
            bad.as_ptr(),
            bad.len(),
            small.as_mut_ptr(),
            small.len(),
            &mut used_small,
        )
    };
    assert_eq!((count, used_small), (0, 0));
    assert!(
        small.iter().all(|&u| u == 0xABAB),
        "buffer must be untouched"
    );

    // 5. Null pointers are an error code, never a crash.
    let mut used_null = 0usize;
    assert_eq!(unsafe { MonspellWscIsCorrect(core::ptr::null(), 3) }, -1);
    assert_eq!(
        unsafe {
            MonspellWscSuggest(
                core::ptr::null(),
                0,
                buf.as_mut_ptr(),
                buf.len(),
                &mut used_null,
            )
        },
        -1
    );
    assert_eq!(
        unsafe {
            MonspellWscNextToken(
                core::ptr::null(),
                0,
                0,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        },
        -1
    );
}
