package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/matyeusm/storey/internal/index"
)

func writeFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func mustRun(t *testing.T, args ...string) {
	t.Helper()
	if code := run(args); code != exitOK {
		t.Fatalf("%v: exit %d", args, code)
	}
}

func mustExist(t *testing.T, root, rel string) {
	t.Helper()
	if _, err := os.Lstat(filepath.Join(root, filepath.FromSlash(rel))); err != nil {
		t.Fatalf("missing %s: %v", rel, err)
	}
}

func mustGone(t *testing.T, root, rel string) {
	t.Helper()
	if _, err := os.Lstat(filepath.Join(root, filepath.FromSlash(rel))); !os.IsNotExist(err) {
		t.Fatalf("%s should be gone", rel)
	}
}

func tagRecord(t *testing.T, root, name string, tags map[string][]string) {
	t.Helper()
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for i := range ix.Files {
		if ix.Files[i].DisplayName == name {
			ix.Files[i].Tags = tags
			found = true
		}
	}
	if !found {
		t.Fatalf("no record %q", name)
	}
	if err := ix.Save(root); err != nil {
		t.Fatal(err)
	}
}

func setupTaggedLibrary(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "films", "goodfellas.mkv"), "g")
	writeFile(t, filepath.Join(root, "films", "casino.mkv"), "c")
	writeFile(t, filepath.Join(root, "doc.txt"), "d")
	chdir(t, root)
	mustRun(t, "init")
	tagRecord(t, root, "goodfellas.mkv", map[string][]string{"date": {"1990"}, "actor": {"X", "Y"}})
	tagRecord(t, root, "casino.mkv", map[string][]string{"date": {"1995"}, "actor": {"X"}})
	return root
}

func assertViewBuilt(t *testing.T, root string) {
	t.Helper()
	for _, p := range []string{
		".files/goodfellas.mkv", ".files/casino.mkv", ".files/doc.txt",
		"1990/X/goodfellas.mkv", "1990/Y/goodfellas.mkv",
		"1995/X/casino.mkv", "Unsorted/doc.txt",
	} {
		mustExist(t, root, p)
	}
	mustGone(t, root, "films")
	content, err := os.ReadFile(filepath.Join(root, "1990", "X", "goodfellas.mkv"))
	if err != nil || string(content) != "g" {
		t.Fatal("view link does not resolve to content")
	}
}

func assertRestored(t *testing.T, root string) {
	t.Helper()
	for _, p := range []string{"films/goodfellas.mkv", "films/casino.mkv", "doc.txt"} {
		mustExist(t, root, p)
	}
	entries, _ := os.ReadDir(filepath.Join(root, ".files"))
	if len(entries) != 0 {
		t.Fatal(".files should be empty after restore")
	}
	for _, p := range []string{"1990", "1995", "Unsorted", "date"} {
		mustGone(t, root, p)
	}
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.View.By) != 0 {
		t.Fatal("view.by should be cleared")
	}
	for _, rec := range ix.Files {
		if rec.Stored() {
			t.Fatalf("%s still marked stored", rec.ID)
		}
	}
}

func TestViewEmptyTagsRendersEmptyDir(t *testing.T) {
	root := setupTaggedLibrary(t)
	mustRun(t, "view", "--tags=date,actor")
	assertViewBuilt(t, root)
	mustRun(t, "view", "--tags=")
	for _, p := range []string{"1990", "1995", "Unsorted", "films"} {
		mustGone(t, root, p)
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 2 {
		t.Fatalf("root has %d entries, want only .files and .index.json", len(entries))
	}
	mustExist(t, root, ".files/goodfellas.mkv")
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(ix.View.By) != 0 {
		t.Fatalf("view.by = %v, want empty", ix.View.By)
	}
	for _, rec := range ix.Files {
		if !rec.Stored() {
			t.Fatalf("%s should stay stored", rec.ID)
		}
	}
	mustRun(t, "view", "--tags=date")
	mustExist(t, root, "1990/goodfellas.mkv")
}

func TestViewJSONEmitsArraysNotNull(t *testing.T) {
	setupTaggedLibrary(t)
	var got any
	emit := func(data any) { got = data }
	if code := cmdView([]string{"--tags=date,actor"}, true, testLogger(), emit); code != exitOK {
		t.Fatalf("view: exit %d", code)
	}
	raw, err := json.Marshal(got)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(raw), ":null") {
		t.Fatalf("view payload contains null: %s", raw)
	}
}

