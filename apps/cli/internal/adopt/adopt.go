// Package adopt folds nested storey libraries into their parent: any
// subdirectory holding its own .index.json is hash-verified record by
// record, merged into the parent index with tags and names intact, and
// its .index.json removed. Duplicates (same content, different path)
// are purged from disk. Storey only moves and deletes here — never
// copies.
package adopt

import (
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/matyeusm/storey/internal/hash"
	"github.com/matyeusm/storey/internal/index"
	"github.com/matyeusm/storey/internal/library"
	"github.com/matyeusm/storey/internal/log"
)

// Result tallies one merge pass.
type Result struct {
	Indexes  int      // sub-indexes fully adopted and removed
	Adopted  []string // adopted record ids
	Purged   []string // duplicate ids deleted from disk
	Problems []string // verification failures, left in place
	Skipped  []string // sub-index dirs with problems (files stay untouched)
}

// Merge folds every nested library under root into ix (root itself is
// excluded, as is anything inside .files/). A sub-index is deleted
// only when all of its records adopted cleanly.
func Merge(root string, ix *index.Index, logger *log.Logger) Result {
	var res Result
	for _, sub := range findSubIndexes(root) {
		mergeOne(root, sub, ix, logger, &res)
	}
	return res
}

// Shielded reports whether slash-relative path p sits inside a skipped
// (failed) sub-index dir. Shielded files stay untouched so the user can
// repair the nested library instead of losing its metadata to a fresh,
// untagged scan.
func Shielded(p string, skipped []string) bool {
	for _, s := range skipped {
		if p == s || strings.HasPrefix(p, s+"/") {
			return true
		}
	}
	return false
}

// findSubIndexes lists slash-relative dirs holding .index.json,
// deepest first so nested libraries merge before their parents.
func findSubIndexes(root string) []string {
	var dirs []string
	_ = filepath.WalkDir(root, func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if !d.IsDir() {
			return nil
		}
		rel, err := filepath.Rel(root, p)
		if err != nil {
			return err
		}
		if rel == "." {
			return nil
		}
		if d.Name() == index.StoreDir {
			return filepath.SkipDir
		}
		if library.HasIndexAt(p) {
			dirs = append(dirs, filepath.ToSlash(rel))
		}
		return nil
	})
	sort.Slice(dirs, func(i, j int) bool {
		if n := strings.Count(dirs[i], "/") - strings.Count(dirs[j], "/"); n != 0 {
			return n > 0
		}
		return dirs[i] < dirs[j]
	})
	return dirs
}

// mergeOne verifies and folds a single sub-index into ix.
func mergeOne(root, sub string, ix *index.Index, logger *log.Logger, res *Result) {
	subAbs := filepath.Join(root, filepath.FromSlash(sub))
	subIx, err := index.Load(subAbs)
	if err != nil {
		res.Problems = append(res.Problems, sub+"/.index.json: "+err.Error())
		res.Skipped = append(res.Skipped, sub)
		return
	}
	byLoc := hashSubRecords(subAbs, subIx, logger)
	failed := false
	for i := range subIx.Files {
		if !adoptRecord(root, sub, subAbs, &subIx.Files[i], byLoc, ix, res) {
			failed = true
		}
	}
	if failed {
		res.Skipped = append(res.Skipped, sub)
		return
	}
	if err := os.Remove(filepath.Join(subAbs, index.FileName)); err != nil {
		res.Problems = append(res.Problems, sub+"/.index.json: "+err.Error())
		return
	}
	res.Indexes++
	_ = os.Remove(filepath.Join(subAbs, index.StoreDir))
	logger.Infof("adopted nested library %s", sub)
}

// hashSubRecords hashes every indexed location of a sub-library,
// keyed by slash-relative path inside the sub-library.
func hashSubRecords(subAbs string, subIx *index.Index, logger *log.Logger) map[string]hash.Result {
	rels := make([]string, 0, len(subIx.Files))
	for i := range subIx.Files {
		rels = append(rels, subIx.Files[i].Location())
	}
	abs := make([]string, len(rels))
	for i, rel := range rels {
		abs[i] = filepath.Join(subAbs, filepath.FromSlash(rel))
	}
	out := map[string]hash.Result{}
	results := hash.Files(abs, hash.Workers(), func(done, total int, path string) {
		rel, _ := filepath.Rel(subAbs, path)
		logger.Progress("hash", done, total, filepath.ToSlash(rel))
	})
	for _, r := range results {
		rel, _ := filepath.Rel(subAbs, r.Path)
		out[filepath.ToSlash(rel)] = r
	}
	return out
}

// adoptRecord verifies one sub-library record and folds it into ix.
// It returns false when the record could not be adopted.
func adoptRecord(root, sub, subAbs string, rec *index.Record, byLoc map[string]hash.Result, ix *index.Index, res *Result) bool {
	if dup := ix.Find(rec.ID); dup != nil && movedEarlier(root, rec, dup) {
		return true
	}
	r, ok := byLoc[rec.Location()]
	loc := sub + "/" + rec.Location()
	if !ok || r.Err != nil {
		return problem(res, loc+": missing")
	}
	if r.Hash != rec.ID {
		return problem(res, loc+": content changed")
	}
	if dup := ix.Find(rec.ID); dup != nil {
		return adoptDuplicate(sub, subAbs, rec, dup, res)
	}
	if rec.Stored() {
		return adoptStored(root, sub, subAbs, rec, r.Size, ix, res)
	}
	for i := range ix.Files {
		if ix.Files[i].Location() == sub+"/"+rec.OriginalRel {
			return problem(res, loc+": path already indexed with different content")
		}
	}
	adoptInPlace(sub, rec, r.Size, ix, res)
	return true
}

// movedEarlier reports whether rec was adopted by a previous pass: a
// stored record whose content already lives at the parent location.
func movedEarlier(root string, rec, dup *index.Record) bool {
	if !rec.Stored() || !dup.Stored() {
		return false
	}
	_, err := os.Lstat(filepath.Join(root, filepath.FromSlash(dup.StoredRel)))
	return err == nil
}

func problem(res *Result, msg string) bool {
	res.Problems = append(res.Problems, msg)
	return false
}
