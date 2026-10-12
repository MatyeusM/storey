package main

import (
	"fmt"
	"os"

	"github.com/matyeusm/storey/internal/adopt"
	"github.com/matyeusm/storey/internal/hash"
	"github.com/matyeusm/storey/internal/index"
	"github.com/matyeusm/storey/internal/library"
	"github.com/matyeusm/storey/internal/log"
)

// initData is the --json data payload for init.
type initData struct {
	Added    []string `json:"added"`
	Purged   []string `json:"purged"`
	Moved    []string `json:"moved"`
	Adopted  []string `json:"adopted"`
	Problems []string `json:"problems"`
}

// cmdInit implements `storey init`: create .index.json in cwd, fold in
// nested libraries, then hash and record everything in place. Nothing
// moves into .files/ until `view`. Accepted only with no index present.
func cmdInit(args []string, asJSON bool, logger *log.Logger, emit func(any)) int {
	dry, exit := parseInitArgs(args)
	if exit != exitOK {
		return exit
	}
	cwd, err := os.Getwd()
	if err != nil {
		return fail("%s", err)
	}
	if _, err := library.FindRoot(cwd); err == nil {
		return fail("already inside a library")
	}
	release, exit := lockLibrary(cwd)
	if exit != exitOK {
		return exit
	}
	defer release()
	ix := index.New()
	data := initData{Added: []string{}, Purged: []string{}, Moved: []string{}, Adopted: []string{}, Problems: []string{}}
	if dry {
		return dryInit(cwd, ix, logger, asJSON, emit)
	}
	return withEmergencySave(cwd, ix, func() int {
		return execInit(cwd, ix, logger, asJSON, emit, data)
	})
}

// execInit adopts, hashes, fills, and saves. It runs under the
// emergency saver: a partial index beats re-adopting from scratch
// (re-run `update` to continue after an abort).
func execInit(root string, ix *index.Index, logger *log.Logger, asJSON bool, emit func(any), data initData) int {
	merged := adoptInto(ix, root, logger, &data)
	paths, byPath, problems, exit := collectHashed(root, ix, merged, logger)
	if exit != exitOK {
		return exit
	}
	data.Problems = append(data.Problems, problems...)
	scanned, err := fillIndex(ix, root, paths, byPath, false)
	if err != nil {
		return fail("%s", err)
	}
	data.Added, data.Purged = append(data.Added, scanned.Added...), append(data.Purged, scanned.Purged...)
	data.Moved = append(data.Moved, scanned.Moved...)
	if exit := saveIndex(root, ix); exit != exitOK {
		return exit
	}
	emit(data)
	if !asJSON {
		fmt.Printf("initialized library at %s\nadded: %d\nadopted: %d\npurged (duplicates): %d\nmoved: %d\nproblems: %d\n",
			root, len(data.Added), len(data.Adopted), len(data.Purged), len(data.Moved), len(data.Problems))
		for _, p := range data.Problems {
			fmt.Printf("  %s\n", p)
		}
	}
	if len(data.Problems) > 0 {
		return exitFailure
	}
	return exitOK
}

// adoptInto folds nested libraries into ix, recording the outcome and
// returning the shielded sub-index list for the file scan.
func adoptInto(ix *index.Index, root string, logger *log.Logger, data *initData) []string {
	merged := adopt.Merge(root, ix, logger)
	data.Adopted = append(data.Adopted, merged.Adopted...)
	data.Problems = append(data.Problems, merged.Problems...)
	data.Purged = append(data.Purged, merged.Purged...)
	return merged.Skipped
}

func parseInitArgs(args []string) (bool, int) {
	dry := false
	for _, a := range args {
		if a == "--dry-run" {
			dry = true
			continue
		}
		return false, usageErr("init takes no arguments (only --dry-run)")
	}
	return dry, exitOK
}

