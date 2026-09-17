// Package log provides stderr diagnostics for the CLI. Stdout stays
// reserved for the --json envelope, so everything here goes to stderr:
// human-readable text by default, one JSON object per line under --json
// so the TUI and GUI can consume progress and warnings as a stream.
package log

import (
	"encoding/json"
	"fmt"
	"os"
	"sync"
)

// Logger writes diagnostics to stderr.
type Logger struct {
	asJSON bool
	quiet  bool
	mu     sync.Mutex
}

// New returns a logger. quiet suppresses info and progress lines;
// warnings and errors always print.
func New(asJSON, quiet bool) *Logger {
	return &Logger{asJSON: asJSON, quiet: quiet}
}

// Infof prints a diagnostic. Suppressed when quiet.
func (l *Logger) Infof(format string, args ...any) {
	if l.quiet {
		return
	}
	l.emit("info", fmt.Sprintf(format, args...), "", 0, 0, "")
}

// Warnf prints a warning. Always printed: warnings are load-bearing
// (e.g. a view link that could not be created).
func (l *Logger) Warnf(format string, args ...any) {
	l.emit("warn", fmt.Sprintf(format, args...), "", 0, 0, "")
}

// Progress reports hashing work: op is "hash" or "verify", path is the
// slash-relative file just finished. Suppressed when quiet.
func (l *Logger) Progress(op string, done, total int, path string) {
	if l.quiet || total <= 0 {
		return
	}
	l.emit("progress", "", op, done, total, path)
}

type event struct {
	Level string `json:"level"`
	Msg   string `json:"msg,omitempty"`
	Op    string `json:"op,omitempty"`
	Done  int    `json:"done,omitempty"`
	Total int    `json:"total,omitempty"`
	Path  string `json:"path,omitempty"`
}

func (l *Logger) emit(level, msg, op string, done, total int, path string) {
	l.mu.Lock()
	defer l.mu.Unlock()
	if l.asJSON {
		enc := json.NewEncoder(os.Stderr)
		_ = enc.Encode(event{Level: level, Msg: msg, Op: op, Done: done, Total: total, Path: path})
		return
	}
	switch level {
	case "warn":
		fmt.Fprintf(os.Stderr, "storey: warning: %s\n", msg)
	case "info":
		fmt.Fprintf(os.Stderr, "storey: %s\n", msg)
	case "progress":
		// One line per update, always appended: rewriting the line
		// with \r leaves trailing characters when a shorter path
		// follows a longer one.
		fmt.Fprintf(os.Stderr, "storey: %s [%d/%d] %s\n", op, done, total, path)
	}
}
