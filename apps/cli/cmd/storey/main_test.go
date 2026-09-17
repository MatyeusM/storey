package main

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/matyeusm/storey/internal/index"
)

func chdir(t *testing.T, dir string) {
	t.Helper()
	prev, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Chdir(dir); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { os.Chdir(prev) })
}

func TestInit(t *testing.T) {
	root := t.TempDir()
	os.WriteFile(filepath.Join(root, "a.txt"), []byte("same"), 0o644)
	os.WriteFile(filepath.Join(root, "b.txt"), []byte("same"), 0o644) // dupe
	os.WriteFile(filepath.Join(root, "c.txt"), []byte("diff"), 0o644)
	os.Symlink("a.txt", filepath.Join(root, "link.txt"))
	chdir(t, root)

	if code := run([]string{"init"}); code != exitOK {
		t.Fatalf("exit %d", code)
	}
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.Files) != 2 {
		t.Fatalf("want 2 records, got %d", len(ix.Files))
	}
	if ix.Storey.Schema != index.Schema || ix.Storey.Algorithm != index.Algorithm {
		t.Fatalf("bad storey header: %+v", ix.Storey)
	}
	if _, err := os.Stat(filepath.Join(root, index.StoreDir)); !os.IsNotExist(err) {
		t.Fatal("init must not create .files (only view does)")
	}
	if _, err := os.Stat(filepath.Join(root, "b.txt")); !os.IsNotExist(err) {
		t.Fatal("dupe b.txt was not purged")
	}
	if _, err := os.Lstat(filepath.Join(root, "link.txt")); err != nil {
		t.Fatal("symlink should be untouched")
	}
	if code := run([]string{"init"}); code != exitFailure {
		t.Fatalf("second init: exit %d, want %d", code, exitFailure)
	}
}

func TestGlobalFlagsFloat(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "A")
	chdir(t, root)

	// --json/--quiet work after the command, not just before it.
	mustRun(t, "init", "--json")
	mustRun(t, "update", "--json")
	mustRun(t, "--json", "update", "--verify", "--quiet")
	mustRun(t, "list", "--quiet")
}
