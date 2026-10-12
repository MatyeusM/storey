package main

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/matyeusm/storey/internal/index"
	"github.com/matyeusm/storey/internal/log"
)

// restoreData is the --json data payload for restore.
type restoreData struct {
	Restored  int               `json:"restored"`
	Skipped   int               `json:"skipped"`
	Conflicts []string          `json:"conflicts"`
	Problems  []string          `json:"problems"`
	Untracked []string          `json:"untracked"`
	Moves     map[string]string `json:"moves,omitempty"`
}

// cmdRestore implements `storey restore`: remove view artifacts and move
// stored objects back to their original relative paths.
func cmdRestore(args []string, asJSON bool, logger *log.Logger, emit func(any)) int {
	force, dry, exit := parseRestoreArgs(args)
	if exit != exitOK {
		return exit
	}
	root, ix, release, exit := loadLocked()
	if exit != exitOK {
		return exit
	}
	defer release()
	data := restoreData{Conflicts: []string{}, Problems: []string{}, Untracked: []string{}}
	stored := storedRecords(ix)
	data.Skipped = len(ix.Files) - len(stored)
	if len(stored) == 0 {
		emit(data)
		if !asJSON {
			fmt.Println("nothing to restore")
		}
		return exitOK
	}
	untracked, exit := findUntracked(root, ix)
	if exit != exitOK {
		return exit
	}
	data.Untracked = untracked
	data.Conflicts = findConflicts(root, stored)
	if (len(untracked) > 0 || len(data.Conflicts) > 0) && !force {
		emit(data)
		reportBlocked("restore", untracked, data.Conflicts)
		return exitFailure
	}
	if problems := missingStored(root, stored); len(problems) > 0 {
		data.Problems = problems
		emit(data)
		return fail("restore: %d stored file(s) missing: %s", len(problems), strings.Join(problems, ", "))
	}
	moves := restoreMoves(stored)
	if dry {
		return dryRestore(asJSON, emit, &data, moves)
	}
	return execRestore(root, ix, asJSON, emit, &data, stored)
}

func dryRestore(asJSON bool, emit func(any), data *restoreData, moves map[string]string) int {
	data.Moves = moves
	emit(*data)
	if !asJSON {
		fmt.Printf("restore (dry run)\nmove back: %d\nconflicts: %d\n", len(moves), len(data.Conflicts))
	}
	return exitOK
}

func execRestore(root string, ix *index.Index, asJSON bool, emit func(any), data *restoreData, stored []*index.Record) int {
	return withEmergencySave(root, ix, func() int {
		if err := clearViews(root, keepInPlace(ix)); err != nil {
			return fail("%s", err)
		}
		restored, err := moveBack(root, stored)
		if err != nil {
			return fail("%s", err)
		}
		data.Restored = restored
		ix.View.By = []string{}
		if err := ix.Save(root); err != nil {
			return fail("%s", err)
		}
		emit(*data)
		if !asJSON {
			fmt.Printf("restored: %d\nskipped (in place): %d\n", data.Restored, data.Skipped)
		}
		return exitOK
	})
}

func parseRestoreArgs(args []string) (force, dry bool, exit int) {
	for _, a := range args {
		switch a {
		case "--force":
			force = true
		case "--dry-run":
			dry = true
		default:
			return false, false, usageErr("restore: unknown argument %q", a)
		}
	}
	return force, dry, exitOK
}

func storedRecords(ix *index.Index) []*index.Record {
	var stored []*index.Record
	for i := range ix.Files {
		if ix.Files[i].Stored() {
			stored = append(stored, &ix.Files[i])
		}
	}
	return stored
}

func findConflicts(root string, stored []*index.Record) []string {
	var conflicts []string
	for _, rec := range stored {
		abs := filepath.Join(root, filepath.FromSlash(rec.OriginalRel))
		if _, err := os.Lstat(abs); err == nil {
			conflicts = append(conflicts, rec.OriginalRel)
		}
	}
	return conflicts
}

