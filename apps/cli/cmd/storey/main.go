// Command storey is the content-addressable storage CLI.
// See docs/CLI-CONTRACT.md for the command surface. Never interactive:
// every command is fully determined by its flags.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/matyeusm/storey/internal/hash"
	"github.com/matyeusm/storey/internal/index"
	"github.com/matyeusm/storey/internal/library"
	"github.com/matyeusm/storey/internal/log"
	"github.com/matyeusm/storey/internal/version"
)

// Exit codes per docs/CLI-CONTRACT.md.
const (
	exitOK      = 0
	exitFailure = 1
	exitUsage   = 2
	// exitInterrupted is returned after an abort that saved the index.
	exitInterrupted = 130
)

func main() {
	os.Exit(run(os.Args[1:]))
}

// envelope is the --json stdout shape.
type envelope struct {
	Schema  int    `json:"schema"`
	Command string `json:"command"`
	Data    any    `json:"data"`
}

func run(args []string) int {
	asJSON, quiet, args := splitGlobals(args)
	for len(args) > 0 {
		switch args[0] {
		case "--help", "-h":
			usage()
			return exitOK
		case "--version":
			fmt.Println("storey " + version.Version)
			return exitOK
		default:
			goto dispatch
		}
	}
dispatch:
	if len(args) == 0 {
		usage()
		return exitUsage
	}
	cmd, rest := args[0], args[1:]
	emit := func(data any) {
		if asJSON {
			enc := json.NewEncoder(os.Stdout)
			enc.Encode(envelope{Schema: 1, Command: cmd, Data: data})
		}
	}
	logger := log.New(asJSON, quiet)
	return dispatch(cmd, rest, asJSON, logger, emit)
}

// splitGlobals extracts --json and --quiet/-q wherever they appear,
// so `update --verify --json` works like `--json update --verify`.
// No command accepts these tokens positionally, so floating them only
// turns previous usage errors into honored flags.
func splitGlobals(args []string) (asJSON, quiet bool, rest []string) {
	for _, a := range args {
		switch a {
		case "--json":
			asJSON = true
		case "--quiet", "-q":
			quiet = true
		default:
			rest = append(rest, a)
		}
	}
	return asJSON, quiet, rest
}

func dispatch(cmd string, rest []string, asJSON bool, logger *log.Logger, emit func(any)) int {
	switch cmd {
	case "init":
		return cmdInit(rest, asJSON, logger, emit)
	case "view":
		return cmdView(rest, asJSON, logger, emit)
	case "restore":
		return cmdRestore(rest, asJSON, logger, emit)
	case "update":
		return cmdUpdate(rest, asJSON, logger, emit)
	case "remove", "rm":
		return cmdRm(rest, asJSON, logger, emit)
	case "mv":
		return cmdMv(rest, asJSON, logger, emit)
	case "tag":
		return cmdTag(rest, asJSON, logger, emit)
	case "list":
		return cmdList(rest, asJSON, logger, emit)
	case "status":
		return cmdStatus(rest, asJSON, logger, emit)
	default:
		if cmd == "--help" || cmd == "-h" {
			usage()
			return exitOK
		}
		fmt.Fprintf(os.Stderr, "storey: unknown command %q\n", cmd)
		usage()
		return exitUsage
	}
}

func usage() {
	fmt.Println("storey " + version.Version)
	fmt.Println("usage: storey [--json] [--quiet] <command> [args]")
	fmt.Println("commands: init update view list tag remove|rm mv restore status")
	fmt.Println("logs and hashing progress go to stderr (JSONL under --json);")
	fmt.Println("see docs/CLI-CONTRACT.md")
}

func fail(format string, args ...any) int {
	fmt.Fprintf(os.Stderr, "storey: "+format+"\n", args...)
	return exitFailure
}

func usageErr(format string, args ...any) int {
	fmt.Fprintf(os.Stderr, "storey: "+format+"\n", args...)
	return exitUsage
}

// loadLibrary resolves the library root from cwd and loads its index.
func loadLibrary() (string, *index.Index, int) {
	cwd, err := os.Getwd()
	if err != nil {
		return "", nil, fail("%s", err)
	}
	root, err := library.FindRoot(cwd)
	if err != nil {
		return "", nil, usageErr("%s", err)
	}
	ix, err := index.Load(root)
	if err != nil {
		return "", nil, fail("%s", err)
	}
	return root, ix, exitOK
}

