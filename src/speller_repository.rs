use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{fs, io};

use parking_lot::Mutex;

use crate::guard;
use crate::monspell_bridge;
use crate::util;

const BOX_MAGIC: [u8; 4] = [0xFF, b'B', b'O', b'X'];
/// The only BOX container version the pinned box-format 0.3.2 can read.
const BOX_VERSION: u32 = 0;
const BOX_HEADER_BYTES: u64 = 24;

lazy_static! {
    static ref REFUSED_ARCHIVES: Mutex<HashSet<PathBuf>> = Mutex::new(HashSet::new());
}

/// Whether divvunspell can be trusted with this archive.
///
/// box-format 0.3.2 never checks the container version. Handed a version 1
/// `.bhfst` (what current Divvun tooling writes) it reads the trailer with
/// the version 0 layout, takes a garbage record count and asks the allocator
/// for petabytes. A failed allocation is an abort, not an error or a panic,
/// so it cannot be contained and it takes the spell-checking host down.
/// Anything that is not a well-formed version 0 container is therefore
/// refused before the parser sees it.
pub fn is_readable_archive(path: &Path) -> bool {
    if path.extension().map_or(true, |ext| ext != "bhfst") {
        return true;
    }
    let problem = match read_box_header(path) {
        Ok((version, trailer, file_len)) => {
            if version != BOX_VERSION {
                format!(
                    "BOX container version {} (only {} is supported)",
                    version, BOX_VERSION
                )
            } else if trailer < BOX_HEADER_BYTES || trailer >= file_len {
                format!(
                    "trailer offset {} outside the {}-byte file",
                    trailer, file_len
                )
            } else {
                return true;
            }
        }
        Err(err) => err.to_string(),
    };
    if REFUSED_ARCHIVES.lock().insert(path.to_path_buf()) {
        guard::report(&format!(
            "ignoring speller archive {}: {}",
            path.display(),
            problem
        ));
    }
    false
}

/// `(version, trailer offset, file length)` from a BOX container header:
/// magic `FF 42 4F 58`, `u32` version, `u64` alignment, `u64` trailer offset,
/// all little-endian.
fn read_box_header(path: &Path) -> io::Result<(u32, u64, u64)> {
    let mut file = fs::File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut header = [0u8; BOX_HEADER_BYTES as usize];
    file.read_exact(&mut header)?;
    if header[0..4] != BOX_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a BOX container",
        ));
    }
    let mut version = [0u8; 4];
    version.copy_from_slice(&header[4..8]);
    let mut trailer = [0u8; 8];
    trailer.copy_from_slice(&header[16..24]);
    Ok((
        u32::from_le_bytes(version),
        u64::from_le_bytes(trailer),
        file_len,
    ))
}

lazy_static! {
    pub static ref HARDCODED_TAG_TABLE: HashMap<String, Vec<String>> = {
        let mut map = HashMap::new();
        for tag in &["se", "sma", "smn", "sms", "smj"] {
            let mut tags = vec![];
            for region in &["NO", "SE", "FI"] {
                tags.push(format!("{}-Latn-{}", tag, region));
                tags.push(format!("{}-{}", tag, region));
            }
            map.insert(tag.to_string(), tags);
        }
        // Mongolian (Khalkha, Cyrillic). mn-Mong-* is deferred.
        map.insert(
            "mn".to_string(),
            vec![
                "mn".to_string(),
                "mn-MN".to_string(),
                "mn-Cyrl".to_string(),
                "mn-Cyrl-MN".to_string(),
            ],
        );
        map
    };
}

fn resolve_local_name(neutral_tag: &str) -> Vec<String> {
    let mut tags: Vec<String> = vec![];

    if let Some(tag) = util::resolve_locale_name(&neutral_tag) {
        tags.push(tag);
    } else {
        tags.push(neutral_tag.to_string());
    }

    if let Some(extra_tags) = HARDCODED_TAG_TABLE.get(neutral_tag) {
        extra_tags.iter().for_each(|tag| tags.push(tag.to_owned()));
    }

    tags
}

pub struct SpellerRepository {
    base_directories: Vec<String>,
}

