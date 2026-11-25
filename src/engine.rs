//! The speller behind one provider instance.

use std::sync::Arc;

use divvunspell::archive::SpellerArchive;
use divvunspell::tokenizer::Tokenize;

use crate::monspell_bridge::MonspellBridge;
use crate::speller_cache::SpellerCache;

pub enum Engine {
    /// A `.bhfst`/`.zhfst` archive through divvunspell (upstream's path).
    Divvun {
        cache: Arc<SpellerCache>,
        _archive: Arc<dyn SpellerArchive + Send + Sync>,
    },
    /// monspell-core + dict-mn through the bridge DLL.
    Monspell(&'static MonspellBridge),
}

/// One word of the checked text. Offsets are UTF-16 code units, which is
/// what `ISpellingError` reports to the caller.
pub struct Token {
    pub start: u32,
    pub length: u32,
    pub text: String,
}

impl Engine {
    pub fn tokens(&self, text: &[u16]) -> Vec<Token> {
        match self {
            Engine::Divvun { .. } => divvun_tokens(text),
            Engine::Monspell(bridge) => {
                // Cyrillic words come from the engine's tokenizer (shared with
                // Word); Latin words are added here, on the WSCAPI side only.
                let mut tokens: Vec<Token> = bridge
                    .tokens(text)
                    .into_iter()
                    .chain(latin_tokens(text))
                    .map(|(start, length)| Token {
                        start: start as u32,
                        length: length as u32,
                        text: String::from_utf16_lossy(&text[start..start + length]),
                    })
                    .collect();
                tokens.sort_by_key(|token| token.start);
                tokens
            }
        }
    }

    pub fn is_correct(&self, word: &str) -> bool {
        match self {
            Engine::Divvun { cache, .. } => cache.is_correct(word),
            // A Latin word is never Mongolian: rejected without asking the
            // engine, like Windows' own en-US speller rejects Cyrillic.
            Engine::Monspell(bridge) => !is_latin_word(word) && bridge.is_correct(word),
        }
    }

    /// Suggestions for a misspelled word. `None` means "not computed yet":
    /// the divvunspell path fills its cache on a background thread.
    pub fn suggest(&self, word: &str) -> Option<Vec<String>> {
        match self {
            Engine::Divvun { cache, .. } => cache.suggest_cache_only(word),
            Engine::Monspell(_) if is_latin_word(word) => Some(Vec::new()),
            Engine::Monspell(bridge) => Some(bridge.suggest(word)),
        }
    }

