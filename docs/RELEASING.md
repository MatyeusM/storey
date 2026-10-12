# Releasing

Single version source: repo-root `VERSION` (currently `0.1.0`). Release flow:

1. Keep `CHANGELOG.md` `[Unreleased]` current as work lands.
2. Bump `VERSION` and the two per-app versions together:
   - `cli_go/internal/version/version.go` literal,
   - `crates/storey-shared/Cargo.toml` `version` and `apps/gui/Cargo.toml`
     `version` (UIs share the version via `storey-shared`).
3. Move the `[Unreleased]` entries under a new
   `## [X.Y.Z] - YYYY-MM-DD` section in `CHANGELOG.md`.
4. Commit, tag `vX.Y.Z`, push tag.
5. `.github/workflows/release.yml` builds the matrix and publishes a GitHub
   Release with per-OS zips (`storey-linux.zip`, `storey-windows.zip`,
   `storey-macos.zip`, each holding the two binaries) + `SHA256SUMS`,
   using the new version section as patch notes
   (`sh scripts/release-notes.sh` fails the job instead of shipping
   empty notes, so step 3 is load-bearing):
   - CLI: `go build` for `linux`, `windows`, `darwin` (`amd64`;
     see `scripts/dist-cli.sh`).
   - GUI: `mise run build:gui-release` per runner OS (Qt installed
     per-OS in the workflow). On Windows the needed Qt DLLs/QML
     plugins are bundled next to the exe with `windeployqt`, so the
     zip runs as-is (MSVC redist usually comes with Windows Update).
     Linux/macOS link Qt dynamically: `apt install qt6-base`
     (or similar) / `brew install qt`.
   - macOS runners are arm64, so `storey-macos.zip` mixes an arm64 GUI
     binary with the amd64 CLI (Rosetta covers it).

`go build` stamps the version with
`-ldflags "-X .../internal/version.Version=vX.Y.Z"`; the literal in
`version.go` is the fallback for local builds.