func missingStored(root string, stored []*index.Record) []string {
	var problems []string
	for _, rec := range stored {
		abs := filepath.Join(root, filepath.FromSlash(rec.StoredRel))
		if _, err := os.Lstat(abs); err != nil {
			problems = append(problems, rec.StoredRel)
		}
	}
	return problems
}

func restoreMoves(stored []*index.Record) map[string]string {
	moves := map[string]string{}
	for _, rec := range stored {
		moves[rec.StoredRel] = rec.OriginalRel
	}
	return moves
}

// keepInPlace returns original paths of records never ingested.
func keepInPlace(ix *index.Index) map[string]bool {
	keep := map[string]bool{}
	for i := range ix.Files {
		if !ix.Files[i].Stored() {
			keep[ix.Files[i].OriginalRel] = true
		}
	}
	return keep
}

func moveBack(root string, stored []*index.Record) (int, error) {
	restored := 0
	for _, rec := range stored {
		srcAbs := filepath.Join(root, filepath.FromSlash(rec.StoredRel))
		dstAbs := filepath.Join(root, filepath.FromSlash(rec.OriginalRel))
		if _, err := os.Lstat(dstAbs); err == nil {
			return restored, fmt.Errorf("restore: %s appeared during restore", rec.OriginalRel)
		}
		if err := os.MkdirAll(filepath.Dir(dstAbs), 0o755); err != nil {
			return restored, err
		}
		if err := os.Rename(srcAbs, dstAbs); err != nil {
			return restored, err
		}
		rec.StoredRel = ""
		restored++
	}
	return restored, nil
}

// clearViews removes every non-system entry except kept files
// (slash-rel). Symlinks are unlinked, never followed.
func clearViews(root string, keep map[string]bool) error {
	files, dirs, err := collectEntries(root, keep)
	if err != nil {
		return err
	}
	for _, f := range files {
		if err := os.RemoveAll(filepath.Join(root, filepath.FromSlash(f))); err != nil {
			return err
		}
	}
	if err := removeEmptyDirs(root, dirs, keepDirSet(keep)); err != nil {
		return err
	}
	return nil
}

// collectEntries gathers deletable files and all non-system directories.
func collectEntries(root string, keep map[string]bool) (files, dirs []string, err error) {
	walkErr := filepath.WalkDir(root, func(p string, d fs.DirEntry, err error) error {
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
		rel = filepath.ToSlash(rel)
		if rel == index.StoreDir || strings.HasPrefix(rel, index.StoreDir+"/") {
			return filepath.SkipDir
		}
		if rel == index.FileName || rel == index.LockName {
			return nil
		}
		if d.IsDir() && d.Type()&fs.ModeSymlink == 0 {
			dirs = append(dirs, rel)
			return nil
		}
		if keep[rel] {
			return nil
		}
		files = append(files, rel)
		// NOTE: plain nil here, never SkipDir — on a file, SkipDir
		// skips the remaining siblings in this directory.
		return nil
	})
	return files, dirs, walkErr
}

// keepDirSet returns root plus every ancestor dir of kept files.
func keepDirSet(keep map[string]bool) map[string]bool {
	set := map[string]bool{"": true}
	for p := range keep {
		for dir := p; ; {
			i := strings.LastIndex(dir, "/")
			if i < 0 {
				break
			}
			dir = dir[:i]
			set[dir] = true
		}
	}
	return set
}

// removeEmptyDirs removes non-kept dirs deepest-first; leftovers are errors.
func removeEmptyDirs(root string, dirs []string, keepDirs map[string]bool) error {
	sort.Slice(dirs, func(i, j int) bool { return len(dirs[i]) > len(dirs[j]) })
	for _, d := range dirs {
		if keepDirs[d] {
			continue
		}
		abs := filepath.Join(root, filepath.FromSlash(d))
		if err := os.Remove(abs); err != nil && !os.IsNotExist(err) {
			if entries, rerr := os.ReadDir(abs); rerr != nil || len(entries) > 0 {
				return fmt.Errorf("restore: cannot clear %s: %w", d, err)
			}
		}
	}
	return nil
}
