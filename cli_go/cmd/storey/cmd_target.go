package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/matyeusm/storey/internal/hash"
	"github.com/matyeusm/storey/internal/index"
)

// targetError is the --json data payload for unresolvable targets.
type targetError struct {
	Error      string   `json:"error"`
	Candidates []string `json:"candidates,omitempty"`
}

// resolveTarget maps a <target> arg to a record position: a path inside
// the library (view link or store object, symlinks resolved, else content
// match) or a full id / unambiguous prefix.
func resolveTarget(root string, ix *index.Index, arg string, emit func(any)) (int, int) {
	if pos, ok := matchPath(root, ix, arg); ok {
		return pos, exitOK
	}
	if looksLikePath(arg) {
		emit(targetError{Error: fmt.Sprintf("no indexed file at %q", arg)})
		return -1, fail("no indexed file at %q", arg)
	}
	rec, ambiguous := ix.FindPrefix(strings.ToLower(arg))
	if ambiguous {
		candidates := prefixCandidates(ix, strings.ToLower(arg))
		emit(targetError{Error: fmt.Sprintf("ambiguous id prefix %q", arg), Candidates: candidates})
		fail("ambiguous id prefix %q: %s", arg, strings.Join(candidates, ", "))
		return -1, exitFailure
	}
	if rec == nil {
		emit(targetError{Error: fmt.Sprintf("no such file or id %q", arg)})
		return -1, fail("no such file or id %q", arg)
	}
	for i := range ix.Files {
		if ix.Files[i].ID == rec.ID {
			return i, exitOK
		}
	}
	return -1, fail("no such file or id %q", arg)
}

// looksLikePath reports whether arg should be treated as a path rather
// than an id: it exists on disk, or contains a path separator.
func looksLikePath(arg string) bool {
	if strings.ContainsRune(arg, '/') || strings.ContainsRune(arg, filepath.Separator) {
		return true
	}
	_, err := os.Lstat(arg)
	return err == nil
}

// matchPath resolves arg as a library path, following symlinks to the
// store object and falling back to content-hash match.
func matchPath(root string, ix *index.Index, arg string) (int, bool) {
	abs := arg
	if !filepath.IsAbs(abs) {
		cwd, err := os.Getwd()
		if err != nil {
			return -1, false
		}
		abs = filepath.Join(cwd, abs)
	}
	if _, err := os.Lstat(abs); err != nil {
		return -1, false
	}
	resolved, err := filepath.EvalSymlinks(abs)
	if err != nil {
		return -1, false
	}
	rel, err := filepath.Rel(root, resolved)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return -1, false
	}
	rel = filepath.ToSlash(rel)
	for i := range ix.Files {
		if ix.Files[i].Location() == rel {
			return i, true
		}
	}
	h, _, err := hash.File(resolved)
	if err != nil {
		return -1, false
	}
	for i := range ix.Files {
		if ix.Files[i].ID == h {
			return i, true
		}
	}
	return -1, false
}

func prefixCandidates(ix *index.Index, prefix string) []string {
	var out []string
	for i := range ix.Files {
		if strings.HasPrefix(ix.Files[i].ID, prefix) {
			out = append(out, ix.Files[i].ID)
		}
	}
	return out
}

// storedValues returns records already ingested into .files/. Only these
// can have view links.
func storedValues(files []index.Record) []index.Record {
	var out []index.Record
	for _, r := range files {
		if r.Stored() {
			out = append(out, r)
		}
	}
	return out
}

// reservedNames are Windows-reserved basenames, matched case-insensitively
// against the stem (up to the first dot).
var reservedNames = map[string]bool{
	"CON": true, "PRN": true, "AUX": true, "NUL": true,
	"COM1": true, "COM2": true, "COM3": true, "COM4": true, "COM5": true,
	"COM6": true, "COM7": true, "COM8": true, "COM9": true,
	"LPT1": true, "LPT2": true, "LPT3": true, "LPT4": true, "LPT5": true,
	"LPT6": true, "LPT7": true, "LPT8": true, "LPT9": true,
}

// checkName validates tag/category/display strings per
// docs/CLI-CONTRACT.md: non-empty after trimming, no <>:"/\|?*, or
// controls, no trailing dot/space, no reserved names. Commas are banned
// because they separate values.
func checkName(s string) error {
	if strings.TrimSpace(s) == "" {
		return fmt.Errorf("name must not be empty")
	}
	for _, r := range s {
		if r < 0x20 || r == 0x7f {
			return fmt.Errorf("name %q contains control characters", s)
		}
		switch r {
		case '<', '>', ':', '"', '/', '\\', '|', '?', '*', ',':
			return fmt.Errorf("name %q contains illegal character %q", s, r)
		}
	}
	if last := s[len(s)-1]; last == '.' || last == ' ' {
		return fmt.Errorf("name %q must not end with a dot or space", s)
	}
	stem := s
	if i := strings.Index(stem, "."); i >= 0 {
		stem = stem[:i]
	}
	if reservedNames[strings.ToUpper(stem)] {
		return fmt.Errorf("name %q is reserved", s)
	}
	return nil
}
