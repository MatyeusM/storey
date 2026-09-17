package log

import (
	"encoding/json"
	"os"
	"strings"
	"testing"
)

// captureStderr runs fn while redirecting os.Stderr into a buffer.
func captureStderr(t *testing.T, fn func()) string {
	t.Helper()
	old := os.Stderr
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stderr = w
	fn()
	w.Close()
	os.Stderr = old
	out := make([]byte, 0, 1024)
	buf := make([]byte, 256)
	for {
		n, err := r.Read(buf)
		out = append(out, buf[:n]...)
		if err != nil {
			break
		}
	}
	r.Close()
	return string(out)
}

func decodeLines(t *testing.T, out string) []map[string]any {
	t.Helper()
	var events []map[string]any
	for _, line := range strings.Split(strings.TrimSpace(out), "\n") {
		if line == "" {
			continue
		}
		var m map[string]any
		if err := json.Unmarshal([]byte(line), &m); err != nil {
			t.Fatalf("not JSONL: %q", line)
		}
		events = append(events, m)
	}
	return events
}

func TestJSONLines(t *testing.T) {
	out := captureStderr(t, func() {
		l := New(true, false)
		l.Infof("hello")
		l.Progress("hash", 1, 2, "a.txt")
		l.Warnf("oops")
	})
	events := decodeLines(t, out)
	if len(events) != 3 {
		t.Fatalf("want 3 events, got %v", events)
	}
	if events[0]["level"] != "info" || events[0]["msg"] != "hello" {
		t.Fatalf("bad info: %v", events[0])
	}
	p := events[1]
	if p["level"] != "progress" || p["op"] != "hash" || p["path"] != "a.txt" {
		t.Fatalf("bad progress: %v", p)
	}
	if p["done"] != float64(1) || p["total"] != float64(2) {
		t.Fatalf("bad counts: %v", p)
	}
	if events[2]["level"] != "warn" {
		t.Fatalf("bad warn: %v", events[2])
	}
}

func TestQuietSuppressesInfoAndProgress(t *testing.T) {
	out := captureStderr(t, func() {
		l := New(false, true)
		l.Infof("hidden")
		l.Progress("hash", 1, 2, "a.txt")
		l.Warnf("shown")
	})
	if strings.Contains(out, "hidden") || strings.Contains(out, "a.txt") {
		t.Fatalf("quiet leaked: %q", out)
	}
	if !strings.Contains(out, "shown") {
		t.Fatalf("warn must print when quiet: %q", out)
	}
}

func TestHumanProgressAppendsLines(t *testing.T) {
	out := captureStderr(t, func() {
		l := New(false, false)
		l.Progress("hash", 1, 2, "a-long-filename.mkv")
		l.Progress("hash", 2, 2, "b.mkv")
	})
	lines := strings.Split(strings.TrimSpace(out), "\n")
	if len(lines) != 2 {
		t.Fatalf("want one line per update, got %q", out)
	}
	if lines[1] != "storey: hash [2/2] b.mkv" {
		t.Fatalf("bad progress line: %q", lines[1])
	}
}
