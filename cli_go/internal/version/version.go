// Package version is the single in-code version source for the CLI.
// It must match the VERSION file at repo root and the release tag.
package version

// Version is set here and overridden at release link time via -ldflags.
// Keep the literal in sync with ../../../../VERSION.
const Version = "0.1.0"
