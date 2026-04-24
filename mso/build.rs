// Force export names to be undecorated (no __stdcall _Name@NN on x86).
// The Office CSAPI host expects plain names — Divvun's divvunspellmso.dll
// verified via dumpbin: plain `SpellerInit`, not `_SpellerInit@8`.
fn main() {
    println!("cargo:rerun-if-changed=monspell_mso.def");
    println!("cargo:rustc-cdylib-link-arg=/DEF:monspell_mso.def");
}
