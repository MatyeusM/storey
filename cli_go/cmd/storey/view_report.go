package main

import (
	"fmt"
	"strings"

	"github.com/matyeusm/storey/internal/views"
)

// linkPaths renders planned view links as slash-relative paths.
func linkPaths(links []views.Link) []string {
	paths := make([]string, 0, len(links))
	for _, l := range links {
		paths = append(paths, l.ViewPath)
	}
	return paths
}

func countUnsorted(links []views.Link) int {
	n := 0
	for _, l := range links {
		for _, seg := range strings.Split(l.ViewPath, "/") {
			if seg == views.Unsorted {
				n++
				break
			}
		}
	}
	return n
}

func printViewSummary(asJSON bool, by []string, moved, purged, links, linkErrors, unsorted int, dry bool) {
	if asJSON {
		return
	}
	suffix := ""
	if dry {
		suffix = " (dry run)"
	}
	fmt.Printf("view by %s%s\nmoved: %d\npurged (duplicates): %d\nlinks: %d\nlink errors: %d\nunsorted: %d\n",
		strings.Join(by, ","), suffix, moved, purged, links, linkErrors, unsorted)
}
