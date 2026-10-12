package main

import (
	"path/filepath"
	"testing"
)

func TestStatusOutsideLibrary(t *testing.T) {
	chdir(t, t.TempDir())
	if code := run([]string{"status"}); code != exitUsage {
		t.Fatalf("status outside: exit %d, want %d", code, exitUsage)
	}
}

func TestStatusOverview(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "A")
	writeFile(t, filepath.Join(root, "b.txt"), "B")
	chdir(t, root)
	mustRun(t, "init")

	var data any
	emit := func(d any) { data = d }
	if code := cmdStatus(nil, false, testLogger(), emit); code != exitOK {
		t.Fatalf("status: exit %d", code)
	}
	got, ok := data.(statusData)
	if !ok {
		t.Fatalf("bad data: %#v", data)
	}
	if got.Root != root || got.Indexed != 2 || got.Stored != 0 || got.Untracked != 0 {
		t.Fatalf("bad overview: %+v", got)
	}

	writeFile(t, filepath.Join(root, "new.txt"), "N")
	mustRun(t, "update")
	mustRun(t, "view", "--tags=date")
	if code := cmdStatus(nil, true, testLogger(), emit); code != exitOK {
		t.Fatalf("status after view: exit %d", code)
	}
	got = data.(statusData)
	if got.Indexed != 3 || got.Stored != 3 || got.Untracked != 0 || len(got.View) != 1 {
		t.Fatalf("bad overview after view: %+v", got)
	}

	if code := run([]string{"status", "--bogus"}); code != exitUsage {
		t.Fatalf("status args: exit %d, want %d", code, exitUsage)
	}
}
