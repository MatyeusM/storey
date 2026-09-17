package hash

import (
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"testing"
)

func TestFile(t *testing.T) {
	p := filepath.Join(t.TempDir(), "f")
	os.WriteFile(p, []byte("abc"), 0o644)
	sum := sha256.Sum256([]byte("abc"))
	h, n, err := File(p)
	if err != nil || n != 3 || h != hex.EncodeToString(sum[:]) {
		t.Fatalf("got %q %d %v", h, n, err)
	}
}

func TestFilesPool(t *testing.T) {
	dir := t.TempDir()
	var paths []string
	for _, name := range []string{"a", "b", "c"} {
		p := filepath.Join(dir, name)
		os.WriteFile(p, []byte(name), 0o644)
		paths = append(paths, p)
	}
	calls := 0
	results := Files(paths, 2, func(done, total int, path string) {
		calls++
		if total != 3 {
			t.Fatalf("total = %d", total)
		}
		if path == "" {
			t.Fatal("progress without path")
		}
	})
	if len(results) != 3 || calls != 3 {
		t.Fatalf("got %d results %d progress calls", len(results), calls)
	}
	for _, r := range results {
		if r.Err != nil || r.Hash == "" {
			t.Fatalf("bad result %+v", r)
		}
	}
	if Workers() < 1 {
		t.Fatal("Workers < 1")
	}
}
