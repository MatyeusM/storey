package views

import (
	"testing"

	"github.com/matyeusm/storey/internal/index"
)

func rec(id, name string, tags map[string][]string) index.Record {
	return index.Record{ID: id, DisplayName: name, Tags: tags}
}

func paths(links []Link) []string {
	var out []string
	for _, l := range links {
		out = append(out, l.ViewPath)
	}
	return out
}

func TestPlanCartesianAndUnsorted(t *testing.T) {
	records := []index.Record{
		rec("1", "g.mkv", map[string][]string{"date": {"1990"}, "actor": {"X", "Y"}}),
		rec("2", "doc.txt", nil),
		rec("3", "p.mkv", map[string][]string{"date": {"1990"}}),
	}
	got := paths(Plan(records, []string{"date", "actor"}))
	want := map[string]bool{
		"1990/X/g.mkv": true, "1990/Y/g.mkv": true,
		"Unsorted/doc.txt":    true,
		"1990/Unsorted/p.mkv": true,
	}
	if len(got) != len(want) {
		t.Fatalf("got %v", got)
	}
	for _, p := range got {
		if !want[p] {
			t.Fatalf("unexpected %q in %v", p, got)
		}
	}
}

func TestPlanDisambiguates(t *testing.T) {
	records := []index.Record{
		rec("1", "a.mkv", map[string][]string{"date": {"2020"}}),
		rec("2", "a.mkv", map[string][]string{"date": {"2020"}}),
		rec("3", "A.mkv", map[string][]string{"date": {"2020"}}),
	}
	got := paths(Plan(records, []string{"date"}))
	want := []string{"2020/a.mkv", "2020/a__2.mkv", "2020/A__3.mkv"}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("got %v want %v", got, want)
		}
	}
}

func TestStoreName(t *testing.T) {
	taken := map[string]string{"a.mkv": "other"}
	if got := StoreName("b.mkv", taken, "id"); got != "b.mkv" {
		t.Fatalf("free name: %q", got)
	}
	if got := StoreName("a.mkv", taken, "other"); got != "a.mkv" {
		t.Fatalf("same content: %q", got)
	}
	if got := StoreName("a.mkv", taken, "id"); got != "a_1.mkv" {
		t.Fatalf("collision: %q", got)
	}
	taken["a_1.mkv"] = "third"
	if got := StoreName("a.mkv", taken, "id"); got != "a_2.mkv" {
		t.Fatalf("second collision: %q", got)
	}
}
