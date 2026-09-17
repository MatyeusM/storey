package adopt

import (
	"os"
	"path/filepath"

	"github.com/matyeusm/storey/internal/index"
	"github.com/matyeusm/storey/internal/views"
)

// adoptDuplicate resolves a verified record whose id is already indexed:
// the same path is the same object (skip), another path is duplicate
// content (purge the merged file). Both outcomes resolve the record.
func adoptDuplicate(sub, subAbs string, rec, dup *index.Record, res *Result) bool {
	if dup.Location() == sub+"/"+rec.OriginalRel {
		return true
	}
	if err := os.Remove(filepath.Join(subAbs, filepath.FromSlash(rec.Location()))); err != nil {
		return problem(res, sub+"/"+rec.Location()+": "+err.Error())
	}
	res.Purged = append(res.Purged, rec.ID)
	return true
}

// adoptStored moves a verified stored object from the sub-store into
// the parent .files/, resolving basename collisions with the stem
// suffix rule, and records it in ix.
func adoptStored(root, sub, subAbs string, rec *index.Record, size int64, ix *index.Index, res *Result) bool {
	taken := storedNames(ix)
	name := views.StoreName(rec.DisplayName, taken, rec.ID)
	dstRel := index.StoreDir + "/" + name
	subLoc := rec.Location()
	if err := os.MkdirAll(filepath.Join(root, index.StoreDir), 0o755); err != nil {
		return problem(res, subLoc+": "+err.Error())
	}
	srcAbs := filepath.Join(subAbs, filepath.FromSlash(subLoc))
	dstAbs := filepath.Join(root, filepath.FromSlash(dstRel))
	if err := os.Rename(srcAbs, dstAbs); err != nil {
		return problem(res, subLoc+": "+err.Error())
	}
	ix.Files = append(ix.Files, index.Record{
		ID:          rec.ID,
		DisplayName: rec.DisplayName,
		OriginalRel: sub + "/" + rec.OriginalRel,
		StoredRel:   dstRel,
		Size:        size,
		Tags:        copyTags(rec.Tags),
	})
	res.Adopted = append(res.Adopted, rec.ID)
	return true
}

// adoptInPlace records a verified in-place file with its tags,
// rebasing its path into parent space. The file stays where it is.
func adoptInPlace(sub string, rec *index.Record, size int64, ix *index.Index, res *Result) {
	ix.Files = append(ix.Files, index.Record{
		ID:          rec.ID,
		DisplayName: rec.DisplayName,
		OriginalRel: sub + "/" + rec.OriginalRel,
		Size:        size,
		Tags:        copyTags(rec.Tags),
	})
	res.Adopted = append(res.Adopted, rec.ID)
}

// storedNames maps parent store basenames to their record ids.
func storedNames(ix *index.Index) map[string]string {
	taken := map[string]string{}
	for i := range ix.Files {
		if ix.Files[i].Stored() {
			taken[filepath.Base(ix.Files[i].StoredRel)] = ix.Files[i].ID
		}
	}
	return taken
}

func copyTags(tags map[string][]string) map[string][]string {
	out := map[string][]string{}
	for cat, vals := range tags {
		out[cat] = append([]string{}, vals...)
	}
	return out
}
