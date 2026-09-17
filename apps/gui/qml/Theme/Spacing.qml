pragma Singleton
import QtQuick

// Spacing scale off Base; same steps as the quickshell shell.
QtObject {
    readonly property real xxs: Base.rem(0.25)
    readonly property real xs: Base.rem(0.5)
    readonly property real sm: Base.rem(0.75)
    readonly property real md: Base.rem(1)
    readonly property real lg: Base.rem(2)
    readonly property real xl: Base.rem(3)
    readonly property real xxl: Base.rem(4)
}
