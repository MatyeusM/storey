package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/matyeusm/storey/internal/index"
	"github.com/matyeusm/storey/internal/log"
	"github.com/matyeusm/storey/internal/views"
)

// viewData is the --json data payload for view.
type viewData struct {
	By         []string          `json:"by"`
	Moved      int               `json:"moved"`
	Purged     []string          `json:"purged"`
	Untracked  []string          `json:"untracked"`
	Orphans    []string          `json:"orphans"`
	Links      int               `json:"links"`
	Unsorted   int               `json:"unsorted"`
	LinkErrors []string          `json:"linkErrors,omitempty"`
	Plan       []string          `json:"plan,omitempty"`
	Moves      map[string]string `json:"moves,omitempty"`
}

// cmdView implements `storey view`: ingest indexed files into .files/,
// wipe everything else non-system, rebuild links, record the key order.
func cmdView(args []string, asJSON bool, logger *log.Logger, emit func(any)) int {
	by, force, dry, exit := parseViewArgs(args)
	if exit != exitOK {
		return exit
	}
	root, ix, release, exit := loadLocked()
	if exit != exitOK {
		return exit
	}
	defer release()
	data := viewData{By: by, Purged: []string{}, Untracked: []string{}, Orphans: []string{}}
	orphans := reconcileStore(root, ix)
	untracked, exit := findUntracked(root, ix)
	if exit != exitOK {
		return exit
	}
	data.Untracked = untracked
	data.Orphans = orphans
	if len(untracked) > 0 && !force {
		emit(data)
		reportBlocked("view", untracked, nil)
		return exitFailure
	}
	if len(orphans) > 0 && !force {
		emit(data)
		reportBlocked("view", untracked, nil)
		for _, o := range orphans {
			fmt.Fprintf(os.Stderr, "  orphaned store file: %s\n", o)
		}
		return exitFailure
	}
	if missing := missingLocations(root, ix); len(missing) > 0 {
		return fail("view: %d indexed file(s) missing: %s", len(missing), strings.Join(missing, ", "))
	}
	moves := computeMoves(ix)
	links := plannedLinks(ix, moves, by)
	data.Links, data.Unsorted = len(links), countUnsorted(links)
	if dry {
		return dryView(asJSON, emit, &data, moves, links, by)
	}
	return execView(root, ix, logger, asJSON, emit, &data, moves, links, by)
}

func dryView(asJSON bool, emit func(any), data *viewData, moves map[string]string, links []views.Link, by []string) int {
	data.Moves, data.Plan = moves, linkPaths(links)
	emit(*data)
	printViewSummary(asJSON, by, len(moves), 0, data.Links, 0, data.Unsorted, true)
	return exitOK
}

func execView(root string, ix *index.Index, logger *log.Logger, asJSON bool, emit func(any), data *viewData, moves map[string]string, links []views.Link, by []string) int {
	if err := os.MkdirAll(filepath.Join(root, index.StoreDir), 0o755); err != nil {
		return fail("%s", err)
	}
	return withEmergencySave(root, ix, func() int {
		if err := probeLinks(root); err != nil {
			return fail("%s", err)
		}
		moved, purged, err := applyMoves(root, ix, moves)
		if err != nil {
			return fail("%s", err)
		}
		data.Moved, data.Purged = moved, purged
		// Files have moved: persist before wiping and linking, so an
		// abort below (or a kill) leaves a loadable index behind.
		if err := ix.Save(root); err != nil {
			return fail("%s", err)
		}
		if err := wipeNonSystem(root); err != nil {
			return fail("%s", err)
		}
		made, linkErrors := createLinks(root, ix, links, logger)
		data.Links, data.LinkErrors, data.Unsorted = made, linkErrors, countUnsorted(links)
		ix.View.By = by
		if err := ix.Save(root); err != nil {
			return fail("%s", err)
		}
		emit(*data)
		printViewSummary(asJSON, by, moved, len(purged), made, len(linkErrors), data.Unsorted, false)
		return exitOK
	})
}

func parseViewArgs(args []string) (by []string, force, dry bool, exit int) {
	tagsArg := ""
	tagsGiven := false
	for i := 0; i < len(args); i++ {
		switch a := args[i]; {
		case a == "--force":
			force = true
		case a == "--dry-run":
			dry = true
		case a == "--tags" && i+1 < len(args):
			i++
			tagsArg, tagsGiven = args[i], true
		case strings.HasPrefix(a, "--tags="):
			tagsArg, tagsGiven = strings.TrimPrefix(a, "--tags="), true
		default:
			return nil, false, false, usageErr("view: unknown argument %q", a)
		}
	}
	if !tagsGiven {
		return nil, false, false, usageErr("view: --tags=<a,b,...> is required")
	}
	by = []string{}
	for _, c := range strings.Split(tagsArg, ",") {
		if c = strings.TrimSpace(c); c != "" {
			by = append(by, c)
		}
	}
	return by, force, dry, exitOK
}

