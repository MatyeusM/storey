package main

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/matyeusm/storey/internal/index"
)

// probeLinks verifies the store can host view links before anything
// moves: symlink preferred, hard link fallback. Filesystems without
// either (exFAT/FAT32 drives, Windows without developer mode, stores
// spanning volumes) would otherwise build a silently empty view.
func probeLinks(root string) error {
	dir := filepath.Join(root, index.StoreDir)
	target := filepath.Join(dir, ".probe-target")
	probe := filepath.Join(dir, ".probe")
	f, err := os.Create(target)
	if err != nil {
		return err
	}
	f.Close()
	defer os.Remove(target)
	defer os.Remove(probe)
	if err := os.Symlink(".probe-target", probe); err == nil {
		return nil
	}
	if err := os.Link(target, probe); err == nil {
		return nil
	}
	return fmt.Errorf("view links need symlinks or hard links, and this filesystem supports neither (exFAT/FAT32 drives, Windows without developer mode, or a store spanning volumes all fail this way)")
}
