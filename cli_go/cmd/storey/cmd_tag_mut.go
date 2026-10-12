package main

import (
	"fmt"

	"github.com/matyeusm/storey/internal/index"
)

// cmdTagWrite implements tag set (replace) and tag add (merge).
func cmdTagWrite(args []string, asJSON bool, emit func(any), merge bool) int {
	cat, raw, pos, exit := parseCatVals(args)
	if exit != exitOK {
		return exit
	}
	if len(pos) != 1 {
		return usageErr("tag set/add takes exactly one <target>")
	}
	if cat == "" || raw == "" {
		return usageErr("tag set/add: --category and --values are both required")
	}
	if err := checkName(cat); err != nil {
		return usageErr("tag: %s", err)
	}
	vals, err := splitValues(raw)
	if err != nil {
		return usageErr("tag: %s", err)
	}
	root, ix, release, exit := loadLocked()
	if exit != exitOK {
		return exit
	}
	defer release()
	idx, exit := resolveTarget(root, ix, pos[0], emit)
	if exit != exitOK {
		return exit
	}
	rec := &ix.Files[idx]
	if rec.Tags == nil {
		rec.Tags = map[string][]string{}
	}
	if merge {
		for _, v := range vals {
			if !contains(rec.Tags[cat], v) {
				rec.Tags[cat] = append(rec.Tags[cat], v)
			}
		}
	} else {
		rec.Tags[cat] = vals
	}
	if exit := saveIndex(root, ix); exit != exitOK {
		return exit
	}
	emit(*rec)
	printWriteResult(asJSON, rec, cat)
	return exitOK
}

// cmdTagRemove drops values, a category, or all tags by arity.
func cmdTagRemove(args []string, asJSON bool, emit func(any)) int {
	cat, raw, pos, exit := parseCatVals(args)
	if exit != exitOK {
		return exit
	}
	if len(pos) != 1 {
		return usageErr("tag remove takes exactly one <target>")
	}
	if raw != "" && cat == "" {
		return usageErr("tag remove: --values needs --category")
	}
	root, ix, release, exit := loadLocked()
	if exit != exitOK {
		return exit
	}
	defer release()
	idx, exit := resolveTarget(root, ix, pos[0], emit)
	if exit != exitOK {
		return exit
	}
	rec := &ix.Files[idx]
	if exit := applyRemove(rec, cat, raw); exit != exitOK {
		return exit
	}
	if exit := saveIndex(root, ix); exit != exitOK {
		return exit
	}
	emit(*rec)
	printRemoveResult(asJSON, rec, cat)
	return exitOK
}

// applyRemove mutates rec: values, category, or everything.
func applyRemove(rec *index.Record, cat, raw string) int {
	if cat == "" {
		rec.Tags = map[string][]string{}
		return exitOK
	}
	if err := checkName(cat); err != nil {
		return usageErr("tag remove: %s", err)
	}
	if raw == "" {
		delete(rec.Tags, cat)
		return exitOK
	}
	vals, err := splitValues(raw)
	if err != nil {
		return usageErr("tag remove: %s", err)
	}
	keep := rec.Tags[cat][:0:0]
	for _, v := range rec.Tags[cat] {
		if !contains(vals, v) {
			keep = append(keep, v)
		}
	}
	if len(keep) == 0 {
		delete(rec.Tags, cat)
	} else {
		rec.Tags[cat] = keep
	}
	return exitOK
}

func printWriteResult(asJSON bool, rec *index.Record, cat string) {
	if asJSON {
		return
	}
	fmt.Printf("%s: %s=%v\n", rec.ID, cat, rec.Tags[cat])
}

func printRemoveResult(asJSON bool, rec *index.Record, cat string) {
	if asJSON {
		return
	}
	if cat == "" {
		fmt.Printf("removed all tags from %s\n", rec.ID)
		return
	}
	fmt.Printf("%s: %s=%v\n", rec.ID, cat, rec.Tags[cat])
}
