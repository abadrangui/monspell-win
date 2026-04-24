# Contributing to `monspell-win`

This repository is a **close fork** of [`divvun/windivvun-service`](https://github.com/divvun/windivvun-service). It exists to ship the Windows side of the open-source Mongolian spellchecker [`monspell`](https://github.com/abadrangui/monspell). The architectural decision is recorded in `monspell`'s [ADR-0002](https://github.com/abadrangui/monspell/blob/main/docs/decisions/0002-windows-architecture-close-fork.md).

This file exists so that future maintainers (including future-me) do not let this fork silently drift into a hard fork. The rebase discipline below is load-bearing.

## Relationship to upstream

- `origin` = `github.com/abadrangui/monspell-win` (this fork, Mongolian-focused).
- `upstream` = `github.com/divvun/windivvun-service` (the original Divvun service).
- Upstream license is `Apache-2.0 OR MIT`; both licenses flow through this fork unchanged.
- Upstream authorship is preserved on every rebase. Commits added in this fork are either:
  - **Mongolian-specific and upstream-candidate** — e.g. adding `mn` to `HARDCODED_TAG_TABLE`. Submitted as a PR to `divvun/windivvun-service` and carried locally only until merged upstream.
  - **Deliberately divergent and fork-local** — e.g. registry root renamed from `HKLM\SOFTWARE\WinDivvun\...` to `HKLM\SOFTWARE\monspell\...` to allow co-installation. Every such divergence is logged in `docs/fork-divergence.md`.

If a change does not fit one of those two categories, it probably should not be made. Prefer filing an upstream issue or PR first.

## Rebase discipline

This is the part that separates a close fork from a hard fork. Lapsing here is the single most likely way this project fails.

- **Cadence.** Rebase `master` onto `upstream/master` every **2–3 months**, or within **30 days of an upstream tagged release**, whichever comes first.
- **Divergence budget.** If accumulated divergence exceeds **~200 net lines of non-trivial change**, rebase immediately regardless of cadence. This number is intentionally small; it is the trip wire.
- **Upstream-first rule.** Before committing a change locally, ask whether it could live upstream. If yes, open the PR against `divvun/windivvun-service` first. Carry the patch in this fork only while the upstream PR is pending.
- **Divergence log.** Every deliberate divergence gets an entry in `docs/fork-divergence.md` at the time of divergence, including the reason it cannot be upstreamed. Retroactive entries are not acceptable — they defeat the purpose.
- **Skipped rebase = re-open ADR-0002.** If we skip a scheduled rebase, or if the divergence log is growing without corresponding upstream PRs, the discipline is breaking. The honest response is to re-open ADR-0002 in the main `monspell` repo and either recommit to discipline or adopt a hard-fork posture explicitly.

## Branches

- `master` — fork-primary branch. Solo-owner workflow: commits land directly here. Matches upstream where possible; diverges only where `docs/fork-divergence.md` records it.
- `upstream/master` — upstream's `master`, used as the rebase target. Do not push to this remote.
- **Upstream PRs** — when a commit is a commons contribution to `divvun/windivvun-service`, cherry-pick it onto a throwaway branch at PR-submission time, push, open the PR, and delete the branch after merge. Do not maintain long-lived upstream-PR-candidate branches.

The `divvun/windivvun-service` upstream uses `master`, `develop`, `release`, and `feature/*` branches. This fork mirrors only `master`; the others are tracked via `upstream/*` and ignored unless a rebase target changes.

## Commits

- Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`).
- DCO sign-off on every commit: `git commit -s`.
- No CLA. Upstream accepts DCO-signed contributions.

## Building

See the upstream `README.md`. Toolchain requirement: `nightly-i686-pc-windows-msvc` with the Windows 10/11 SDK. Changes that affect build steps must be reflected in both files, or the README delta captured in `docs/fork-divergence.md`.

## Issues and PRs

- Bugs in Mongolian-specific behavior (tag-table entries, `.bhfst` packaging, registry path): file here.
- Bugs in the general WSCAPI implementation, COM plumbing, or Sámi language handling: file upstream.
- When in doubt: file upstream first.
