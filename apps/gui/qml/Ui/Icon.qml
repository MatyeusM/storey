import QtQuick
import com.storey.gui

// Nerd-font glyph with optical centering and height normalization:
// (own-module import: unqualified sibling references only resolve
// within the same directory, and Icon lives in Ui/ while
// the Theme singletons live in Theme/.)
// MDI/FontAwesome shapes carry different ink placement AND heights,
// so raw Text looks off-center and uneven. The painted bounds (not
// the advance) are centered via tightBoundingRect, and the ink
// height is normalized to a fixed share of the box.
//
//   Icon { glyph: "\uf011"; size: 16; color: AppTheme.text }
//
Item {
    id: root

    // Single nerd-font glyph to render. Empty renders nothing.
    property string glyph: ""
    property real size: 16
    property color color: AppTheme.text
    readonly property bool valid: root.glyph !== ""
    readonly property int renderedSize: Math.max(1, Math.round(root.size))
    // Share of the box the ink height is normalized to.
    readonly property real inkShare: 0.72
    readonly property real inkHeight: Math.max(1, inkMetrics.tightBoundingRect.height)
    readonly property real heightScale: Math.min(1.5, Math.max(0.66, root.size * root.inkShare / root.inkHeight))
    readonly property int scaledSize: Math.max(1, Math.round(root.renderedSize * root.heightScale))
    // Tight bounds at the painted size, for the centering math below.
    readonly property real scaledTightX: inkMetrics.tightBoundingRect.x * root.heightScale
    readonly property real scaledTightWidth: Math.max(1, inkMetrics.tightBoundingRect.width * root.heightScale)
    readonly property real horizontalCorrection: ink.implicitWidth / 2 - (root.scaledTightX + root.scaledTightWidth / 2)

    implicitWidth: root.size
    implicitHeight: root.size

    TextMetrics {
        id: inkMetrics
        font.family: Typography.iconFamily
        font.pixelSize: root.renderedSize
        font.weight: Font.Bold
        text: root.glyph
    }
    Text {
        id: ink
        anchors.centerIn: parent
        anchors.horizontalCenterOffset: root.horizontalCorrection
        text: root.glyph
        color: root.color
        font.family: Typography.iconFamily
        font.pixelSize: root.scaledSize
        font.weight: Font.Bold
        renderType: Text.NativeRendering
        visible: root.valid
    }
}
