pragma Singleton
import QtQuick

// Win11-like light palette. `apply` overwrites the tokens from the
// StoreyLibrary, which already merged ui.toml `[gui]` overrides
// over these same defaults. (Named AppTheme: short theme names
// risk colliding with Qt-provided QML singletons and properties.)
QtObject {
    id: root

    property color background: "#f3f3f3"
    property color surface: "#ffffff"
    property color border: "#e1e1e1"
    property color text: "#1b1b1b"
    property color muted: "#605e5c"
    property color accent: "#005fb8"
    property color danger: "#c42b1c"

    function apply(library) {
        var overrides = JSON.parse(library.gui_overrides || "{}");
        if (overrides.background)
            root.background = Color.fromString(overrides.background);
        if (overrides.surface)
            root.surface = Color.fromString(overrides.surface);
        if (overrides.border)
            root.border = Color.fromString(overrides.border);
        if (overrides.text)
            root.text = Color.fromString(overrides.text);
        if (overrides.muted)
            root.muted = Color.fromString(overrides.muted);
        if (overrides.accent)
            root.accent = Color.fromString(overrides.accent);
        if (overrides.danger)
            root.danger = Color.fromString(overrides.danger);
    }
}
