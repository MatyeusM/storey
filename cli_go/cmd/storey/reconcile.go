package main

import (
	"os"
	"path/filepath"

	"github.com/matyeusm/storey/internal/hash"
	"github.com/matyeusm/storey/internal/index"
)

// reconcileStore heals crash residue: files sitting in .files/ with no
// record pointing at them. A hash match against a record whose location
// is gone repoints it (tags kept); anything else is reported for
// --force to override. Runs before any missing/prune check so recovery
// precedes judgment.
func reconcileStore(root string, ix *index.Index) []string {
	referenced := make(map[string]bool, len(ix.Files))
	for i := range ix.Files {
		if ix.Files[i].Stored() {
			referenced[ix.Files[i].StoredRel] = true
		}
	}
	entries, err := os.ReadDir(filepath.Join(root, index.StoreDir))
	if err != nil {
		return []string{}
	}
	byHash := make(map[string]int, len(ix.Files))
	for i := range ix.Files {
		byHash[ix.Files[i].ID] = i
	}
	var orphans = []string{}
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		rel := index.StoreDir + "/" + e.Name()
		if referenced[rel] {
			continue
		}
		h, size, err := hash.File(filepath.Join(root, filepath.FromSlash(rel)))
		if err != nil {
			orphans = append(orphans, rel+": "+err.Error())
			continue
		}
		j, ok := byHash[h]
		if !ok || onDisk(root, ix.Files[j].Location()) {
			orphans = append(orphans, rel)
			continue
		}
		rec := &ix.Files[j]
		rec.StoredRel = rel
		rec.Size = size
	}
	return orphans
}
