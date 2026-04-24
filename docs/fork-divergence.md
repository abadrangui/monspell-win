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

_(No divergences recorded yet. First entries will land as Phase 0 implementation diverges from upstream — e.g. registry root rename from `HKLM\SOFTWARE\WinDivvun\Spellers\` to `HKLM\SOFTWARE\monspell\Spellers\`, if that turns out to require source changes rather than configuration.)_
