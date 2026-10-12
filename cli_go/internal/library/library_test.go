package library

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/matyeusm/storey/internal/index"
)

func setupWalk(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	os.MkdirAll(filepath.Join(root, "sub"), 0o755)
	os.MkdirAll(filepath.Join(root, index.StoreDir), 0o755)
	os.WriteFile(filepath.Join(root, "b.txt"), []byte("b"), 0o644)
	os.WriteFile(filepath.Join(root, "a.txt"), []byte("a"), 0o644)
	os.WriteFile(filepath.Join(root, "sub", "c.txt"), []byte("c"), 0o644)
	os.WriteFile(filepath.Join(root, index.StoreDir, "stored"), []byte("s"), 0o644)
	os.WriteFile(filepath.Join(root, index.FileName), []byte("{}"), 0o644)
	os.WriteFile(filepath.Join(root, index.LockName), []byte("123\n"), 0o644)
	os.Symlink("a.txt", filepath.Join(root, "link.txt"))
	os.Symlink("sub", filepath.Join(root, "linkdir"))
	os.WriteFile(filepath.Join(root, ".DS_Store"), []byte("j"), 0o644)
	os.WriteFile(filepath.Join(root, "Thumbs.db"), []byte("j"), 0o644)
	os.WriteFile(filepath.Join(root, "film.mkv.part"), []byte("j"), 0o644)
	return root
}

func TestWalk(t *testing.T) {
	paths, err := Walk(setupWalk(t))
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"a.txt", "b.txt", "sub/c.txt"}
	if len(paths) != len(want) {
		t.Fatalf("got %v", paths)
	}
	for i := range want {
		if paths[i] != want[i] {
			t.Fatalf("got %v want %v", paths, want)
		}
	}
}

func TestFindRoot(t *testing.T) {
	root := setupWalk(t)
	sub := filepath.Join(root, "sub")
	got, err := FindRoot(sub)
	if err != nil || got != root {
		t.Fatalf("got %q %v", got, err)
	}
	if _, err := FindRoot(t.TempDir()); !errors.Is(err, ErrNotFound) {
		t.Fatalf("expected ErrNotFound, got %v", err)
	}
}