fn find_speller_bundles(dir: &Path) -> Vec<PathBuf> {
    let mut results: Vec<PathBuf> = vec![];

    fn visit_dirs(dir: &Path, results: &mut Vec<PathBuf>) -> io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, results)?;
                } else {
                    if let Some(ext) = path.extension() {
                        if (ext == "zhfst" || ext == "bhfst") && is_readable_archive(&path) {
                            results.push(path.to_owned());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    let err = visit_dirs(dir, &mut results);
    if let Err(e) = err {
        error!("Error listing {:?}: {:?}", dir, e);
    }

    results
}

impl SpellerRepository {
    pub fn new(base_directories: Vec<String>) -> Self {
        SpellerRepository { base_directories }
    }

    pub fn get_speller_archives(&self) -> Vec<PathBuf> {
        self.base_directories
            .iter()
            .flat_map(|base_directory| {
                info!("Enumerate dictionaries in {:?}", base_directory);
                find_speller_bundles(&Path::new(base_directory))
            })
            .collect()
    }

    pub fn get_supported_languages(&self) -> Vec<String> {
        info!("Resolve supported languages");
        let mut res: Vec<String> = self
            .get_speller_archives()
            .iter()
            .filter_map(|path| {
                path.file_stem()
                    .map(|path| resolve_local_name(&path.to_string_lossy()))
            })
            .flatten()
            .collect();
        if monspell_bridge::is_installed() {
            res.extend(resolve_local_name(monspell_bridge::LANGUAGE));
        }
        res.sort();
        res.dedup();
        res
    }

    /// Whether `language_tag` is served by the monspell engine rather than
    /// by a divvunspell archive.
    pub fn is_monspell_language(&self, language_tag: &str) -> bool {
        monspell_bridge::is_installed()
            && resolve_local_name(monspell_bridge::LANGUAGE)
                .iter()
                .any(|tag| tag == language_tag)
    }

    pub fn get_speller_archive(&self, language_tag: &str) -> Option<PathBuf> {
        info!("Get speller archive");
        for path in self.get_speller_archives() {
            let tag_name = path
                .file_stem()
                .map(|path| resolve_local_name(&path.to_string_lossy()));
            if let Some(tags) = tag_name {
                for tag in tags {
                    if tag == language_tag {
                        return Some(path);
                    }
                }
            }
        }

        None
    }
}

#[test]
fn mongolian_tag_expansion() {
    let tags = HARDCODED_TAG_TABLE
        .get("mn")
        .expect("mn not in HARDCODED_TAG_TABLE");
    for expected in &["mn", "mn-MN", "mn-Cyrl", "mn-Cyrl-MN"] {
        assert!(
            tags.iter().any(|t| t == expected),
            "expected {:?} in mn tag expansion, got {:?}",
            expected,
            tags
        );
    }
}

#[test]
fn sami_baseline_preserved() {
    for neutral in &["se", "sma", "smn", "sms", "smj"] {
        let tags = HARDCODED_TAG_TABLE
            .get(*neutral)
            .unwrap_or_else(|| panic!("{} missing from HARDCODED_TAG_TABLE", neutral));
        assert!(
            tags.iter().any(|t| t == &format!("{}-Latn-NO", neutral)),
            "expected {}-Latn-NO in {} expansion",
            neutral,
            neutral
        );
    }
}

#[cfg(test)]
fn temp_archive(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("windivvun-{}-{}", std::process::id(), name));
    fs::write(&path, bytes).expect("write temp archive");
    path
}

#[cfg(test)]
fn box_header(version: u32, trailer: u64) -> Vec<u8> {
    let mut bytes = BOX_MAGIC.to_vec();
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&8u64.to_le_bytes());
    bytes.extend_from_slice(&trailer.to_le_bytes());
    bytes
}

/// The archive that took down msspellcheckinghost.exe: a BOX version 1 file
/// that box-format 0.3.2 parses into a 59-petabyte allocation request.
#[test]
fn box_v1_archive_is_refused() {
    let stub = Path::new(env!("CARGO_MANIFEST_DIR")).join("Spellers/mn.bhfst");
    assert_eq!(read_box_header(&stub).expect("stub header").0, 1);
    assert!(!is_readable_archive(&stub));
    assert!(find_speller_bundles(&stub.parent().expect("Spellers dir"))
        .iter()
        .all(|path| path != &stub));
}

#[test]
fn box_header_validation() {
    let mut good = box_header(0, 32);
    good.resize(64, 0);
    let good = temp_archive("good.bhfst", &good);
    assert!(is_readable_archive(&good));

    let mut v1 = box_header(1, 32);
    v1.resize(64, 0);
    let v1 = temp_archive("v1.bhfst", &v1);
    assert!(!is_readable_archive(&v1));

    // Trailer offset past the end of the file.
    let dangling = temp_archive("dangling.bhfst", &box_header(0, 4096));
    assert!(!is_readable_archive(&dangling));

    let truncated = temp_archive("truncated.bhfst", &BOX_MAGIC);
    assert!(!is_readable_archive(&truncated));

    let garbage = temp_archive("garbage.bhfst", &[0u8; 64]);
    assert!(!is_readable_archive(&garbage));

    // Zip archives go through a different parser and are not judged here.
    let zip = temp_archive("other.zhfst", &[0u8; 8]);
    assert!(is_readable_archive(&zip));

    for path in &[good, v1, dangling, truncated, garbage, zip] {
        let _ = fs::remove_file(path);
    }
}
