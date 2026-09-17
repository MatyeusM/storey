package main

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/matyeusm/storey/internal/index"
)

func setupViewedLibrary(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "films", "a.mkv"), "A")
	writeFile(t, filepath.Join(root, "films", "b.mkv"), "B")
	chdir(t, root)
	mustRun(t, "init")
	tagRecord(t, root, "a.mkv", map[string][]string{"date": {"2020"}})
	tagRecord(t, root, "b.mkv", map[string][]string{"date": {"2020"}})
	mustRun(t, "view", "--tags=date")
	return root
}

func recordID(t *testing.T, root, name string) string {
	t.Helper()
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	for _, rec := range ix.Files {
		if rec.DisplayName == name {
			return rec.ID
		}
	}
	t.Fatalf("no record %q", name)
	return ""
}

func TestRmByLinkPath(t *testing.T) {
	root := setupViewedLibrary(t)
	mustRun(t, "rm", "2020/a.mkv")
	mustGone(t, root, "2020/a.mkv")
	mustGone(t, root, ".files/a.mkv")
	mustExist(t, root, "2020/b.mkv")
	mustExist(t, root, ".files/b.mkv")
	ix, _ := index.Load(root)
	if len(ix.Files) != 1 {
		t.Fatalf("want 1 record, got %d", len(ix.Files))
	}
}

func TestRmByIDPrefixAndDryRun(t *testing.T) {
	root := setupViewedLibrary(t)
	if code := run([]string{"rm", "--dry-run", "2020/b.mkv"}); code != exitOK {
		t.Fatalf("dry run: exit %d", code)
	}
	mustExist(t, root, "2020/b.mkv")
	mustRun(t, "rm", recordID(t, root, "b.mkv")[:12])
	mustGone(t, root, ".files/b.mkv")
	mustExist(t, root, "2020/a.mkv")
	mustRun(t, "rm", recordID(t, root, "a.mkv")[:12])
	mustGone(t, root, "2020")
	if code := run([]string{"rm", "nope"}); code != exitFailure {
		t.Fatalf("unknown target: exit %d", code)
	}
}

func TestMvStored(t *testing.T) {
	root := setupViewedLibrary(t)
	mustRun(t, "mv", "2020/a.mkv", "alpha.mkv")
	mustGone(t, root, "2020/a.mkv")
	mustExist(t, root, "2020/alpha.mkv")
	mustExist(t, root, ".files/alpha.mkv")
	content, _ := os.ReadFile(filepath.Join(root, "2020", "alpha.mkv"))
	if string(content) != "A" {
		t.Fatal("renamed link has wrong content")
	}
	mustRun(t, "mv", recordID(t, root, "b.mkv")[:8], "alpha.mkv")
	mustExist(t, root, ".files/alpha_1.mkv")
}

func TestMvInPlaceAndValidation(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "sub", "a.txt"), "A")
	chdir(t, root)
	mustRun(t, "init")
	mustRun(t, "mv", "sub/a.txt", "renamed.txt")
	mustExist(t, root, "sub/renamed.txt")
	mustRun(t, "mv", "sub/renamed.txt", "renamed.txt")
	for _, bad := range []string{"a/b", "CON", "x.", "", "a,b", "a*b"} {
		if code := run([]string{"mv", "sub/renamed.txt", bad}); code != exitUsage {
			t.Fatalf("name %q: exit %d, want %d", bad, code, exitUsage)
		}
	}
	if code := run([]string{"mv", "--dry-run", "sub/renamed.txt", "dry.txt"}); code != exitOK {
		t.Fatalf("dry run: exit %d", code)
	}
	mustGone(t, root, "sub/dry.txt")
}

func TestCheckName(t *testing.T) {
	for _, ok := range []string{"hello world", "a-b_c.mkv", "X", "1990"} {
		if err := checkName(ok); err != nil {
			t.Fatalf("%q rejected: %s", ok, err)
		}
	}
	for _, bad := range []string{"", "  ", "a/b", `a\b`, "CON", "nul.txt", "x.", "y ", "a:b", "a,b"} {
		if err := checkName(bad); err == nil {
			t.Fatalf("%q accepted", bad)
		}
	}
}