// loadLocked resolves the root, claims the write lock, then loads:
// holding the lock across the whole read-modify-write span is what
// prevents lost updates between concurrent writers. Callers release
// via defer (fail returns, never exits, so defers always run).
// Readers (status, list, tag get) stay on loadLibrary so polling
// never blocks: the index itself saves atomically.
func loadLocked() (string, *index.Index, func(), int) {
	noop := func() {}
	cwd, err := os.Getwd()
	if err != nil {
		return "", nil, noop, fail("%s", err)
	}
	root, err := library.FindRoot(cwd)
	if err != nil {
		return "", nil, noop, usageErr("%s", err)
	}
	release, exit := lockLibrary(root)
	if exit != exitOK {
		return "", nil, noop, exit
	}
	ix, err := index.Load(root)
	if err != nil {
		release()
		return "", nil, noop, fail("%s", err)
	}
	return root, ix, release, exitOK
}

// lockLibrary claims the library for writing with an atomic create:
// a second writer fails loudly instead of clobbering the first one's
// records. A leftover lock (killed process) needs a manual remove,
// which the error names along with the owning pid.
func lockLibrary(root string) (func(), int) {
	noop := func() {}
	path := filepath.Join(root, index.LockName)
	f, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o644)
	if err != nil {
		holder := "unknown"
		if raw, rerr := os.ReadFile(path); rerr == nil {
			holder = strings.TrimSpace(string(raw))
		}
		return noop, fail("library locked by pid %s (remove %s if no storey process is running)", holder, path)
	}
	fmt.Fprintf(f, "%d\n", os.Getpid())
	f.Close()
	return func() { os.Remove(path) }, exitOK
}

// loadIndex loads the validated index at root.
func loadIndex(root string) (*index.Index, error) {
	return index.Load(root)
}

// findUntracked lists regular files that are neither tracked nor derived
// view artifacts of the recorded view.
func findUntracked(root string, ix *index.Index) ([]string, int) {
	untracked, err := library.FindUntracked(root, library.CurrentLocations(ix.Files), planSet(ix.Files, ix.View.By))
	if err != nil {
		return nil, fail("%s", err)
	}
	if untracked == nil {
		// Keep --json arrays (nil slices marshal to null, which
		// breaks typed consumers of view/restore payloads).
		untracked = []string{}
	}
	return untracked, exitOK
}

// reportBlocked names untracked files and conflicts on stderr.
func reportBlocked(cmd string, untracked, conflicts []string) {
	fmt.Fprintf(os.Stderr, "storey: %s: blocked (use --force to destroy/overwrite):\n", cmd)
	for _, u := range untracked {
		fmt.Fprintf(os.Stderr, "  untracked: %s\n", u)
	}
	for _, c := range conflicts {
		fmt.Fprintf(os.Stderr, "  conflict: %s\n", c)
	}
}

// missingLocations lists indexed paths absent from disk.
func missingLocations(root string, ix *index.Index) []string {
	var missing []string
	for i := range ix.Files {
		abs := filepath.Join(root, filepath.FromSlash(ix.Files[i].Location()))
		if _, err := os.Lstat(abs); err != nil {
			missing = append(missing, ix.Files[i].Location())
		}
	}
	return missing
}

// rootJoin joins a slash-relative record path onto root.
func rootJoin(root, rel string) string {
	return filepath.Join(root, filepath.FromSlash(rel))
}

// baseName returns the basename of a slash-relative path.
func baseName(rel string) string {
	return filepath.Base(filepath.FromSlash(rel))
}

// hashRelPaths hashes slash-relative paths under root with the worker
// pool, keying results by relative path. Progress is reported as op
// ("hash" or "verify") with slash-relative paths for the logger.
// Unreadable files are skipped and reported, never fatal: the caller
// appends them to Problems and indexes the rest.
func hashRelPaths(root string, rels []string, logger *log.Logger, op string) (map[string]hash.Result, []string, []string, int) {
	abs := make([]string, len(rels))
	relOf := make(map[string]string, len(rels))
	for i, rel := range rels {
		abs[i] = filepath.Join(root, filepath.FromSlash(rel))
		relOf[abs[i]] = rel
	}
	byPath := map[string]hash.Result{}
	var hashed, problems []string
	results := hash.Files(abs, hash.Workers(), func(done, total int, path string) {
		logger.Progress(op, done, total, relOf[path])
	})
	for _, r := range results {
		rel := relOf[r.Path]
		if r.Err != nil {
			problems = append(problems, rel+": "+r.Err.Error())
			continue
		}
		byPath[rel] = r
	}
	for _, rel := range rels {
		if _, ok := byPath[rel]; ok {
			hashed = append(hashed, rel)
		}
	}
	return byPath, hashed, problems, exitOK
}
