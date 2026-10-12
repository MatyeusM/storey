package main

import (
	"path/filepath"
	"testing"

	"github.com/matyeusm/storey/internal/index"
)

func TestInitAdoptsInPlaceSubLibrary(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "top.txt"), "T")
	sub := filepath.Join(root, "sub")
	writeFile(t, filepath.Join(sub, "s.txt"), "S")
	chdir(t, sub)
	mustRun(t, "init")
	mustRun(t, "tag", "set", "s.txt", "--category=actor", "--values=X")
	chdir(t, root)

	mustRun(t, "init")
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.Files) != 2 {
		t.Fatalf("want 2 records, got %+v", ix.Files)
	}
	for _, rec := range ix.Files {
		if rec.DisplayName == "s.txt" {
			if rec.OriginalRel != "sub/s.txt" {
				t.Fatalf("bad rebased path: %q", rec.OriginalRel)
			}
			if len(rec.Tags["actor"]) != 1 || rec.Tags["actor"][0] != "X" {
				t.Fatalf("tags lost: %+v", rec.Tags)
			}
		}
	}
	mustGone(t, root, "sub/.index.json")
	mustExist(t, root, "sub/s.txt")
}

func TestInitAdoptsStoredSubLibrary(t *testing.T) {
	root := t.TempDir()
	sub := filepath.Join(root, "sub")
	writeFile(t, filepath.Join(sub, "s.txt"), "S")
	chdir(t, sub)
	mustRun(t, "init")
	mustRun(t, "view", "--tags=date")
	chdir(t, root)
	writeFile(t, filepath.Join(root, "top.txt"), "T")

	mustRun(t, "init")
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.Files) != 2 {
		t.Fatalf("want 2 records, got %+v", ix.Files)
	}
	for _, rec := range ix.Files {
		if rec.DisplayName == "s.txt" {
			if !rec.Stored() {
				t.Fatalf("stored state lost: %+v", rec)
			}
			if rec.OriginalRel != "sub/s.txt" {
				t.Fatalf("bad rebased original: %q", rec.OriginalRel)
			}
			mustExist(t, root, rec.StoredRel)
		}
	}
	mustGone(t, root, "sub/.index.json")
}

func TestInitKeepsTamperedSubLibrary(t *testing.T) {
	root := t.TempDir()
	sub := filepath.Join(root, "sub")
	writeFile(t, filepath.Join(sub, "s.txt"), "S")
	chdir(t, sub)
	mustRun(t, "init")
	chdir(t, root)
	writeFile(t, filepath.Join(sub, "s.txt"), "TAMPERED")

	if code := run([]string{"init"}); code != exitFailure {
		t.Fatalf("tampered sub: exit %d, want %d", code, exitFailure)
	}
	// Untrusted index stays put for the user to inspect, and its files
	// are shielded from fresh scans so no metadata is lost.
	mustExist(t, root, "sub/.index.json")
	mustExist(t, root, "sub/s.txt")
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.Files) != 0 {
		t.Fatalf("shielded files must not be scanned: %+v", ix.Files)
	}
}

func TestInitDedupesAcrossSubLibrary(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "SAME")
	sub := filepath.Join(root, "sub")
	writeFile(t, filepath.Join(sub, "copy.txt"), "SAME")
	chdir(t, sub)
	mustRun(t, "init")
	chdir(t, root)

	mustRun(t, "init")
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.Files) != 1 {
		t.Fatalf("want 1 record, got %+v", ix.Files)
	}
	// The adopted (already indexed) record wins; the later scan hit is
	// purged per the standing duplicate rule.
	if ix.Files[0].OriginalRel != "sub/copy.txt" {
		t.Fatalf("adopted record should win: %+v", ix.Files[0])
	}
	mustGone(t, root, "a.txt")
	mustExist(t, root, "sub/copy.txt")
}
