# Releasing

Single version source: repo-root `VERSION` (currently `0.1.0`). Release flow:

1. Keep `CHANGELOG.md` `[Unreleased]` current as work lands.
2. Bump `VERSION` and the `[workspace.package]` version in root
   `Cargo.toml` together (all members inherit it).
3. Move the `[Unreleased]` entries under a new
   `## [X.Y.Z] - YYYY-MM-DD` section in `CHANGELOG.md`.
4. Commit, tag `vX.Y.Z`, push tag.
5. `.github/workflows/release.yml` builds the matrix and publishes a GitHub
   Release with per-app archives (`storey-<ver>-<os>-<arch>.tar.gz`,
   `storey-gui-<ver>-<os>-<arch>.tar.gz`, `.zip` on Windows) + `SHA256SUMS`,
   using the new version section as patch notes
   (`sh scripts/release-notes.sh` fails the job instead of shipping
   empty notes, so step 3 is load-bearing):
   - CLI: `mise run build:cli-release` per runner OS (native arch).
   - GUI: `mise run build:gui-release` per runner OS (Qt installed
     per-OS in the workflow). On Windows the needed Qt DLLs/QML
     plugins are bundled next to the exe with `windeployqt`, so the
     zip runs as-is (MSVC redist usually comes with Windows Update).
     Linux/macOS link Qt dynamically: `apt install qt6-base`
     (or similar) / `brew install qt`.
   - macOS runners are arm64, so macOS archives are
     `*-macos-aarch64` throughout (no Rosetta needed).

All binaries report the `[workspace.package]` version (via
`CARGO_PKG_VERSION`); keep it in sync with repo-root `VERSION`.
