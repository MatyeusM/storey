# Plugin guide: tag suggesters

Storey ships **one built-in suggestion source**: embedded file
metadata (`storey-meta`: audio tags, JPEG EXIF, PNG text) behind the
GUI's `Metadata` button. It is local-only by construction. Every
other suggestion source is
an external plugin — a program in any language that reads one record
and prints suggested tags. Rule of thumb: local file reads belong
in the program, anything needing the network belongs in a plugin.
The GUI (`storey-gui`) queries
plugins through `ui` (`crates/ui/src/tags.rs`). A person or an LLM following this file can write, register,
and debug a plugin without reading the Rust code.

## Shapes: file or directory

A plugin is either:

- a **single executable file** (anything with the execute bit: a binary,
  a shell script with a shebang, `suggest.py` made executable), run
  directly with no arguments; or
- a **directory** holding a `manifest.json`, for anything needing an
  interpreter, dependencies, or extra files (e.g. `python3 suggest.py`).

The working directory is the plugin's own directory for directory
plugins, or the executable's parent directory for file plugins.

## `manifest.json` (directory plugins only)

```json
{
  "name": "music",
  "version": "0.1.0",
  "description": "acoustic-id lookup",
  "entrypoint": ["python3", "suggest.py"],
  "supported_extensions": ["mp3", "flac"],
  "protocol": 1
}
```

| Field | Required | Meaning |
|---|---|---|
| `name` | yes | Human name; shown in the UIs. |
| `version` | no (default `""`) | Shown in the GUI Plugins dialog. |
| `description` | no (default `""`) | Human blurb; not displayed yet. |
| `entrypoint` | yes, non-empty | Program plus args (see below). |
| `supported_extensions` | no (default `[]`) | **Documentary only** — it is parsed but never enforced. The effective filter is the `extensions` allow-list in `ui.toml`. |
| `protocol` | no (default `1`) | Must be `1`; anything else is rejected. |

Entrypoint resolution (`manifest::resolve` + `split_entrypoint`):

- The first element is the program, the rest are its arguments.
- An absolute first element is used as-is.
- A relative first element naming a file inside the plugin directory
  resolves against that directory (`"./suggest.py"`, `"bin/run"`).
- Anything else (e.g. `"python3"`) is looked up on `PATH` at spawn time.
- A missing path, an empty entrypoint, or a directory without a
  readable `manifest.json` marks the plugin broken (see Troubleshooting).

## Activating a plugin (`ui.toml`)

Plugins are declared, in query order, in `storey/ui.toml`:

| OS | Location |
|---|---|
| Linux | `$XDG_CONFIG_HOME/storey/ui.toml`, else `~/.config/storey/ui.toml` |
| macOS | `~/Library/Application Support/storey/ui.toml` |
| Windows | `%APPDATA%\storey\ui.toml` |

```toml
[[plugins]]
name = "music"
path = "/home/u/plugins/suggest-music"   # file or manifest.json dir
enabled = true                            # default true
extensions = ["mp3", "flac"]              # default [] = all files
timeout_secs = 10                         # default 10, minimum 1
```

