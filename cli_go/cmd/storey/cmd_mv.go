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

// mvData is the --json data payload for mv.
type mvData struct {
	ID  string `json:"id"`
	Old string `json:"old"`
	New string `json:"new"`
}

// cmdMv implements `storey mv`: rename the display name, the file, and
// all view links. Lossless: nothing is deleted.
func cmdMv(args []string, asJSON bool, logger *log.Logger, emit func(any)) int {
	dry, target, newName, exit := parseMvArgs(args)
	if exit != exitOK {
		return exit
	}
	if err := checkName(newName); err != nil {
		return usageErr("mv: %s", err)
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
	rec := &ix.Files[pos]
	data := mvData{ID: rec.ID, Old: rec.DisplayName, New: newName}
	if newName == rec.DisplayName {
		emit(data)
		if !asJSON {
			fmt.Println("name unchanged")
		}
		return exitOK
	}
	oldLinks := linksFor(ix, rec.ID)
	plan, exit := planRename(root, ix, pos, newName)
	if exit != exitOK {
		return exit
	}
	data.New = plan.newName
	if dry {
		return dryRename(data, oldLinks, asJSON, emit)
	}
	if err := applyRename(root, ix, pos, plan, oldLinks); err != nil {
		return fail("%s", err)
	}
	if exit := saveIndex(root, ix); exit != exitOK {
		return exit
	}
	emit(data)
	if !asJSON {
		fmt.Printf("renamed %s -> %s\n", data.Old, data.New)
	}
	return exitOK
}

// dryRename previews a rename without touching files or the index.
func dryRename(data mvData, oldLinks []string, asJSON bool, emit func(any)) int {
	emit(data)
	if !asJSON {
		fmt.Printf("would rename %s -> %s (%d link(s))\n", data.Old, data.New, len(oldLinks))
	}
	return exitOK
}

func parseMvArgs(args []string) (dry bool, target, newName string, exit int) {
	var positionals []string
	for _, a := range args {
		if a == "--dry-run" {
			dry = true
			continue
		}
		positionals = append(positionals, a)
	}
	if len(positionals) != 2 {
		return false, "", "", usageErr("mv takes <target> <new-name>")
	}
	return dry, positionals[0], positionals[1], exitOK
}

// renamePlan describes the file move for mv.
type renamePlan struct {
	src     string // slash-rel current location
	dst     string // slash-rel destination
	newName string // final display name (after _N fallback)
}

// planRename computes the destination. Stored objects stay in .files/
// with _N fallback on collision; in-place files keep their directory and
// abort when the destination exists.
func planRename(root string, ix *index.Index, pos int, newName string) (renamePlan, int) {
	rec := &ix.Files[pos]
	if !rec.Stored() {
		dst := joinDir(path.Dir(rec.OriginalRel), newName)
		if _, err := os.Lstat(filepath.Join(root, filepath.FromSlash(dst))); err == nil {
			return renamePlan{}, usageErr("mv: %s already exists", dst)
		}
		return renamePlan{src: rec.Location(), dst: dst, newName: newName}, exitOK
	}
	taken := map[string]string{}
	for i := range ix.Files {
		other := &ix.Files[i]
		if i != pos && other.Stored() {
			taken[filepath.Base(other.StoredRel)] = other.ID
		}
	}
	final := views.StoreName(newName, taken, rec.ID)
	return renamePlan{src: rec.Location(), dst: index.StoreDir + "/" + final, newName: final}, exitOK
}

func joinDir(dir, name string) string {
	if dir == "." || dir == "" {
		return name
	}
	return dir + "/" + name
}

// applyRename removes old links, moves the file, updates the record, and
// rebuilds links (stored records only).
func applyRename(root string, ix *index.Index, pos int, plan renamePlan, oldLinks []string) error {
	rec := &ix.Files[pos]
	if err := removeLinks(root, oldLinks); err != nil {
		return err
	}
	srcAbs := filepath.Join(root, filepath.FromSlash(plan.src))
	dstAbs := filepath.Join(root, filepath.FromSlash(plan.dst))
	if err := os.Rename(srcAbs, dstAbs); err != nil {
		return err
	}
	rec.DisplayName = plan.newName
	if rec.Stored() {
		rec.StoredRel = plan.dst
	} else {
		rec.OriginalRel = plan.dst
	}
	if !rec.Stored() {
		return nil
	}
	for _, p := range linksFor(ix, rec.ID) {
		linkAbs := filepath.Join(root, filepath.FromSlash(p))
		if err := os.MkdirAll(filepath.Dir(linkAbs), 0o755); err != nil {
			return err
		}
		storeAbs := filepath.Join(root, filepath.FromSlash(rec.StoredRel))
		if _, err := views.CreateLink(storeAbs, linkAbs); err != nil {
			return fmt.Errorf("%s: %w", p, err)
		}
	}
	return nil
}
