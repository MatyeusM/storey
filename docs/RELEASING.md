# Releasing

Single version source: repo-root `VERSION` (currently `0.1.0`). Release flow:

1. Keep `CHANGELOG.md` `[Unreleased]` current as work lands.
2. Bump `VERSION` and the three per-app versions together:
   - `apps/cli/internal/version/version.go` literal,
   - `apps/tui/Cargo.toml` `version`,
   - `apps/shared/Cargo.toml` `version` and `apps/gui/Cargo.toml`
     `version` (UIs share the version via `storey-shared`).
3. Move the `[Unreleased]` entries under a new
   `## [X.Y.Z] - YYYY-MM-DD` section in `CHANGELOG.md`.
4. Commit, tag `vX.Y.Z`, push tag.
5. `.github/workflows/release.yml` builds the matrix and publishes a GitHub
   Release with artifacts + `SHA256SUMS`, using the new version
   section as patch notes (`sh scripts/release-notes.sh` fails the
   job instead of shipping empty notes, so step 3 is load-bearing):
   - CLI: `go build` for `linux`, `windows`, `darwin` (`amd64`;
     see `scripts/dist-cli.sh`).
   - TUI: `mise run build:tui-release` per runner OS (native arch),
     shipping the CLI next door so the binary resolves it.
   - GUI: `mise run build:gui-release` per runner OS (Qt installed
     per-OS in the workflow). The GUI links Qt dynamically, so the
     binary needs system Qt6 (>= 6.6) at runtime too.

`go build` stamps the version with
`-ldflags "-X .../internal/version.Version=vX.Y.Z"`; the literal in
`version.go` is the fallback for local builds.
