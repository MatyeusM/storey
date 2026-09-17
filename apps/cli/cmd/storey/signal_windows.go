//go:build windows

package main

import "os"

// interruptSignals traps Ctrl-C/Ctrl-Break; unix SIGTERM has no
// windows equivalent, so only the portable signal is listed.
var interruptSignals = []os.Signal{os.Interrupt}
