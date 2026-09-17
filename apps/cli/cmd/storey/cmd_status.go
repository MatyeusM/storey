package main

import (
	"fmt"
	"os"
	"strings"

	"github.com/matyeusm/storey/internal/library"
	"github.com/matyeusm/storey/internal/log"
)

// statusData is the --json data payload for status.
type statusData struct {
	Root      string   `json:"root"`
	Indexed   int      `json:"indexed"`
	Stored    int      `json:"stored"`
	Untracked int      `json:"untracked"`
	View      []string `json:"view"`
}

// cmdStatus implements `storey status`: a read-only overview for humans
// and UIs — where the library is, how much is indexed, how much is
// stored, and how many files wait on disk. No hashing, no mutation.
func cmdStatus(args []string, asJSON bool, _ *log.Logger, emit func(any)) int {
	if len(args) != 0 {
		return usageErr("status takes no arguments")
	}
	cwd, err := os.Getwd()
	if err != nil {
		return fail("%s", err)
	}
	root, err := library.FindRoot(cwd)
	if err != nil {
		fmt.Fprintln(os.Stderr, "fatal: not a storey repository")
		return exitUsage
	}
	ix, err := loadIndex(root)
	if err != nil {
		return fail("%s", err)
	}
	data := statusData{Root: root, View: ix.View.By}
	if data.View == nil {
		data.View = []string{}
	}
	for i := range ix.Files {
		data.Indexed++
		if ix.Files[i].Stored() {
			data.Stored++
		}
	}
	untracked, exit := findUntracked(root, ix)
	if exit != exitOK {
		return exit
	}
	data.Untracked = len(untracked)
	emit(data)
	if !asJSON {
		printStatus(data)
	}
	return exitOK
}

func printStatus(data statusData) {
	view := strings.Join(data.View, ",")
	if view == "" {
		view = "(none)"
	}
	fmt.Printf("storey repository at %s\nindexed: %d\nstored: %d\nuntracked: %d\nview by: %s\n",
		data.Root, data.Indexed, data.Stored, data.Untracked, view)
}
