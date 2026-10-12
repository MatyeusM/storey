// Package hash provides SHA-256 file hashing with a worker pool.
// Worker default is NumCPU-1 (one core left for UI/OS), per
// docs/ARCHITECTURE.md.
package hash

import (
	"crypto/sha256"
	"encoding/hex"
	"io"
	"os"
	"runtime"
	"sync"
)

// Workers returns the default pool size: NumCPU-1, minimum 1.
func Workers() int {
	if n := runtime.NumCPU() - 1; n >= 1 {
		return n
	}
	return 1
}

// File hashes one file, returning hex digest and size.
func File(path string) (string, int64, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", 0, err
	}
	defer f.Close()
	h := sha256.New()
	n, err := io.Copy(h, f)
	if err != nil {
		return "", 0, err
	}
	return hex.EncodeToString(h.Sum(nil)), n, nil
}

// Result is the outcome for one path.
type Result struct {
	Path string
	Hash string
	Size int64
	Err  error
}

// Files hashes paths concurrently. progress, when non-nil, is called
// after each file with (done, total, path). Results arrive unordered.
func Files(paths []string, workers int, progress func(done, total int, path string)) []Result {
	if workers < 1 {
		workers = 1
	}
	jobs := make(chan string)
	out := make(chan Result)
	var wg sync.WaitGroup
	for range workers {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for p := range jobs {
				h, n, err := File(p)
				out <- Result{Path: p, Hash: h, Size: n, Err: err}
			}
		}()
	}
	go func() {
		defer close(jobs)
		for _, p := range paths {
			jobs <- p
		}
	}()
	go func() {
		wg.Wait()
		close(out)
	}()
	var results []Result
	done := 0
	for r := range out {
		results = append(results, r)
		done++
		if progress != nil {
			progress(done, len(paths), r.Path)
		}
	}
	return results
}
