package main

import (
	"fmt"
	"os"
	"os/signal"

	"github.com/matyeusm/storey/internal/index"
)

// withEmergencySave runs fn, saving the index first when an interrupt
// arrives mid-move: moved files with an unsaved index strand the
// library (view then refuses, verify then prunes tags). Returns
// exitInterrupted; the caller's deferred lock release still runs.
func withEmergencySave(root string, ix *index.Index, fn func() int) int {
	sigs := make(chan os.Signal, 2)
	signal.Notify(sigs, interruptSignals...)
	defer signal.Stop(sigs)
	done := make(chan int, 1)
	go func() { done <- fn() }()
	select {
	case code := <-done:
		return code
	case <-sigs:
		signal.Stop(sigs)
		if err := ix.Save(root); err != nil {
			fmt.Fprintf(os.Stderr, "storey: interrupted: %s (index may be stale)\n", err)
		} else {
			fmt.Fprintf(os.Stderr, "storey: interrupted, index saved\n")
		}
		return exitInterrupted
	}
}
