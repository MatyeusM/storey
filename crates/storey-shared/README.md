# storey-shared

Rust library backing `storey-gui`. The GUI drives
the `storey` CLI as a subprocess and never touch the store or index
directly — this crate holds what they share (see
`docs/CLI-CONTRACT.md`):

- `cli` — typed `Client` driver: `bin --json` in a chosen dir,
  stdout envelope parsed even on failures, stderr JSONL logs
  streamed live (progress, warnings) via callback, `CliError`
  keeps exit code.
- `commands` — one `Client` method per contract command
  (`status`, `list`, `tag get/set/add/remove`, `view`, ...),
  returning `model` types directly.
- `model` — `Record`, `Status`, `ViewReport` plus display helpers
  (`short_id`, `display_tags`, `display_view`).
- `tags` — plugin-only suggesters (`ExternalPlugin`: record JSON on
  stdin, suggestion array on stdout), `Suggestion`, `rank`.
- `settings` — `storey/ui.toml` in the OS config dir (`APPDATA` /
  `Library/Application Support` / `XDG_CONFIG_HOME`): `storey_bin`
  plus ordered `[[plugins]]` (`name`, `path`, `enabled`,
  `extensions`, `timeout_secs`).
- `manifest` — directory-plugin `manifest.json` (`entrypoint`,
  `supported_extensions`, protocol check); file paths run directly.

```sh
mise run test:shared   # cargo test
mise run lint:shared   # fmt check + clippy
```

Version: `Cargo.toml` `version` must match repo-root `VERSION`.