    /// Start computing suggestions for a word that was just reported.
    pub fn prime(&self, word: &str) {
        if let Engine::Divvun { cache, .. } = self {
            cache.prime(word);
        }
    }
}

// Latin words in the Mongolian (WSCAPI) checker.
//
// The engine only tokenizes Cyrillic, so a Latin word used to pass silently.
// Clients that run several spellers at once (Chromium/Edge: a word is marked
// only when EVERY enabled language rejects it) then never marked an English
// typo while Mongolian was on, because the mn speller "accepted" it. Windows'
// own en-US speller rejects every Cyrillic word; the mn checker now mirrors
// that and rejects every Latin word. Trade-off: with ONLY Mongolian enabled,
// English words show as errors. Word (CSAPI) is not affected: it never comes
// through this file.

fn is_ascii_letter(unit: u16) -> bool {
    (0x41..=0x5A).contains(&unit) || (0x61..=0x7A).contains(&unit)
}

/// `'` or `’`, allowed between two letters (`don't`, `O’Neil`).
fn is_apostrophe(unit: u16) -> bool {
    unit == 0x27 || unit == 0x2019
}

/// The character that ends just before `units[i]` (a surrogate pair counts
/// as one character; a lone surrogate is none).
fn char_before(units: &[u16], i: usize) -> Option<char> {
    if i == 0 {
        return None;
    }
    let low = units[i - 1];
    if (0xDC00..=0xDFFF).contains(&low) && i >= 2 && (0xD800..=0xDBFF).contains(&units[i - 2]) {
        return std::char::decode_utf16([units[i - 2], low].iter().copied())
            .next()
            .and_then(Result::ok);
    }
    std::char::from_u32(u32::from(low))
}

/// The character that starts at `units[i]`.
fn char_at(units: &[u16], i: usize) -> Option<char> {
    std::char::decode_utf16(units.get(i..)?.iter().copied().take(2))
        .next()
        .and_then(Result::ok)
}

/// Latin words in `units` as `(start, length)` in UTF-16 units. A Latin word
/// is a maximal run of A-Z/a-z, with optional inner apostrophes, that does
/// not touch a letter or digit of any kind (Cyrillic, accented Latin, 0-9):
/// `teh` and `don't` count; `5G`, `COVID19`, `café` and a Latin letter glued
/// to Cyrillic do not. `Excel` in `Excel-д` counts (the hyphen separates).
fn latin_tokens(units: &[u16]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < units.len() {
        if !is_ascii_letter(units[i]) {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i;
        loop {
            while end < units.len() && is_ascii_letter(units[end]) {
                end += 1;
            }
            if end + 1 < units.len() && is_apostrophe(units[end]) && is_ascii_letter(units[end + 1])
            {
                end += 1;
                continue;
            }
            break;
        }
        i = end;
        let touches = |c: Option<char>| c.map_or(false, char::is_alphanumeric);
        if !touches(char_before(units, start)) && !touches(char_at(units, end)) {
            out.push((start, end - start));
        }
    }
    out
}

/// `word` is exactly one Latin word (see `latin_tokens`).
fn is_latin_word(word: &str) -> bool {
    let units: Vec<u16> = word.encode_utf16().collect();
    !units.is_empty() && latin_tokens(&units) == [(0, units.len())]
}

fn divvun_tokens(text: &[u16]) -> Vec<Token> {
    // Lossy decoding maps an unpaired surrogate (one unit) to U+FFFD (one
    // unit), so UTF-16 offsets into `decoded` equal offsets into `text`.
    let decoded = String::from_utf16_lossy(text);
    let mut tokens = Vec::new();
    let mut byte_pos = 0usize;
    let mut unit_pos = 0usize;
    let words = decoded
        .word_bound_indices()
        .filter(|(_, word)| word.chars().any(|ch| ch.is_alphanumeric()));
    for (start, word) in words {
        unit_pos += decoded[byte_pos..start].encode_utf16().count();
        byte_pos = start;
        tokens.push(Token {
            start: unit_pos as u32,
            length: word.encode_utf16().count() as u32,
            text: word.to_string(),
        });
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(text: &str) -> Vec<(u32, u32, String)> {
        let units: Vec<u16> = text.encode_utf16().collect();
        divvun_tokens(&units)
            .into_iter()
            .map(|t| (t.start, t.length, t.text))
            .collect()
    }

    #[test]
    fn offsets_are_utf16_units_not_bytes() {
        // Cyrillic is two bytes per letter in UTF-8 but one UTF-16 unit.
        assert_eq!(
            spans("сайн байна уу"),
            vec![
                (0, 4, "сайн".to_string()),
                (5, 5, "байна".to_string()),
                (11, 2, "уу".to_string()),
            ]
        );
    }

    #[test]
    fn astral_characters_count_as_two_units() {
        // U+1F600 is four UTF-8 bytes and a surrogate pair in UTF-16.
        assert_eq!(spans("\u{1F600} сайн"), vec![(3, 4, "сайн".to_string())]);
    }

    fn latin(text: &str) -> Vec<String> {
        let units: Vec<u16> = text.encode_utf16().collect();
        latin_tokens(&units)
            .into_iter()
            .map(|(start, len)| String::from_utf16_lossy(&units[start..start + len]))
            .collect()
    }

    #[test]
    fn latin_words_next_to_mongolian_are_found() {
        let units: Vec<u16> = "хуулын teh word.".encode_utf16().collect();
        assert_eq!(latin_tokens(&units), vec![(7, 3), (11, 4)]);
    }

    #[test]
    fn latin_word_keeps_inner_apostrophes_only() {
        assert_eq!(latin("don't O\u{2019}Neil 'quoted' rock'n'roll"), vec!["don't", "O\u{2019}Neil", "quoted", "rock'n'roll"]);
    }

    #[test]
    fn latin_run_touching_letters_or_digits_is_skipped() {
        // Cyrillic glued on, digits, accented Latin, a math letter (surrogate pair).
        assert!(latin("abcд дabc 5G A4 COVID19 café \u{1D400}abc").is_empty());
    }

    #[test]
    fn latin_word_next_to_punctuation_or_emoji_counts() {
        assert_eq!(latin("Excel-д (PDF), \u{1F600}ok"), vec!["Excel", "PDF", "ok"]);
        let units: Vec<u16> = "\u{1F600}ok".encode_utf16().collect();
        assert_eq!(latin_tokens(&units), vec![(2, 2)]);
    }

    #[test]
    fn is_latin_word_needs_one_whole_latin_word() {
        assert!(is_latin_word("teh"));
        assert!(is_latin_word("don't"));
        assert!(!is_latin_word("хуулын"));
        assert!(!is_latin_word("Excel-д"));
        assert!(!is_latin_word("a1"));
        assert!(!is_latin_word("two words"));
        assert!(!is_latin_word(""));
    }

    #[test]
    fn unpaired_surrogate_keeps_offsets() {
        let units: Vec<u16> = vec![0xD800, 0x20, 0x0441, 0x0430, 0x0439, 0x043D];
        let tokens = divvun_tokens(&units);
        assert_eq!(tokens.len(), 1);
        assert_eq!((tokens[0].start, tokens[0].length), (2, 4));
    }
}
