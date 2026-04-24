# Fork divergence log

Every deliberate divergence between `abadrangui/monspell-win` and upstream `divvun/windivvun-service` is recorded here at the time of divergence. Entries are append-only. Retroactive edits are not acceptable — see `CONTRIBUTING.md` § "Rebase discipline."

Entry format:

```
## YYYY-MM-DD — short title
- **What:** the divergence in one sentence.
- **Where:** the file(s) and roughly where in the file.
- **Why not upstream:** the reason this cannot be submitted as a PR to `divvun/windivvun-service`.
- **Revisit trigger:** the event that would cause us to revisit whether this divergence should still exist (e.g. upstream accepting a related change, a registry-collision complaint, an API change).
- **Links:** commits, ADRs, or issues that bear on the divergence.
```

The related architectural decision lives in [`monspell`'s ADR-0002](https://github.com/abadrangui/monspell/blob/main/docs/decisions/0002-windows-architecture-close-fork.md).

---

## 2026-04-24 — Parallel Inno Setup script for monspell branding + stub speller

- **What:** Added `install-monspell.iss` alongside upstream `install.iss` (kept pristine). Added `Spellers/mn.bhfst` (stub dictionary — flags every word, input-as-suggestion). Added `/dist/` to `.gitignore`.
- **Where:** fork root — `install-monspell.iss`, `Spellers/mn.bhfst`, `.gitignore`.
- **Why not upstream:**
  - Publisher / AppId / CLSID / install path are monspell-specific; patching upstream's file would guarantee collisions in every rebase.
  - Phase 0 intentionally skips `divvunspellmso.dll` (Office 2010–2019 Custom Proofing) and `spelli.exe` (the installer script writes the registry keys inline). Upstream's `install.iss` references both via hard-coded `artifacts\...` paths.
  - `Spellers/mn.bhfst` is a Phase 0 stub built from `divvunspell/tests/fixtures/{eps-lexicon,identity-mutator}.thfst` via `thfst-tools thfsts-to-bhfst`. The real Mongolian speller comes in Phase 1 from `giellalt/lang-khk`.
  - Fresh UUIDs: `AppId = {1BA62C41-FCD1-4350-B2E1-A959B77CA711}`, `CLSID = {AAC10BA6-2952-4734-AAE7-02DB0C9FE6DE}`. Picked new rather than reused from upstream so WinDivvun and monspell can co-install without clobbering each other's COM/WSCAPI registrations.
- **Revisit trigger:** Phase 1 replaces `Spellers/mn.bhfst` with a real FST built from `lang-khk`. Phase 6 (Native OS integrations) will likely need `divvunspellmso.dll` for Office 2013–2019 coverage — if we build our own or consume Divvun's, this script's `[Files]` section expands. Consider re-integrating `spelli` if the registry write needs grow beyond what Inno `[Registry]` can express.
- **Links:** commits adding the files on this branch; main `monspell` repo CHANGELOG "2026-04-24 — Phase 0 installer built" entry.

## 2026-04-24 — Gate off bitrotted upstream `tokens()` test

- **What:** Added `#[cfg(any())]` above the existing `#[test] fn tokens()` in `src/spell_impl/EnumSpellingError.rs` so it does not compile.
- **Where:** `src/spell_impl/EnumSpellingError.rs`, the `#[test]` block that previously started with `let res: Vec<Token> = "Hello world how are you doing".tokenize()`.
- **Why not upstream:** The test is bitrotted against the current `divvunspell::tokenizer` API. It references a `Token` enum and a `.tokenize()` method on `&str`, but the present `Tokenize` trait exposes `word_bound_indices`, `word_indices`, `word_bound_indices_with_alphabet`, and `words_with_alphabet` — and there is no `Token` enum (the closest type is `IndexedWord`). `cargo test` therefore refuses to compile the lib-test binary at all, which blocks *any* test from running in this crate. Rewriting the test is possible but requires deciding what it should actually assert; gating it off is the minimum change to unblock `cargo test` for the `HARDCODED_TAG_TABLE` tests added in `feature/mn-tag-table`. The rewrite is upstream-candidate work and should be submitted as a separate PR; this gate-off is fork-local until that lands.
- **Revisit trigger:** Either upstream PRs a rewritten `tokens()` test against the current API, or this fork rewrites it as a commons-contribution PR. Either way the `#[cfg(any())]` comes off in the rebase that picks up the rewrite.
- **Links:** commit on `feature/mn-tag-table` that added the gate; main `monspell` repo CHANGELOG.md "2026-04-24 — Phase 0 build unblocked" entry.
