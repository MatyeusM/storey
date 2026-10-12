package main

import (
	"testing"

	"github.com/matyeusm/storey/internal/index"
	"github.com/matyeusm/storey/internal/log"
)

// testLogger silences progress and info lines during tests.
func testLogger() *log.Logger {
	return log.New(false, true)
}

func sampleRecords() []index.Record {
	return []index.Record{
		{ID: "a", Tags: map[string][]string{"actor": {"X"}}},
		{ID: "b", Tags: map[string][]string{"actor": {"Y"}, "date": {"2020"}}},
		{ID: "c", Tags: map[string][]string{}},
	}
}

func ids(files []index.Record) []string {
	var out []string
	for _, rec := range files {
		out = append(out, rec.ID)
	}
	return out
}

func TestFilterRecords(t *testing.T) {
	files := sampleRecords()
	if got := ids(filterRecords(files, false, nil)); len(got) != 3 {
		t.Fatalf("all: %v", got)
	}
	if got := ids(filterRecords(files, true, nil)); len(got) != 1 || got[0] != "c" {
		t.Fatalf("untagged: %v", got)
	}
	if got := ids(filterRecords(files, false, &catVal{"actor", "X"})); len(got) != 1 || got[0] != "a" {
		t.Fatalf("actor=X: %v", got)
	}
	if got := ids(filterRecords(files, false, &catVal{"date", "2020"})); len(got) != 1 {
		t.Fatalf("date=2020: %v", got)
	}
	if got := filterRecords(files, false, &catVal{"actor", "Z"}); len(got) != 0 {
		t.Fatalf("actor=Z should be empty: %v", got)
	}
}

func TestListArgsAndEnvelope(t *testing.T) {
	setupTaggedLib(t)
	mustRun(t, "tag", "set", "a.txt", "--category=actor", "--values=X")
	var data any
	emit := func(d any) { data = d }
	if code := cmdList(nil, true, testLogger(), emit); code != exitOK {
		t.Fatalf("list: exit %d", code)
	}
	matched, ok := data.([]index.Record)
	if !ok || len(matched) != 1 {
		t.Fatalf("envelope data: %#v", data)
	}
	if code := cmdList([]string{"actor=X"}, true, testLogger(), emit); code != exitOK {
		t.Fatalf("filter: exit %d", code)
	}
	for _, args := range [][]string{{"a=1", "b=2"}, {"--untagged", "a=1"}, {"nofilter"}, {"a="}, {"=v"}} {
		if code := cmdList(args, false, testLogger(), emit); code != exitUsage {
			t.Fatalf("%v: exit %d, want %d", args, code, exitUsage)
		}
	}
	if code := cmdList([]string{"--untagged"}, true, testLogger(), emit); code != exitOK {
		t.Fatalf("untagged: exit %d", code)
	}
}
