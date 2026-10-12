package main

import (
	"fmt"
	"sort"
	"strings"

	"github.com/matyeusm/storey/internal/index"
	"github.com/matyeusm/storey/internal/log"
)

// catVal is a single category=value filter.
type catVal struct {
	cat string
	val string
}

// cmdList implements `storey list`: all records, optionally one
// category=value filter or --untagged. Empty results still exit 0.
func cmdList(args []string, asJSON bool, logger *log.Logger, emit func(any)) int {
	untagged, filter, exit := parseListArgs(args)
	if exit != exitOK {
		return exit
	}
	_, ix, exit := loadLibrary()
	if exit != exitOK {
		return exit
	}
	matched := filterRecords(ix.Files, untagged, filter)
	emit(matched)
	if !asJSON {
		printTable(matched)
	}
	return exitOK
}

func parseListArgs(args []string) (bool, *catVal, int) {
	untagged := false
	var positionals []string
	for _, a := range args {
		if a == "--untagged" {
			untagged = true
			continue
		}
		positionals = append(positionals, a)
	}
	if len(positionals) > 1 {
		return false, nil, usageErr("list takes at most one category=value filter")
	}
	var filter *catVal
	if len(positionals) == 1 {
		cat, val, ok := strings.Cut(positionals[0], "=")
		if !ok || cat == "" || val == "" {
			return false, nil, usageErr("list: filter must be category=value")
		}
		filter = &catVal{cat: cat, val: val}
	}
	if untagged && filter != nil {
		return false, nil, usageErr("list: --untagged excludes filters")
	}
	return untagged, filter, exitOK
}

// filterRecords selects untagged records, one exact category=value match,
// or everything.
func filterRecords(files []index.Record, untagged bool, filter *catVal) []index.Record {
	var matched []index.Record
	for _, rec := range files {
		switch {
		case untagged:
			if len(rec.Tags) == 0 {
				matched = append(matched, rec)
			}
		case filter != nil:
			if contains(rec.Tags[filter.cat], filter.val) {
				matched = append(matched, rec)
			}
		default:
			matched = append(matched, rec)
		}
	}
	return matched
}

func printTable(files []index.Record) {
	for _, rec := range files {
		fmt.Printf("%s  %s  %d  %s\n", rec.ID[:12], rec.Location(), rec.Size, formatTags(rec.Tags))
	}
}

// formatTags renders actor=X,Y date=2020 with sorted categories.
func formatTags(tags map[string][]string) string {
	if len(tags) == 0 {
		return "-"
	}
	cats := make([]string, 0, len(tags))
	for cat := range tags {
		cats = append(cats, cat)
	}
	sort.Strings(cats)
	pairs := make([]string, 0, len(cats))
	for _, cat := range cats {
		pairs = append(pairs, cat+"="+strings.Join(tags[cat], ","))
	}
	return strings.Join(pairs, " ")
}