func TestViewRestoreRoundtrip(t *testing.T) {
	root := setupTaggedLibrary(t)
	mustRun(t, "view", "--tags=date,actor")
	assertViewBuilt(t, root)
	writeFile(t, filepath.Join(root, "new.txt"), "n")
	if code := run([]string{"view", "--tags=date"}); code != exitFailure {
		t.Fatalf("re-view with untracked: exit %d, want %d", code, exitFailure)
	}
	mustRun(t, "view", "--tags=date", "--force")
	mustGone(t, root, "new.txt")
	mustRun(t, "restore")
	assertRestored(t, root)
	if code := run([]string{"restore"}); code != exitOK {
		t.Fatalf("second restore: exit %d", code)
	}
}

func TestRestoreConflict(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "orig")
	chdir(t, root)
	mustRun(t, "init")
	mustRun(t, "view", "--tags=date")
	writeFile(t, filepath.Join(root, "a.txt"), "squatter")
	if code := run([]string{"restore"}); code != exitFailure {
		t.Fatalf("conflict restore: exit %d, want %d", code, exitFailure)
	}
	mustRun(t, "restore", "--force")
	content, _ := os.ReadFile(filepath.Join(root, "a.txt"))
	if string(content) != "orig" {
		t.Fatalf("conflict not overwritten: %q", content)
	}
}

func TestRestoreClearsSharedViewDirs(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.mkv"), "a")
	writeFile(t, filepath.Join(root, "b.mkv"), "b")
	chdir(t, root)
	mustRun(t, "init")
	tagRecord(t, root, "a.mkv", map[string][]string{"date": {"2020"}})
	tagRecord(t, root, "b.mkv", map[string][]string{"date": {"2020"}})
	mustRun(t, "view", "--tags=date")
	mustExist(t, root, "2020/a.mkv")
	mustExist(t, root, "2020/b.mkv")
	mustRun(t, "restore")
	mustExist(t, root, "a.mkv")
	mustExist(t, root, "b.mkv")
	mustGone(t, root, "2020")
}

func TestViewReconcilesCrashResidue(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "A")
	chdir(t, root)
	mustRun(t, "init")
	mustRun(t, "tag", "set", "a.txt", "--category", "genre", "--values", "rock")
	mustRun(t, "view", "--tags=genre")
	// Simulate a crash mid-move: store file stranded, index stale.
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	if !ix.Files[0].Stored() {
		t.Fatalf("record should be stored: %+v", ix.Files[0])
	}
	stored := ix.Files[0].StoredRel
	if err := os.Rename(filepath.Join(root, filepath.FromSlash(stored)), filepath.Join(root, ".files", "stranded")); err != nil {
		t.Fatal(err)
	}
	ix.Files[0].StoredRel = ""
	if err := ix.Save(root); err != nil {
		t.Fatal(err)
	}
	mustRun(t, "view", "--tags=genre")
	ix, err = index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	rec := ix.Files[0]
	if rec.StoredRel != ".files/stranded" {
		t.Fatalf("orphan not repointed: %+v", rec)
	}
	if len(rec.Tags["genre"]) != 1 || rec.Tags["genre"][0] != "rock" {
		t.Fatalf("tags lost on reconcile: %+v", rec.Tags)
	}
	mustExist(t, root, "rock/a.txt")
}
