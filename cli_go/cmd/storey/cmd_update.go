package main

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/matyeusm/storey/internal/adopt"
	"github.com/matyeusm/storey/internal/hash"
	"github.com/matyeusm/storey/internal/index"
	"github.com/matyeusm/storey/internal/library"
	"github.com/matyeusm/storey/internal/log"
)

// updateData is the --json data payload for update.
type updateData struct {
	Added      []string `json:"added"`
	Purged     []string `json:"purged"`
	Moved      []string `json:"moved"`
	Adopted    []string `json:"adopted"`
	Removed    []string `json:"removed"`
	Reverified int      `json:"reverified"`
	Problems   []string `json:"problems"`
}

// cmdUpdate implements `storey update`: fold in nested libraries, hash
// new files (recording, repointing, or purging dupes), and — only with
// --verify — re-hash every indexed path. Default is trusted: no
// reverification. Report-only: problems exit 1. Requires a library.
// --dry-run previews without adopting, writing, or deleting.
func cmdUpdate(args []string, asJSON bool, logger *log.Logger, emit func(any)) int {
	verify, dry, exit := parseUpdateArgs(args)
	if exit != exitOK {
		return exit
	}
	root, ix, release, exit := loadLocked()
	if exit != exitOK {
		return exit
	}
	defer release()
	if dry {
		return dryUpdate(root, ix, verify, logger, asJSON, emit)
	}
	return withEmergencySave(root, ix, func() int {
		return execUpdate(root, ix, verify, logger, asJSON, emit)
	})
}

// execUpdate folds nested libraries, hashes intake, verifies, and
// saves. It runs under the emergency saver: adopt and purge move or
// delete files, so an abort must persist the index.
func execUpdate(root string, ix *index.Index, verify bool, logger *log.Logger, asJSON bool, emit func(any)) int {
	data := updateData{Added: []string{}, Purged: []string{}, Moved: []string{}, Adopted: []string{}, Removed: []string{}, Problems: []string{}}
	merged := adopt.Merge(root, ix, logger)
	data.Adopted = append(data.Adopted, merged.Adopted...)
	data.Problems = append(data.Problems, merged.Problems...)
	data.Purged = append(data.Purged, merged.Purged...)
	candidates, exit := updateCandidates(root, ix, merged.Skipped)
	if exit != exitOK {
		return exit
	}
	byPath, hashed, problems, exit := hashRelPaths(root, candidates, logger, "hash")
	if exit != exitOK {
		return exit
	}
	data.Problems = append(data.Problems, problems...)
	phase1, err := fillIndex(ix, root, hashed, byPath, false)
	if err != nil {
		return fail("%s", err)
	}
	data.Added, data.Purged = phase1.Added, append(data.Purged, phase1.Purged...)
	data.Moved = append(data.Moved, phase1.Moved...)
	if verify {
		data.Problems = append(data.Problems, reconcileStore(root, ix)...)
		problems, reverified, removed := reverify(root, ix, logger)
		data.Problems, data.Reverified, data.Removed = append(data.Problems, problems...), reverified, removed
	}
	if exit := saveIndex(root, ix); exit != exitOK {
		return exit
	}
	return reportUpdate(data, asJSON, emit)
}

// reportUpdate emits the payload, prints the human summary, and maps
// leftover problems to exit 1.
func reportUpdate(data updateData, asJSON bool, emit func(any)) int {
	emit(data)
	if !asJSON {
		fmt.Printf("added: %d\nadopted: %d\npurged (duplicates): %d\nmoved: %d\nremoved (missing): %d\nreverified: %d\nproblems: %d\n",
			len(data.Added), len(data.Adopted), len(data.Purged), len(data.Moved), len(data.Removed), data.Reverified, len(data.Problems))
		for _, p := range data.Problems {
			fmt.Printf("  %s\n", p)
		}
	}
	if len(data.Problems) > 0 {
		return exitFailure
	}
	return exitOK
}

