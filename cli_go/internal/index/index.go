// Package index holds the .index.json read/write logic and record types.
// Schema v1, per docs/CLI-CONTRACT.md. Writes are atomic
// (temp file + fsync + rename) so a crash never leaves a half-written index.
package index

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

// Schema is the current index format version.
const Schema = 1

// Algorithm is the hash recorded in the index header.
const Algorithm = "sha256"

// FileName is the index filename at the library root.
const FileName = ".index.json"

// StoreDir is the content store directory at the library root.
const StoreDir = ".files"

// LockName is the write-lock file at the library root. Walk skips it
// so the lock itself is never indexed as content.
const LockName = ".index.lock"

// Index is the on-disk library state. The storey header identifies the
// file as a storey index and versions the format + hash algorithm, so a
// foreign .index.json is never mistaken for ours.
type Index struct {
	Storey Header   `json:"storey"`
	View   View     `json:"view"`
	Files  []Record `json:"files"`
}

// Header identifies the index format version and hash algorithm.
type Header struct {
	Schema    int    `json:"schema"`
	Algorithm string `json:"algorithm"`
}

// View records the last materialized view key order.
type View struct {
	By []string `json:"by"`
}

// Record is one indexed file. ID is the SHA-256 hex of the content.
// OriginalRel is the slash-separated path (relative to root) where the
// file was first indexed. StoredRel is the slash-separated path of the
// store object (".files/<name>"), empty until `view` ingests the file.
// DisplayName is the basename used for the store object and view links.
// Tags maps category → unique values.
type Record struct {
	ID          string              `json:"id"`
	DisplayName string              `json:"displayName"`
	OriginalRel string              `json:"originalRel"`
	StoredRel   string              `json:"storedRel"`
	Size        int64               `json:"size"`
	Tags        map[string][]string `json:"tags"`
}

// Stored reports whether the object has been ingested into .files/.
func (r *Record) Stored() bool {
	return r.StoredRel != ""
}

// Location returns the record's current path relative to root.
func (r *Record) Location() string {
	if r.Stored() {
		return r.StoredRel
	}
	return r.OriginalRel
}

// New returns an empty v1 index.
func New() *Index {
	return &Index{Storey: Header{Schema: Schema, Algorithm: Algorithm}, View: View{By: []string{}}, Files: []Record{}}
}

// Load reads and validates the index at root. Only the namespaced
// {"storey": {"schema": 1, "algorithm": "sha256"}, ...} shape is
// accepted — anything else is rejected, never silently misread.
func Load(root string) (*Index, error) {
	data, err := os.ReadFile(filepath.Join(root, FileName))
	if err != nil {
		return nil, err
	}
	var ix Index
	if err := json.Unmarshal(data, &ix); err != nil {
		return nil, fmt.Errorf("parse %s: %w", FileName, err)
	}
	if ix.Storey.Schema == 0 && ix.Storey.Algorithm == "" {
		return nil, fmt.Errorf("parse %s: missing storey header", FileName)
	}
	if ix.Storey.Schema != Schema {
		return nil, fmt.Errorf("unsupported %s schema %d (want %d)", FileName, ix.Storey.Schema, Schema)
	}
	if ix.Storey.Algorithm != Algorithm {
		return nil, fmt.Errorf("unsupported hash algorithm %q (want %q)", ix.Storey.Algorithm, Algorithm)
	}
	if ix.Files == nil {
		ix.Files = []Record{}
	}
	if ix.View.By == nil {
		ix.View.By = []string{}
	}
	for i := range ix.Files {
		if ix.Files[i].Tags == nil {
			ix.Files[i].Tags = map[string][]string{}
		}
	}
	return &ix, nil
}

// Save writes the index atomically: temp file in the same directory,
// fsync, then rename over the original.
func (ix *Index) Save(root string) error {
	data, err := json.MarshalIndent(ix, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	dir := root
	tmp, err := os.CreateTemp(dir, ".index.json.tmp-*")
	if err != nil {
		return err
	}
	tmpName := tmp.Name()
	defer os.Remove(tmpName)
	if _, err := tmp.Write(data); err != nil {
		tmp.Close()
		return err
	}
	if err := tmp.Sync(); err != nil {
		tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	return os.Rename(tmpName, filepath.Join(dir, FileName))
}

// Find returns the record with the given id, or nil.
func (ix *Index) Find(id string) *Record {
	for i := range ix.Files {
		if ix.Files[i].ID == id {
			return &ix.Files[i]
		}
	}
	return nil
}

// FindPrefix resolves a full id or unambiguous hex prefix.
// It returns the record, or nil + true when the prefix is ambiguous.
func (ix *Index) FindPrefix(prefix string) (rec *Record, ambiguous bool) {
	var match *Record
	for i := range ix.Files {
		h := ix.Files[i].ID
		if h == prefix {
			return &ix.Files[i], false
		}
		if len(prefix) < len(h) && h[:len(prefix)] == prefix {
			if match != nil {
				return nil, true
			}
			match = &ix.Files[i]
		}
	}
	return match, false
}
