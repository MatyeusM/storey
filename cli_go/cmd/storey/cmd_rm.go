package main

import (
	"fmt"
	"os"
	"path"
	"path/filepath"

	"github.com/matyeusm/storey/internal/index"
	"github.com/matyeusm/storey/internal/log"
	"github.com/matyeusm/storey/internal/views"
)

// rmData is the --json data payload for rm.
type rmData struct {
	Removed string `json:"removed"`
}

// cmdRm implements `storey remove|rm`: delete the object's view links,
// then its file and index record. Permanent.
func cmdRm(args []string, asJSON bool, logger *log.Logger, emit func(any)) int {
	dry, target, exit := parseRmArgs(args)
	if exit != exitOK {
		return exit
	}
	root, ix, release, exit := loadLocked()
	if exit != exitOK {
		return exit
	}
	defer release()
	pos, exit := resolveTarget(root, ix, target, emit)
	if exit != exitOK {
		return exit
	}
	rec := ix.Files[pos]
	links := linksFor(ix, rec.ID)
	data := rmData{Removed: rec.ID}
	if dry {
		emit(data)
		if !asJSON {
			fmt.Printf("would remove %s (%s) and %d link(s)\n", rec.ID, rec.Location(), len(links))
		}
		return exitOK
	}
	if err := removeLinks(root, links); err != nil {
		return fail("%s", err)
	}
	abs := filepath.Join(root, filepath.FromSlash(rec.Location()))
	if _, err := os.Lstat(abs); err != nil {
		return fail("rm: %s: %s", rec.Location(), err)
	}
	if err := os.Remove(abs); err != nil {
		return fail("%s", err)
	}
	ix.Files = append(ix.Files[:pos], ix.Files[pos+1:]...)
	if err := ix.Save(root); err != nil {
		return fail("%s", err)
	}
	emit(data)
	if !asJSON {
		fmt.Printf("removed %s (%d link(s))\n", rec.ID, len(links))
	}
	return exitOK
}

func parseRmArgs(args []string) (dry bool, target string, exit int) {
	for _, a := range args {
		if a == "--dry-run" {
			dry = true
			continue
		}
		if target != "" {
			return false, "", usageErr("rm takes exactly one <target>")
		}
		target = a
	}
	if target == "" {
		return false, "", usageErr("rm takes exactly one <target>")
	}
	return dry, target, exitOK
}

// linksFor derives current view link paths for one id (nil when no view
// was ever materialized, so plain filenames are never misclassified).
func linksFor(ix *index.Index, id string) []string {
	if len(ix.View.By) == 0 {
		return nil
	}
	var out []string
	for _, l := range views.Plan(storedValues(ix.Files), ix.View.By) {
		if l.ID == id {
			out = append(out, l.ViewPath)
		}
	}
	return out
}

// removeLinks unlinks view paths and prunes newly-empty parent dirs.
func removeLinks(root string, links []string) error {
	for _, p := range links {
		abs := filepath.Join(root, filepath.FromSlash(p))
		if err := os.Remove(abs); err != nil && !os.IsNotExist(err) {
			return err
		}
		pruneParents(root, p)
	}
	return nil
}

// pruneParents removes empty ancestors of rel, stopping at the first
// non-empty dir (never above root).
func pruneParents(root, rel string) {
	for dir := path.Dir(rel); dir != "." && dir != ""; dir = path.Dir(dir) {
		if err := os.Remove(filepath.Join(root, filepath.FromSlash(dir))); err != nil {
			return
		}
	}
}
