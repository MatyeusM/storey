pragma Singleton
import QtQuick

// Central glyph registry: hardcoded Symbols Nerd Font codepoints.
// Views reference Icons.name instead of scattering raw "\uf..."
// strings, so swaps and audits happen in one place.
QtObject {
    readonly property string up: "\uf062"
    readonly property string folder: "\uf114"
    readonly property string file: "\uf016"
    readonly property string search: "\uf002"
    readonly property string alert: "\uf071"
    readonly property string plus: "\uf067"
    readonly property string close: "\uf00d"
    readonly property string external: "\uf08e"
    readonly property string tag: "\uf02b"
    readonly property string plugin: "\uf1e6"
    readonly property string grid: "\uf009"
    readonly property string check: "\uf00c"
    readonly property string home: "\uf015"
    readonly property string trash: "\uf1f8"
    readonly property string chevronRight: "\uf054"
    readonly property string refresh: "\uf021"
}
