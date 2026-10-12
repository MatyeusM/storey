# storey (CLI)

Go CLI. Source of truth for CAS + index + views. The GUI shells out to the
`storey` binary — no shared libraries.

Implemented: `init` (creates only `.index.json`, adopts nested
libraries, `--dry-run` preview), `update` (trusted by default,
`--verify` to re-hash all, `--dry-run` preview, moved files
repointed instead of purged), `view` (ingests into `.files/`,
symlink/hardlink only with a pre-move filesystem probe, flat
`Unsorted` for fully untagged files, crash-residue reconcile,
`--dry-run` plan), `restore`, `rm`/`remove`, `mv`, `tag`, `list`
(plus `--help`/`--version`, `--json` envelope with live stderr JSONL
progress, `--quiet`, exit codes). Mutating commands hold an
`.index.lock` write lock; the CLI contract in `docs/CLI-CONTRACT.md`
is fully covered.

```sh
mise run build:cli   # -> bin/storey
mise run dev:cli     # go run ./cmd/storey --help
mise run test:cli    # vet + test
```

Version: `internal/version/version.go` literal + `-ldflags` override at
release time; must match repo-root `VERSION`.

## Conventions

- Stdlib only (no third-party dependencies).
- Files ≤ 300 lines, functions ≤ 50 lines — enforced by
  `scripts/check-limits.sh` via `mise run lint:cli` (same limits apply
  to all Rust crates).
- Never interactive: no prompts, no stdin reads.
