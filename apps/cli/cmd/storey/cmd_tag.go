package main

import (
	"fmt"
	"strings"

	"github.com/matyeusm/storey/internal/log"
)

// cmdTag dispatches tag subcommands: get, set, add, remove.
// Reads live under get so writes are never confused with queries.
func cmdTag(args []string, asJSON bool, logger *log.Logger, emit func(any)) int {
	_ = logger
	if len(args) == 0 {
		return tagUsage()
	}
	verb, rest := args[0], args[1:]
	switch verb {
	case "get":
		return cmdTagGet(rest, asJSON, emit)
	case "set":
		return cmdTagWrite(rest, asJSON, emit, false)
	case "add":
		return cmdTagWrite(rest, asJSON, emit, true)
	case "remove":
		return cmdTagRemove(rest, asJSON, emit)
	default:
		return usageErr("tag: unknown subcommand %q", verb)
	}
}

func tagUsage() int {
	return usageErr("tag: usage: tag get [target] [--category=C] | tag set|add <target> --category=C --values=a,b | tag remove <target> [--category=C] [--values=a]")
}

// parseCatVals splits --category and --values flags from positionals.
// Unknown -- flags are usage errors.
func parseCatVals(args []string) (cat, raw string, pos []string, exit int) {
	for i := 0; i < len(args); i++ {
		a := args[i]
		switch {
		case a == "--category" && i+1 < len(args):
			i++
			cat = args[i]
		case strings.HasPrefix(a, "--category="):
			cat = strings.TrimPrefix(a, "--category=")
		case a == "--values" && i+1 < len(args):
			i++
			raw = args[i]
		case strings.HasPrefix(a, "--values="):
			raw = strings.TrimPrefix(a, "--values=")
		case strings.HasPrefix(a, "--"):
			return "", "", nil, usageErr("tag: unknown flag %q", a)
		default:
			pos = append(pos, a)
		}
	}
	return cat, raw, pos, exitOK
}

func contains(list []string, s string) bool {
	for _, v := range list {
		if v == s {
			return true
		}
	}
	return false
}

// splitValues splits comma-separated values, trims spaces, drops dupes.
// Empty entries are usage errors.
func splitValues(raw string) ([]string, error) {
	var vals []string
	for _, v := range strings.Split(raw, ",") {
		v = strings.TrimSpace(v)
		if v == "" {
			return nil, fmt.Errorf("empty value in %q", raw)
		}
		if err := checkName(v); err != nil {
			return nil, err
		}
		if !contains(vals, v) {
			vals = append(vals, v)
		}
	}
	return vals, nil
}