func parseUpdateArgs(args []string) (bool, bool, int) {
	verify, dry := false, false
	for _, a := range args {
		switch a {
		case "--verify":
			verify = true
		case "--dry-run":
			dry = true
		default:
			return false, false, usageErr("update: unknown argument %q", a)
		}
	}
	return verify, dry, exitOK
}

// dryUpdate previews intake on a throwaway copy: no adoption (it moves
// files), no index write, no deletes. Composes with --verify.
func dryUpdate(root string, ix *index.Index, verify bool, logger *log.Logger, asJSON bool, emit func(any)) int {
	data := updateData{Added: []string{}, Purged: []string{}, Moved: []string{}, Adopted: []string{}, Removed: []string{}, Problems: []string{}}
	candidates, exit := updateCandidates(root, ix, nil)
	if exit != exitOK {
		return exit
	}
	byPath, hashed, problems, exit := hashRelPaths(root, candidates, logger, "hash")
	if exit != exitOK {
		return exit
	}
	data.Problems = append(data.Problems, problems...)
	simIx := copyIndex(ix)
	sim, err := fillIndex(simIx, root, hashed, byPath, true)
	if err != nil {
		return fail("%s", err)
	}
	data.Added, data.Purged, data.Moved =
		append(data.Added, sim.Added...), append(data.Purged, sim.Purged...), append(data.Moved, sim.Moved...)
	if verify {
		data.Problems = append(data.Problems, reconcileStore(root, simIx)...)
		more, reverified, removed := reverify(root, simIx, logger)
		data.Problems, data.Reverified, data.Removed =
			append(data.Problems, more...), reverified, removed
	}
	emit(data)
	if !asJSON {
		fmt.Printf("update (dry run, adoption skipped)\nwould add: %d\nwould purge (duplicates): %d\nwould repoint (moved): %d\nremoved (missing): %d\nreverified: %d\nproblems: %d\n",
			len(data.Added), len(data.Purged), len(data.Moved), len(data.Removed), data.Reverified, len(data.Problems))
	}
	return exitOK
}

// updateCandidates lists on-disk files needing hashing: not at a tracked
// location, not a derived view artifact (hard links would otherwise be
// mistaken for new files), and not under a failed nested library (those
// stay shielded for repair).
func updateCandidates(root string, ix *index.Index, skipped []string) ([]string, int) {
	all, err := library.Walk(root)
	if err != nil {
		return nil, fail("%s", err)
	}
	known := library.CurrentLocations(ix.Files)
	artifacts := planSet(ix.Files, ix.View.By)
	var candidates []string
	for _, p := range all {
		if !known[p] && !artifacts[p] && !adopt.Shielded(p, skipped) {
			candidates = append(candidates, p)
		}
	}
	return candidates, exitOK
}

// reverify re-hashes every indexed path with progress. Changed files
// are problems; missing files are pruned from the index at the end
// (the user may have renamed or deleted them — the entry is dropped
// instead of reported). Nothing else is repaired.
func reverify(root string, ix *index.Index, logger *log.Logger) ([]string, int, []string) {
	var problems []string
	var missing []int
	reverified := 0
	for i := range ix.Files {
		rec := &ix.Files[i]
		h, _, err := hash.File(filepath.Join(root, filepath.FromSlash(rec.Location())))
		logger.Progress("verify", i+1, len(ix.Files), rec.Location())
		switch {
		case os.IsNotExist(err):
			missing = append(missing, i)
		case err != nil:
			problems = append(problems, rec.Location()+": "+err.Error())
		case h != rec.ID:
			problems = append(problems, rec.Location()+": content changed")
		default:
			reverified++
		}
	}
	return problems, reverified, pruneMissing(ix, missing)
}

// pruneMissing drops index records in index order, returning their ids.
func pruneMissing(ix *index.Index, missing []int) []string {
	removed := make([]string, 0, len(missing))
	for _, i := range missing {
		removed = append(removed, ix.Files[i].ID)
	}
	for k := len(missing) - 1; k >= 0; k-- {
		i := missing[k]
		ix.Files = append(ix.Files[:i], ix.Files[i+1:]...)
	}
	return removed
}
