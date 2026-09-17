pragma Singleton
import QtQuick

// PT Sans for UI copy, Symbols Nerd Font for icons.
//
// Family names are hardcoded ("PT Sans", "Symbols Nerd Font") so the
// loaders can live in a Repeater: one weight set for copy, a single
// file for icons (it needs no repeater). `sansFamily` defaults to
// the bundled PT Sans and is assignable via ui.toml `[gui]`
// sans_family (a system family then); apply() picks it up at startup
// like Base/Color.
//
// Root is Item (not QtObject): QtObject has no default property,
// so declarative FontLoader children fail to create.
Item {
    id: root

    property string sansFamily: "PT Sans"
    readonly property string iconFamily: "Symbols Nerd Font"

    // Side-effect loads: register the families with the engine
    // (direct children: Repeater delegates must be Items, and a
    // FontLoader is not one, so the fonts never loaded that way).
    FontLoader {
        id: iconLoader
        source: "qrc:/qt/qml/com/storey/gui/qml/Theme/fonts/SymbolsNerdFont-Regular.ttf"
    }
    FontLoader {
        source: "qrc:/qt/qml/com/storey/gui/qml/Theme/fonts/PTSans-Regular.ttf"
    }
    FontLoader {
        source: "qrc:/qt/qml/com/storey/gui/qml/Theme/fonts/PTSans-Italic.ttf"
    }
    FontLoader {
        source: "qrc:/qt/qml/com/storey/gui/qml/Theme/fonts/PTSans-Bold.ttf"
    }

    readonly property real xs: Base.rem(0.75)
    readonly property real sm: Base.rem(0.875)
    readonly property real md: Base.rem(1)
    readonly property real lg: Base.rem(1.25)
    readonly property real xl: Base.rem(1.5)
    readonly property real xxl: Base.rem(1.75)

    function font(px, family, weight) {
        return {
            family: family || root.sansFamily,
            pixelSize: px || root.md,
            weight: weight || Font.Normal
        };
    }
    function apply(library) {
        var overrides = JSON.parse(library.gui_overrides || "{}");
        if (overrides.sans_family)
            root.sansFamily = overrides.sans_family;
    }
}
