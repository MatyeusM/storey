package index

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestSaveLoadRoundtrip(t *testing.T) {
	root := t.TempDir()
	ix := New()
	ix.Files = append(ix.Files, Record{
		ID: "abc", DisplayName: "a.mkv", OriginalRel: "sub/a.mkv",
		StoredRel: ".files/a.mkv", Size: 3, Tags: map[string][]string{"actor": {"X"}},
	})
	if err := ix.Save(root); err != nil {
		t.Fatal(err)
	}
	loaded, err := Load(root)
	if err != nil {
		t.Fatal(err)
	}
	rec := loaded.Files[0]
	if rec.ID != "abc" || rec.DisplayName != "a.mkv" || !rec.Stored() {
		t.Fatalf("bad roundtrip: %+v", rec)
	}
	if rec.Location() != ".files/a.mkv" {
		t.Fatalf("bad location: %q", rec.Location())
	}
	if loaded.Storey.Schema != Schema || loaded.Storey.Algorithm != Algorithm {
		t.Fatalf("bad header: %+v", loaded)
	}
}

func TestSaveUsesStoreyHeader(t *testing.T) {
	root := t.TempDir()
	if err := New().Save(root); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join(root, FileName))
	if err != nil {
		t.Fatal(err)
	}
	var m map[string]any
	if err := json.Unmarshal(data, &m); err != nil {
		t.Fatal(err)
	}
	header, ok := m["storey"].(map[string]any)
	if !ok {
		t.Fatalf("missing storey header in %s", data)
	}
	if header["schema"] != float64(Schema) || header["algorithm"] != Algorithm {
		t.Fatalf("bad storey header: %v", header)
	}
	if _, ok := m["schema"]; ok {
		t.Fatalf("legacy flat schema key must not be written: %s", data)
	}
}

func TestLoadLegacyFlatRejected(t *testing.T) {
	root := t.TempDir()
	legacy := `{"schema":1,"algorithm":"sha256","view":{"by":[]},"files":[]}` + "\n"
	if err := os.WriteFile(filepath.Join(root, FileName), []byte(legacy), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := Load(root); err == nil {
		t.Fatal("expected legacy flat shape to be rejected")
	}
}

func TestLoadRejectsForeignIndex(t *testing.T) {
	root := t.TempDir()
	os.WriteFile(filepath.Join(root, FileName), []byte(`{"files":[]}`), 0o644)
	if _, err := Load(root); err == nil {
		t.Fatal("expected missing-header error")
	}
}

func TestJSONKeys(t *testing.T) {
	data, _ := json.Marshal(Record{})
	var m map[string]any
	json.Unmarshal(data, &m)
	for _, k := range []string{"id", "displayName", "originalRel", "storedRel", "size", "tags"} {
		if _, ok := m[k]; !ok {
			t.Fatalf("missing key %q in %v", k, m)
		}
	}
}

func TestUnstoredLocation(t *testing.T) {
	rec := Record{OriginalRel: "a.txt"}
	if rec.Stored() || rec.Location() != "a.txt" {
		t.Fatal("unstored record misbehaves")
	}
}

func TestLoadRejectsBadSchema(t *testing.T) {
	root := t.TempDir()
	os.WriteFile(filepath.Join(root, FileName), []byte(`{"storey":{"schema":99,"algorithm":"sha256"}}`), 0o644)
	if _, err := Load(root); err == nil {
		t.Fatal("expected schema error")
	}
}

func TestFindPrefix(t *testing.T) {
	ix := New()
	ix.Files = []Record{{ID: "aa00"}, {ID: "aa11"}, {ID: "bb00"}}
	if got := ix.Find("aa00"); got == nil {
		t.Fatal("exact find failed")
	}
	if _, amb := ix.FindPrefix("aa"); amb != true {
		t.Fatal("expected ambiguous")
	}
	rec, amb := ix.FindPrefix("aa0")
	if amb || rec == nil || rec.ID != "aa00" {
		t.Fatalf("expected unique prefix match, got %+v %v", rec, amb)
	}
	if rec, _ := ix.FindPrefix("zz"); rec != nil {
		t.Fatal("expected miss")
	}
}
