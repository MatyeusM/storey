package main

import (
	"fmt"
	"sort"

	"github.com/matyeusm/storey/internal/index"
)

// cmdTagGet implements tag get reads: categories, file tags, or values.
func cmdTagGet(args []string, asJSON bool, emit func(any)) int {
	cat, raw, pos, exit := parseCatVals(args)
	if exit != exitOK {
		return exit
	}
	if raw != "" {
		return usageErr("tag get: --values is for set/add/remove, not get")
	}
	if len(pos) > 1 {
		return usageErr("tag get takes at most one <target>")
	}
	root, ix, exit := loadLibrary()
	if exit != exitOK {
		return exit
	}
	if len(pos) == 0 {
		if cat != "" {
			return usageErr("tag get: --category needs a <target>")
		}
		return emitCategories(ix, asJSON, emit)
	}
	if cat != "" {
		if err := checkName(cat); err != nil {
			return usageErr("tag get: %s", err)
		}
	}
	idx, exit := resolveTarget(root, ix, pos[0], emit)
	if exit != exitOK {
		return exit
	}
	rec := ix.Files[idx]
	if cat == "" {
		return emitTagMap(rec, asJSON, emit)
	}
	return emitValues(rec, cat, asJSON, emit)
}

// emitCategories lists distinct categories across the index, sorted.
func emitCategories(ix *index.Index, asJSON bool, emit func(any)) int {
	seen := map[string]bool{}
	out := []string{}
	for i := range ix.Files {
		for cat := range ix.Files[i].Tags {
			if !seen[cat] {
				seen[cat] = true
				out = append(out, cat)
			}
		}
	}
	sort.Strings(out)
	emit(out)
	if !asJSON {
		for _, c := range out {
			fmt.Println(c)
		}
	}
	return exitOK
}

// emitTagMap returns a copy of one record's tags, never nil.
func emitTagMap(rec index.Record, asJSON bool, emit func(any)) int {
	tags := map[string][]string{}
	for cat, vals := range rec.Tags {
		cp := append([]string{}, vals...)
		tags[cat] = cp
	}
	emit(tags)
	if !asJSON {
		fmt.Println(formatTags(tags))
	}
	return exitOK
}

// emitValues returns one category's values, empty when absent.
func emitValues(rec index.Record, cat string, asJSON bool, emit func(any)) int {
	vals := append([]string{}, rec.Tags[cat]...)
	if vals == nil {
		vals = []string{}
	}
	emit(vals)
	if !asJSON {
		for _, v := range vals {
			fmt.Println(v)
		}
	}
	return exitOK
}
