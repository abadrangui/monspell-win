//! Containment for everything the Windows spell-checking host calls.
//!
//! This DLL runs inside `msspellcheckinghost.exe`, which serves spell
//! checking for every app on the machine. A panic that unwinds out of a COM
//! method or a DLL export takes that process down, so each entry point runs
//! its body through [`hresult`] or [`contain`], which turn a panic into a
//! failure code plus a log line.
//!
//! Not covered: a failed allocation aborts the process instead of panicking,
//! and nothing here can catch that. Input that could ask for an absurd
//! allocation has to be refused before it is parsed; see
//! `speller_repository::is_readable_archive`.

use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use winapi::shared::winerror::E_UNEXPECTED;
use winapi::um::winnt::HRESULT;

const LOG_LIMIT_BYTES: u64 = 256 * 1024;

/// Run the body of a COM method or export that returns an `HRESULT`.
pub fn hresult(entry_point: &'static str, body: impl FnOnce() -> HRESULT) -> HRESULT {
    contain(entry_point, E_UNEXPECTED, body)
}

/// Run the body of an entry point; a panic yields `fallback`.
pub fn contain<R>(entry_point: &'static str, fallback: R, body: impl FnOnce() -> R) -> R {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(value) => value,
        Err(_) => {
            // The panic hook has already logged the message and location.
            report(&format!("{} failed: panic contained", entry_point));
            fallback
        }
    }
}

/// Route panic messages to the error log; the host has no stderr to read.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| report(&format!("panic: {}", info))));
}

/// Record a failure where it can be found afterwards: the trace console when
/// the debug switch is on, and `%LOCALAPPDATA%\monspell\windivvun.log`
/// always. The spell-checking host runs in an AppContainer, so from there
/// the file lands in
/// `%LOCALAPPDATA%\Packages\microsoft^windows^spellchecking^host\AC\monspell\`,
/// and Windows wipes that profile when the host exits: read it while the
/// host is still up. Must not panic: the panic hook calls it.
pub fn report(message: &str) {
    error!("{}", message);
    // Unit tests must not write into the developer's profile.
    #[cfg(not(test))]
    let _ = append_to_log(message);
}

#[cfg_attr(test, allow(dead_code))]
fn append_to_log(message: &str) -> std::io::Result<()> {
    let mut path = match std::env::var_os("LOCALAPPDATA") {
        Some(dir) => PathBuf::from(dir),
        None => return Ok(()),
    };
    path.push("monspell");
    std::fs::create_dir_all(&path)?;
    path.push("windivvun.log");

    let oversized = std::fs::metadata(&path)
        .map(|meta| meta.len() > LOG_LIMIT_BYTES)
        .unwrap_or(false);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(!oversized)
        .truncate(oversized)
        .open(&path)?;
    // One write per line, so lines from concurrent threads do not interleave.
    let line = format!(
        "{} pid={} {}\r\n",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
        std::process::id(),
        message
    );
    file.write_all(line.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use winapi::shared::winerror::S_OK;

    #[test]
    fn panic_becomes_a_failure_code() {
        assert_eq!(hresult("test", || panic!("boom")), E_UNEXPECTED);
        assert_eq!(contain("test", 7, || -> i32 { panic!("boom") }), 7);
    }

    #[test]
    fn normal_results_pass_through() {
        assert_eq!(hresult("test", || S_OK), S_OK);
    }
}
