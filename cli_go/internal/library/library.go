// Package library handles library root discovery and directory walks.
package library

import (
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/matyeusm/storey/internal/index"
)

// ErrNotFound is returned when no library root exists above start.
var ErrNotFound = errors.New("not inside a storey library (no .index.json found)")

// FindRoot returns the nearest ancestor of start (including start)
// containing .index.json.
func FindRoot(start string) (string, error) {
	abs, err := filepath.Abs(start)
	if err != nil {
		return "", err
	}
	for {
		if _, err := os.Stat(filepath.Join(abs, index.FileName)); err == nil {
			return abs, nil
		}
		parent := filepath.Dir(abs)
		if parent == abs {
			return "", ErrNotFound
		}
		abs = parent
	}
}

// HasIndexAt reports whether dir itself directly contains .index.json.
func HasIndexAt(dir string) bool {
	_, err := os.Stat(filepath.Join(dir, index.FileName))
	return err == nil
}

// Walk returns slash-separated paths (relative to root) of all regular
// files under root, sorted. It skips .files/ dirs, every .index.json
// (root or nested — nested libraries merge via adopt, never as content),
// every symlink, OS junk (thumbnails, caches, partial downloads), and
// never descends into symlinked directories.
func Walk(root string) ([]string, error) {
	var paths []string
	err := filepath.WalkDir(root, func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(root, p)
		if err != nil {
			return err
		}
		if rel == "." {
			return nil
		}
		name := d.Name()
		if d.IsDir() {
			if name == index.StoreDir || d.Type()&fs.ModeSymlink != 0 {
				return filepath.SkipDir
			}
			return nil
		}
		if d.Type()&fs.ModeSymlink != 0 {
			return nil
		}
		if !d.Type().IsRegular() {
			return nil
		}
		if name == index.FileName || name == index.LockName || ignoredFile(name) {
			return nil
		}
		paths = append(paths, filepath.ToSlash(rel))
		return nil
	})
	if err != nil {
		return nil, err
	}
	sort.Strings(paths)
	return paths, nil
}

// ignoredNames are exact basenames never indexed: OS thumbnails,
// caches, and progress markers that would otherwise pollute libraries.
var ignoredNames = map[string]bool{
	".DS_Store": true, "Thumbs.db": true, "desktop.ini": true,
}

// ignoredSuffixes are partial-download markers: indexing them would
// freeze a half-written file under its hash.
var ignoredSuffixes = []string{".part", ".crdownload"}

// ignoredFile reports OS junk by exact name or download suffix.
func ignoredFile(name string) bool {
	if ignoredNames[name] {
		return true
	}
	for _, suffix := range ignoredSuffixes {
		if strings.HasSuffix(name, suffix) {
			return true
		}
	}
	return false
}

// FindUntracked returns sorted slash-relative paths of regular files that
// are neither at a tracked location nor a known view artifact. tracked
// holds current record locations; artifacts holds derived view link paths
// (which catches hardlink/copy-fallback links that Walk cannot tell from
// real files; symlinks never appear since Walk skips them).
func FindUntracked(root string, tracked, artifacts map[string]bool) ([]string, error) {
	paths, err := Walk(root)
	if err != nil {
		return nil, err
	}
	var untracked []string
	for _, p := range paths {
		if !tracked[p] && !artifacts[p] {
			untracked = append(untracked, p)
		}
	}
	return untracked, nil
}

// CurrentLocations returns the set of current record paths (stored or in
// place), slash-relative.
func CurrentLocations(files []index.Record) map[string]bool {
	set := map[string]bool{}
	for i := range files {
		set[files[i].Location()] = true
	}
	return set
}