| Field | Required | Meaning |
|---|---|---|
| `name` | yes | Label for the Plugins-view row and error messages. (The `source` shown per suggestion comes from the plugin's own output, not from here.) |
| `path` | yes | Executable file or directory with `manifest.json`. |
| `enabled` | no (default `true`) | `false` skips the plugin entirely. |
| `extensions` | no (default `[]`) | Allow-list matched case-insensitively against the record's file extension. Empty means every record is passed to the plugin. |
| `timeout_secs` | no (default `10`) | Kill the plugin if it runs longer (floored at 1s). |

The file is optional; a missing file means defaults (no plugins).
Only `enabled` plugins are queried, in declared order.

## Communication contract (protocol 1)

One lookup is one child-process run:

1. The UI spawns the resolved program with the resolved args.
2. It writes **exactly one JSON object** — the record — to the
   plugin's **stdin**, then closes stdin (EOF).
3. The plugin writes **exactly one JSON array** — its suggestions —
   to **stdout** and exits `0`.
4. Anything on **stderr is discarded** (it goes to null, so log there
   freely while debugging — but nothing will show it).

### stdin: the record

Field names are camelCase on the wire. Both rels arrive as **absolute
paths** (joined onto the library root from `storey status`), so the
plugin can open the file without knowing the library layout:

```json
{
  "id": "98ea6e4f216f2fb4b69fff9b3a44842c38686ca685f3f55dc48c5d3fb1107be4",
  "displayName": "Concert.mkv",
  "originalRel": "/media/Elements/Audio/films/Concert.mkv",
  "storedRel": "/media/Elements/Audio/.files/Concert.mkv",
  "size": 123456,
  "tags": {"genre": ["rock"], "date": ["2020"]}
}
```

- To inspect file bytes, open `storedRel` when non-empty, otherwise
  `originalRel` (the file is not yet ingested into `.files/`).
- An empty `storedRel` stays `""` — it never points at the root itself.
- `storedRel` may be absent in older payloads; treat missing as `""`.
- Prefer `displayName` for extension checks and display.
- `tags` maps category → value list and may be `{}`.

### stdout: the suggestions

```json
[
  {"category": "genre", "value": "jazz", "score": 0.9, "source": "music"},
  {"category": "date", "value": "2020", "score": 0.4, "source": "music"}
]
```

- `category` / `value`: the suggested tag. Empty strings are passed
  through untouched, so don't emit them.
- `score`: `0.0`–`1.0` confidence. Higher wins when two plugins suggest
  the same `category`+`value` pair (see Ranking).
- `source`: free text naming where the hit came from (plugin name,
  database, model). Shown in the GUI suggestion list.
- `[]` is a valid answer: "no suggestions for this record".

### Failure rules

- Non-zero exit: all output is discarded and the lookup errors.
- Invalid JSON on stdout: the lookup errors.
- Overrunning `timeout_secs`: the child is killed, the lookup errors.
- A failing plugin never blocks the others; the GUI skips it and keeps
  the rest. (The GUI reports "no suggestions" only when *nothing*
  came back from *any* plugin.)

### Extension filtering (before spawning)

A plugin is not even spawned for a record whose extension is outside
its `ui.toml` `extensions` list (case-insensitive; empty list = all).
That counts as "no suggestions", not an error.

### Ranking and merge (`tags::rank`)

The GUI concatenates every plugin's suggestions, sorts by `score`
descending, and dedupe by (`category`, `value`), keeping the highest
score. Applying suggestions writes them with `tag add` grouped by
category — merges only, never clobbers existing values.

## Minimal examples

Directory plugin (Python). `~/plugins/upper/manifest.json`:

```json
{
  "name": "upper",
  "version": "0.1.0",
  "entrypoint": ["python3", "suggest.py"],
  "protocol": 1
}
```

`~/plugins/upper/suggest.py` (executable not required — `python3`
is the program):

```python
import json, sys

record = json.load(sys.stdin)  # read to EOF
name = record.get("displayName", "")
suggestions = []
if name.lower().endswith(".mkv"):
    suggestions.append({
        "category": "media",
        "value": "video",
        "score": 1.0,
        "source": "upper",
    })
json.dump(suggestions, sys.stdout)
```

Single-file plugin (shell). `~/bin/suggest-constant`, executable:

```sh
#!/bin/sh
# Ignore stdin, always suggest one tag.
printf '[{"category":"checked","value":"yes","score":0.5,"source":"constant"}]'
```

Register either in `ui.toml`:

```toml
[[plugins]]
name = "upper"
path = "/home/u/plugins/upper"

[[plugins]]
name = "constant"
path = "/home/u/bin/suggest-constant"
extensions = ["mkv"]
```

Test by hand before registering — feed a record on stdin and inspect
stdout (exit code must be `0`):

```sh
echo '{"id":"ab","displayName":"a.mkv","originalRel":"a.mkv","size":1,"tags":{}}' \
  | python3 ~/plugins/upper/suggest.py; echo "exit=$?"
```

## How the GUI uses plugins

- **Tag editor**: `Suggest` runs `lookup` on every enabled plugin
  through `crates/ui` and lists ranked suggestions under the
  editor; `Apply all` merges them via grouped `tag add`.
- **Plugins dialog**: one row per `[[plugins]]` declaration with
  health — `ok`, `missing binary` (program not found), or the
  resolve error. Health comes from `ExternalPlugin::from_decl`
  plus `available()`: a bare program name is assumed to be on `PATH`,
  while any path containing a separator must exist.

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| Row shows the resolve error | `path` missing, directory without `manifest.json`, empty `entrypoint`, or `protocol` ≠ 1. |
| Row shows `missing binary` | Entrypoint program not on `PATH` / not at the resolved path (`available()` check). |
| Plugin never runs for some files | `extensions` allow-list in `ui.toml` (remember: the manifest's `supported_extensions` is **not** enforced). |
| Lookup errors, nothing applied | Non-zero exit, invalid JSON on stdout, or timeout — test by hand per above. |
| Plugin runs but suggests nothing | It printed `[]`, or the extension filter skipped the record. |
| Hangs the UI briefly | Lookup is synchronous up to `timeout_secs`; lower it while iterating. |
