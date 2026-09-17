package main

import (
	"path/filepath"
	"testing"

	"github.com/matyeusm/storey/internal/index"
)

func setupTaggedLib(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "a.txt"), "A")
	chdir(t, root)
	mustRun(t, "init")
	return root
}

func tagsOf(t *testing.T, root string) map[string][]string {
	t.Helper()
	ix, err := index.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	return ix.Files[0].Tags
}

func TestTagSetAddRemove(t *testing.T) {
	root := setupTaggedLib(t)
	mustRun(t, "tag", "set", "a.txt", "--category=actor", "--values=X, Y, X")
	if got := tagsOf(t, root)["actor"]; len(got) != 2 || got[0] != "X" || got[1] != "Y" {
		t.Fatalf("set+dedupe: %v", got)
	}
	mustRun(t, "tag", "add", "a.txt", "--category=actor", "--values=Y,Z")
	if got := tagsOf(t, root)["actor"]; len(got) != 3 || got[2] != "Z" {
		t.Fatalf("add: %v", got)
	}
	mustRun(t, "tag", "remove", "a.txt", "--category=actor", "--values=X")
	if got := tagsOf(t, root)["actor"]; len(got) != 2 {
		t.Fatalf("remove value: %v", got)
	}
	mustRun(t, "tag", "remove", "a.txt", "--category=actor")
	if _, ok := tagsOf(t, root)["actor"]; ok {
		t.Fatal("category should be gone")
	}
}

func TestTagRemoveAllAndCombos(t *testing.T) {
	root := setupTaggedLib(t)
	mustRun(t, "tag", "set", "a.txt", "--category=a", "--values=1")
	mustRun(t, "tag", "set", "a.txt", "--category=b", "--values=2")
	mustRun(t, "tag", "remove", "a.txt")
	if len(tagsOf(t, root)) != 0 {
		t.Fatal("all tags should be gone")
	}
	bad := [][]string{
		{"tag", "set", "a.txt", "--category=a", "--values=1", "--merge"},
		{"tag", "set", "a.txt", "--values=1"},
		{"tag", "set", "a.txt", "--category=a"},
		{"tag", "set", "a.txt"},
		{"tag", "set"},
		{"tag", "remove", "a.txt", "--values=1"},
		{"tag", "get", "a.txt", "--values=1"},
		{"tag", "get", "a.txt", "b.txt"},
		{"tag", "bogus", "a.txt"},
		{"tag", "set", "a.txt", "--category=a/b", "--values=x"},
		{"tag", "set", "a.txt", "--category=a", "--values=x,"},
	}
	for _, args := range bad {
		if code := run(args); code != exitUsage {
			t.Fatalf("%v: exit %d, want %d", args, code, exitUsage)
		}
	}
}

func TestTagGetReads(t *testing.T) {
	root := setupTaggedLib(t)
	mustRun(t, "tag", "set", "a.txt", "--category=actor", "--values=X,Y")
	mustRun(t, "tag", "set", "a.txt", "--category=date", "--values=2020")
	mustRun(t, "tag", "get")
	mustRun(t, "tag", "get", "a.txt")
	mustRun(t, "tag", "get", "a.txt", "--category=actor")
	if code := run([]string{"tag", "get", "a.txt", "--category=missing"}); code != exitOK {
		t.Fatalf("missing category read: exit %d", code)
	}
	_ = root
}

func TestTagByIDAndViewRoundtrip(t *testing.T) {
	root := setupTaggedLib(t)
	ix, _ := index.Load(root)
	mustRun(t, "tag", "set", ix.Files[0].ID[:10], "--category=date", "--values=2020")
	mustRun(t, "view", "--tags=date")
	mustExist(t, root, "2020/a.txt")
}