// computeMoves maps current location -> store destination ("" = purge as
// duplicate) in index order; first occurrence of a hash wins the move.
func computeMoves(ix *index.Index) map[string]string {
	taken := map[string]string{}
	storedIDs := map[string]bool{}
	for _, rec := range ix.Files {
		if rec.Stored() {
			taken[filepath.Base(rec.StoredRel)] = rec.ID
			storedIDs[rec.ID] = true
		}
	}
	moves := map[string]string{}
	for i := range ix.Files {
		rec := &ix.Files[i]
		if rec.Stored() {
			continue
		}
		if storedIDs[rec.ID] {
			moves[rec.Location()] = ""
			continue
		}
		name := views.StoreName(rec.DisplayName, taken, rec.ID)
		taken[name] = rec.ID
		storedIDs[rec.ID] = true
		moves[rec.Location()] = index.StoreDir + "/" + name
	}
	return moves
}

func applyMoves(root string, ix *index.Index, moves map[string]string) (int, []string, error) {
	moved := 0
	purged := []string{}
	for i := range ix.Files {
		rec := &ix.Files[i]
		dst, ok := moves[rec.Location()]
		if !ok {
			continue
		}
		srcAbs := filepath.Join(root, filepath.FromSlash(rec.Location()))
		if dst == "" {
			if err := os.Remove(srcAbs); err != nil {
				return moved, purged, err
			}
			purged = append(purged, rec.ID)
			continue
		}
		if err := os.Rename(srcAbs, filepath.Join(root, filepath.FromSlash(dst))); err != nil {
			return moved, purged, err
		}
		rec.StoredRel = dst
		moved++
	}
	return moved, purged, nil
}

func createLinks(root string, ix *index.Index, links []views.Link, logger *log.Logger) (int, []string) {
	made := 0
	var failed []string
	byID := make(map[string]*index.Record, len(ix.Files))
	for i := range ix.Files {
		byID[ix.Files[i].ID] = &ix.Files[i]
	}
	for _, l := range links {
		rec := byID[l.ID]
		if rec == nil {
			failed = append(failed, l.ViewPath+": unknown id "+l.ID)
			logger.Warnf("view: %s: unknown id %s", l.ViewPath, l.ID)
			continue
		}
		linkAbs := filepath.Join(root, filepath.FromSlash(l.ViewPath))
		if err := os.MkdirAll(filepath.Dir(linkAbs), 0o755); err != nil {
			failed = append(failed, l.ViewPath+": "+err.Error())
			logger.Warnf("view: %s: %s", l.ViewPath, err)
			continue
		}
		storeAbs := filepath.Join(root, filepath.FromSlash(rec.StoredRel))
		if _, err := views.CreateLink(storeAbs, linkAbs); err != nil {
			failed = append(failed, l.ViewPath+": "+err.Error())
			logger.Warnf("view: cannot link %s: %s", l.ViewPath, err)
			continue
		}
		made++
	}
	return made, failed
}

// plannedLinks previews post-ingest view links: unstored records with a
// move destination count as stored, purged dupes are dropped. Empty `by`
// renders an empty directory: no links, everything stays stored.
func plannedLinks(ix *index.Index, moves map[string]string, by []string) []views.Link {
	if len(by) == 0 {
		return nil
	}
	recs := make([]index.Record, 0, len(ix.Files))
	for i := range ix.Files {
		rec := ix.Files[i]
		if dst, ok := moves[rec.Location()]; ok {
			if dst == "" {
				continue
			}
			rec.StoredRel = dst
		}
		if rec.Stored() {
			recs = append(recs, rec)
		}
	}
	return views.Plan(recs, by)
}

// planSet returns derived view link paths as a set (empty when no view
// was ever materialized, so plain filenames are never misclassified).
func planSet(files []index.Record, by []string) map[string]bool {
	set := map[string]bool{}
	if len(by) == 0 {
		return set
	}
	for _, l := range views.Plan(storedValues(files), by) {
		set[l.ViewPath] = true
	}
	return set
}

// wipeNonSystem removes every entry in root except .files/,
// .index.json, and the write lock. Symlinks are unlinked, never followed.
func wipeNonSystem(root string) error {
	entries, err := os.ReadDir(root)
	if err != nil {
		return err
	}
	for _, e := range entries {
		name := e.Name()
		if name == index.StoreDir || name == index.FileName || name == index.LockName {
			continue
		}
		if err := os.RemoveAll(filepath.Join(root, name)); err != nil {
			return err
		}
	}
	return nil
}