// dryInit previews intake on a throwaway copy: no adoption (it moves
// files), no index write, no deletes.
func dryInit(root string, ix *index.Index, logger *log.Logger, asJSON bool, emit func(any)) int {
	data := initData{Added: []string{}, Purged: []string{}, Moved: []string{}, Adopted: []string{}, Problems: []string{}}
	paths, byPath, problems, exit := collectHashed(root, ix, nil, logger)
	if exit != exitOK {
		return exit
	}
	data.Problems = append(data.Problems, problems...)
	sim, err := fillIndex(copyIndex(ix), root, paths, byPath, true)
	if err != nil {
		return fail("%s", err)
	}
	data.Added, data.Purged, data.Moved =
		append(data.Added, sim.Added...), append(data.Purged, sim.Purged...), append(data.Moved, sim.Moved...)
	emit(data)
	if !asJSON {
		fmt.Printf("init (dry run, adoption skipped)\nwould add: %d\nwould purge (duplicates): %d\nwould repoint (moved): %d\nproblems: %d\n",
			len(data.Added), len(data.Purged), len(data.Moved), len(data.Problems))
	}
	return exitOK
}

// collectHashed walks root and hashes every untracked file with the
// worker pool, reporting progress through logger. Files under failed
// (shielded) sub-indexes are left alone. Unreadable files are
// reported, never fatal.
func collectHashed(root string, ix *index.Index, skipped []string, logger *log.Logger) ([]string, map[string]hash.Result, []string, int) {
	paths, err := library.Walk(root)
	if err != nil {
		return nil, nil, nil, fail("%s", err)
	}
	known := library.CurrentLocations(ix.Files)
	fresh := paths[:0:0]
	for _, p := range paths {
		if !known[p] && !adopt.Shielded(p, skipped) {
			fresh = append(fresh, p)
		}
	}
	byPath, hashed, problems, exit := hashRelPaths(root, fresh, logger, "hash")
	if exit != exitOK {
		return nil, nil, nil, exit
	}
	return hashed, byPath, problems, exitOK
}

// fillIndex records paths in sorted order; first hash wins. A later
// path with a known hash is a duplicate and deleted from disk —
// unless the indexed file is gone, which means the content moved:
// repoint the record (keeping tags) instead of deleting anything.
// Under dry, nothing is deleted or repointed: classification only,
// so callers may pass a throwaway copy.
func fillIndex(ix *index.Index, root string, paths []string, byPath map[string]hash.Result, dry bool) (initData, error) {
	data := initData{Added: []string{}, Purged: []string{}, Moved: []string{}}
	byHash := make(map[string]int, len(ix.Files))
	for i := range ix.Files {
		byHash[ix.Files[i].ID] = i
	}
	for _, rel := range paths {
		r := byPath[rel]
		if j, ok := byHash[r.Hash]; ok {
			dup := &ix.Files[j]
			if !onDisk(root, dup.Location()) {
				dup.OriginalRel = rel
				dup.DisplayName = baseName(rel)
				dup.StoredRel = ""
				dup.Size = r.Size
				data.Moved = append(data.Moved, dup.ID)
				continue
			}
			if !dry {
				if err := os.Remove(rootJoin(root, rel)); err != nil {
					return data, err
				}
			}
			data.Purged = append(data.Purged, r.Hash)
			continue
		}
		ix.Files = append(ix.Files, index.Record{
			ID:          r.Hash,
			DisplayName: baseName(rel),
			OriginalRel: rel,
			Size:        r.Size,
			Tags:        map[string][]string{},
		})
		byHash[r.Hash] = len(ix.Files) - 1
		data.Added = append(data.Added, r.Hash)
	}
	return data, nil
}

// copyIndex shallow-copies the file list for dry-run simulation:
// fillIndex may repoint and append on the copy, never the live index.
func copyIndex(ix *index.Index) *index.Index {
	dup := *ix
	dup.Files = append([]index.Record(nil), ix.Files...)
	return &dup
}

// onDisk reports whether rel exists under root. Any error counts as
// gone (never delete on doubt): a dangling link repoints rather than
// purges the only good copy.
func onDisk(root, rel string) bool {
	_, err := os.Stat(rootJoin(root, rel))
	return err == nil
}

func saveIndex(root string, ix *index.Index) int {
	if err := ix.Save(root); err != nil {
		return fail("%s", err)
	}
	return exitOK
}
