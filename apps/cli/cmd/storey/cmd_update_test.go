package main

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/matyeusm/storey/internal/index"
)

func TestUpdateDetectsMove(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "A")
	chdir(t, root)
	mustRun(t, "init")
	mustRun(t, "tag", "set", "a.txt", "--category", "genre", "--values", "rock")
	if err := os.MkdirAll(filepath.Join(root, "sub"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Rename(filepath.Join(root, "a.txt"), filepath.Join(root, "sub", "b.txt")); err != nil {
		t.Fatal(err)
	}
	mustRun(t, "update")
	mustExist(t, root, "sub/b.txt")
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.Files) != 1 {
		t.Fatalf("want 1 record, got %+v", ix.Files)
	}
	rec := ix.Files[0]
	if rec.OriginalRel != "sub/b.txt" || rec.DisplayName != "b.txt" {
		t.Fatalf("record not repointed: %+v", rec)
	}
	if len(rec.Tags["genre"]) != 1 || rec.Tags["genre"][0] != "rock" {
		t.Fatalf("tags lost on move: %+v", rec.Tags)
	}
}

func TestUpdateTrustedSkipsReverify(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "A")
	writeFile(t, filepath.Join(root, "b.txt"), "B")
	chdir(t, root)
	mustRun(t, "init")

	writeFile(t, filepath.Join(root, "c.txt"), "C")
	writeFile(t, filepath.Join(root, "dupe.txt"), "A")
	writeFile(t, filepath.Join(root, "dupe2.txt"), "C")
	writeFile(t, filepath.Join(root, "b.txt"), "B-changed")
	os.Remove(filepath.Join(root, "a.txt"))

	// Default is trusted: new files are indexed, old ones untouched.
	// dupe.txt matches the deleted a.txt, so it is repointed as a move
	// (tags kept); dupe2.txt matches live c.txt, so it is purged.
	mustRun(t, "update")
	mustExist(t, root, "dupe.txt")
	mustGone(t, root, "dupe2.txt")
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.Files) != 3 {
		t.Fatalf("want 3 records, got %d", len(ix.Files))
	}

	// --verify re-hashes everything: changed files are problems.
	if code := run([]string{"update", "--verify"}); code != exitFailure {
		t.Fatalf("verify with problems: exit %d, want %d", code, exitFailure)
	}
	writeFile(t, filepath.Join(root, "b.txt"), "B")
	os.Remove(filepath.Join(root, "c.txt"))
	mustRun(t, "update", "--verify")
	ix, err = index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.Files) != 2 {
		t.Fatalf("want b + dupe = 2 records, got %+v", ix.Files)
	}
	for _, rec := range ix.Files {
		if rec.OriginalRel == "c.txt" {
			t.Fatalf("missing file should be pruned: %+v", rec)
		}
	}
}

func TestUpdateVerifyPrunesMissingOnly(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "A")
	writeFile(t, filepath.Join(root, "b.txt"), "B")
	chdir(t, root)
	mustRun(t, "init")
	os.Remove(filepath.Join(root, "a.txt"))

	// A lone missing file prunes cleanly: exit 0, record dropped.
	mustRun(t, "update", "--verify")
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.Files) != 1 || ix.Files[0].OriginalRel != "b.txt" {
		t.Fatalf("want only b.txt left, got %+v", ix.Files)
	}
}

func TestUpdateRejectsUnknownFlags(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "A")
	chdir(t, root)
	mustRun(t, "init")
	if code := run([]string{"update", "--bogus"}); code != exitUsage {
		t.Fatalf("bogus flag: exit %d, want %d", code, exitUsage)
	}
}

func TestUpdateAfterView(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "A")
	chdir(t, root)
	mustRun(t, "init")
	mustRun(t, "view", "--tags=date")
	writeFile(t, filepath.Join(root, "new.txt"), "N")
	mustRun(t, "update")
	mustExist(t, root, "new.txt")
}

func TestUpdateAdoptsNestedLibrary(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "top.txt"), "T")
	chdir(t, root)
	mustRun(t, "init")

	// A nested library cannot be init'ed inside a library; it arrives
	// from elsewhere (moved in) and is adopted on update.
	src := t.TempDir()
	writeFile(t, filepath.Join(src, "s.txt"), "S")
	chdir(t, src)
	mustRun(t, "init")
	mustRun(t, "tag", "set", "s.txt", "--category=actor", "--values=X")
	chdir(t, root)
	if err := os.Rename(src, filepath.Join(root, "sub")); err != nil {
		t.Fatal(err)
	}

	mustRun(t, "update")
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, rec := range ix.Files {
		if rec.OriginalRel == "sub/s.txt" {
			found = true
			if len(rec.Tags["actor"]) != 1 || rec.Tags["actor"][0] != "X" {
				t.Fatalf("tags lost on adopt: %+v", rec)
			}
		}
	}
	if !found {
		t.Fatalf("nested record not adopted: %+v", ix.Files)
	}
	mustGone(t, root, "sub/.index.json")
}

func TestUnreadableFileSkippedNotFatal(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "good.txt"), "G")
	bad := filepath.Join(root, "bad.txt")
	writeFile(t, bad, "B")
	if err := os.Chmod(bad, 0); err != nil {
		t.Skip("cannot restrict file permissions")
	}
	defer os.Chmod(bad, 0o644)
	if f, err := os.Open(bad); err == nil {
		f.Close()
		t.Skip("unreadable files stay readable here")
	}
	chdir(t, root)
	if code := run([]string{"init"}); code != exitFailure {
		t.Fatalf("init with problems: exit %d, want %d", code, exitFailure)
	}
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.Files) != 1 || ix.Files[0].DisplayName != "good.txt" {
		t.Fatalf("good file should index alone: %+v", ix.Files)
	}
}

func TestDryRunChangesNothing(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "A")
	writeFile(t, filepath.Join(root, "dupe.txt"), "A")
	chdir(t, root)
	mustRun(t, "init", "--dry-run")
	mustGone(t, root, ".index.json")
	mustExist(t, root, "dupe.txt")
	mustRun(t, "init")
	mustGone(t, root, "dupe.txt")
	writeFile(t, filepath.Join(root, "dupe2.txt"), "A")
	mustRun(t, "update", "--dry-run", "--verify")
	mustExist(t, root, "dupe2.txt")
	mustRun(t, "update")
	mustGone(t, root, "dupe2.txt")
}

func TestSecondWriterFailsLoudly(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "A")
	chdir(t, root)
	mustRun(t, "init")
	release, exit := lockLibrary(root)
	if exit != exitOK {
		t.Fatal("cannot claim test lock")
	}
	if code := run([]string{"update"}); code != exitFailure {
		t.Fatalf("locked update: exit %d, want %d", code, exitFailure)
	}
	release()
	mustRun(t, "update")
	mustGone(t, root, ".index.lock")
}
