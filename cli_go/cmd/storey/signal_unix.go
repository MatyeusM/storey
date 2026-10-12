//go:build !windows

package main

import (
	"os"
	"syscall"
)

// interruptSignals traps Ctrl-C everywhere and SIGTERM on unix so a
// mid-move abort can save the index before exiting.
var interruptSignals = []os.Signal{os.Interrupt, syscall.SIGTERM}
