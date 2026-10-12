// Package views holds the pure view mapping: which store object links
// from which view path. Shared by view (build), rm and restore (derive).
// Deterministic in record order, so rebuilds are stable without storing
// the mapping.
package views

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/matyeusm/storey/internal/index"
)

// Unsorted is the view folder for objects missing a value at a level.
const Unsorted = "Unsorted"

// Link maps one store object to one view path (slash-separated,
// relative to root).
type Link struct {
	ViewPath string
	ID       string
}

// Plan computes view links for records under key order by. A missing
// value at a level maps to Unsorted/; multiple values expand to the
// cartesian product across levels. Path collisions (same path, or paths
// differing only by case) get a deterministic __2, __3, ... suffix on the
// link basename, in record order.
func Plan(records []index.Record, by []string) []Link {
	var links []Link
	used := map[string]bool{} // lower(viewPath) -> taken
	for _, rec := range records {
		paths := expand(rec, by)
		for _, p := range paths {
			p = disambiguate(p, used)
			used[strings.ToLower(p)] = true
			links = append(links, Link{ViewPath: p, ID: rec.ID})
		}
	}
	return links
}

// expand returns the view dir paths (without basename) for one record.
func expand(rec index.Record, by []string) []string {
	// Missing every level: one flat Unsorted, not one per level.
	if len(by) > 0 && missingAll(rec, by) {
		return []string{join(Unsorted, rec.DisplayName)}
	}
	dirs := []string{""}
	for _, cat := range by {
		vals := rec.Tags[cat]
		if len(vals) == 0 {
			vals = []string{Unsorted}
		}
		var next []string
		for _, d := range dirs {
			for _, v := range vals {
				next = append(next, join(d, v))
			}
		}
		dirs = next
	}
	paths := make([]string, 0, len(dirs))
	for _, d := range dirs {
		paths = append(paths, join(d, rec.DisplayName))
	}
	return paths
}

// missingAll reports no tag values for any key in by.
func missingAll(rec index.Record, by []string) bool {
	for _, cat := range by {
		if len(rec.Tags[cat]) > 0 {
			return false
		}
	}
	return true
}

func join(dir, name string) string {
	if dir == "" {
		return name
	}
	return dir + "/" + name
}

// disambiguate appends __2, __3, ... to the basename while the
// case-folded path is taken by another record.
func disambiguate(path string, used map[string]bool) string {
	if !used[strings.ToLower(path)] {
		return path
	}
	dir, base := split(path)
	ext := filepath.Ext(base)
	stem := strings.TrimSuffix(base, ext)
	for i := 2; ; i++ {
		candidate := join(dir, fmt.Sprintf("%s__%d%s", stem, i, ext))
		if !used[strings.ToLower(candidate)] {
			return candidate
		}
	}
}

func split(path string) (dir, base string) {
	if i := strings.LastIndex(path, "/"); i >= 0 {
		return path[:i], path[i+1:]
	}
	return "", path
}

// StoreName resolves the .files/ object name for display: display itself,
// or stem_N.ext while the name is taken by different content.
// taken maps store basename -> id.
func StoreName(display string, taken map[string]string, id string) string {
	if owner, ok := taken[display]; !ok || owner == id {
		return display
	}
	ext := filepath.Ext(display)
	stem := strings.TrimSuffix(display, ext)
	for i := 1; ; i++ {
		candidate := fmt.Sprintf("%s_%d%s", stem, i, ext)
		if owner, ok := taken[candidate]; !ok || owner == id {
			return candidate
		}
	}
}

// CreateLink creates linkAbs pointing at storeAbs: symlink preferred,
// hard link otherwise. There is no copy fallback — storey never copies
// file content; when both links fail it returns an error and the caller
// logs the missing link and carries on.
func CreateLink(storeAbs, linkAbs string) (string, error) {
	rel, err := filepath.Rel(filepath.Dir(linkAbs), storeAbs)
	if err != nil {
		rel = storeAbs
	}
	if err := os.Symlink(rel, linkAbs); err == nil {
		return "symlink", nil
	}
	if err := os.Link(storeAbs, linkAbs); err == nil {
		return "hardlink", nil
	} else {
		return "", err
	}
}
