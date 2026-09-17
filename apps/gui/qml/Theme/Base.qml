pragma Singleton
import QtQuick

// Base unit for the spacing/typography scales. `remPx` is applied
// from the StoreyLibrary (ui.toml `[gui] rem_px`) at startup;
// everything else derives from it, so one value rescales the UI.
QtObject {
    id: root

    property real remPx: 16

    // Grid-safe multipliers only (whole / quarter / sixteenth steps).
    function isValid(value) {
        if (typeof value !== "number" || !Number.isFinite(value))
            return false;
        if (value >= 4)
            return Number.isInteger(value);
        if (value >= 1)
            return Number.isInteger(value * 4);
        if (value >= 0)
            return Number.isInteger(value * 16);
        return false;
    }
    function rem(multiplier) {
        if (!isValid(multiplier))
            throw new Error("Base.rem: invalid multiplier " + multiplier);
        return multiplier * root.remPx;
    }
    function apply(library) {
        var overrides = JSON.parse(library.gui_overrides || "{}");
        if (overrides.rem_px > 0)
            root.remPx = overrides.rem_px;
    }
}
