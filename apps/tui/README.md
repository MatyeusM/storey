# storey-tui

Rust + ratatui terminal interface. Thin client over the `storey` CLI binary
— all CAS/index/view logic stays in `apps/cli`; the TUI shells out to it
(see `docs/CLI-CONTRACT.md`).

```sh
mise run build:tui          # debug build (bundles the CLI next door)
mise run build:tui-release  # release build
mise run test:tui           # cargo test
mise run lint:tui           # fmt + clippy + size limits
```

`--storey-bin` overrides binary resolution; otherwise `ui.toml`,
a sibling `storey`, then `PATH`. Plugins come only from `ui.toml`
`[[plugins]]` entries.

Views: `Browse` (breadcrumbs in libraries, file browser in plain dirs;
`r` restores stored files back when the view is empty), `Files`
(`storey list` rows: `j/k` move, `Enter` tags, `u`/`U` update, `F`
filter, `d` remove with confirm, `m` rename, `Esc` back), `Detail`
(per-record tagging: `c`/`v` edit, `m` set/add, `X` remove category,
`s` plugin suggestions, `P` plugins), `Plugins` (declaration health,
`Enter` opens the plugin folder). Long `update`/`init`/`restore` runs
execute in the background with a progress readout at the control bar's
right end.

Version: `Cargo.toml` `version` must match repo-root `VERSION`.
