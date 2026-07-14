// Force export names to be undecorated (no __stdcall _Name@NN on x86).
// The Office CSAPI host expects plain names — Divvun's divvunspellmso.dll
// verified via dumpbin: plain `SpellerInit`, not `_SpellerInit@8`.
// MSVC-only: on other targets (contract tests run on macOS/Linux too) the
// /DEF flag would be handed to clang and break the link.
fn main() {
    println!("cargo:rerun-if-changed=monspell_mso.def");
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows-msvc") {
        println!("cargo:rustc-cdylib-link-arg=/DEF:monspell_mso.def");
    }
}
